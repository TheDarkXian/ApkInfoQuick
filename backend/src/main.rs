use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use apk_info_backend::cli::{CliArgs, Commands};
use apk_info_backend::model::ApkInfoEnvelope;
use apk_info_backend::parser::parse_apk_to_envelope;
use clap::Parser;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ParseRecord {
    path: String,
    envelope: ApkInfoEnvelope,
    icon_exported_to: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorReport {
    ok: bool,
    apk_ready: bool,
    aab_ready: bool,
    cwd: String,
    exe: Option<String>,
    aapt: Option<String>,
    bundletool: Option<String>,
    java: Option<String>,
    tools_dir: Option<String>,
    warnings: Vec<String>,
}

fn main() {
    let args = CliArgs::parse();
    let exit_code = match run(args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(args: CliArgs) -> Result<i32, String> {
    match args.command {
        Commands::Parse {
            inputs,
            recursive,
            text,
            pretty,
            compact,
            quiet,
            out,
            export_icon,
            template,
        } => {
            let apk_paths = collect_apk_paths(&inputs, recursive);
            if apk_paths.is_empty() {
                return Err("No APK/AAB files found.".to_string());
            }

            let total = apk_paths.len();
            let template_text = if text {
                load_text_template(template.as_deref())
            } else {
                None
            };
            let mut records = Vec::new();
            for (index, path) in apk_paths.into_iter().enumerate() {
                if !quiet {
                    eprint_progress_start(index + 1, total, &path);
                }
                let envelope = parse_apk_to_envelope(&path);
                let icon_exported_to = if envelope.success {
                    export_icon
                        .as_ref()
                        .and_then(|dir| export_icon_file(&envelope, &path, dir).ok())
                } else {
                    None
                };
                if !quiet {
                    eprint_progress_done(envelope.success);
                }
                records.push(ParseRecord {
                    path: path.to_string_lossy().to_string(),
                    envelope,
                    icon_exported_to,
                });
            }

            let rendered = if text {
                render_text(&records, template_text.as_deref())
            } else {
                render_json(&records, compact && !pretty)?
            };

            write_output(out.as_deref(), &rendered)?;
            Ok(if records.iter().any(|item| !item.envelope.success) {
                2
            } else {
                0
            })
        }
        Commands::Doctor { compact } => {
            let report = build_doctor_report();
            let rendered = if compact {
                serde_json::to_string(&report)
            } else {
                serde_json::to_string_pretty(&report)
            }
            .map_err(|err| err.to_string())?;
            println!("{rendered}");
            Ok(0)
        }
    }
}

fn collect_apk_paths(inputs: &[PathBuf], recursive: bool) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();

    for input in inputs {
        let candidates = if input.is_dir() {
            collect_from_dir(input, recursive)
        } else {
            vec![input.clone()]
        };

        for path in candidates {
            if !is_supported_input_path(&path) {
                continue;
            }
            let key = canonical_key(&path);
            if seen.insert(key) {
                out.push(path);
            }
        }
    }

    out
}

fn collect_from_dir(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && recursive {
            out.extend(collect_from_dir(&path, true));
        } else if path.is_file() && is_supported_input_path(&path) {
            out.push(path);
        }
    }

    out.sort();
    out
}

fn is_supported_input_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("apk") || ext.eq_ignore_ascii_case("aab"))
        .unwrap_or(false)
}

fn canonical_key(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

fn eprint_progress_start(index: usize, total: usize, path: &Path) {
    let file_name = get_file_name(path);
    let ext = get_ext(path).to_uppercase();
    if ext == "AAB" {
        eprint!(
            "Parsing [{index}/{total}] {file_name} ({ext})... converting with bundletool, this may take a while... "
        );
    } else {
        eprint!("Parsing [{index}/{total}] {file_name} ({ext})... ");
    }
}

fn eprint_progress_done(success: bool) {
    if success {
        eprintln!("ok");
    } else {
        eprintln!("failed");
    }
}

fn get_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(str::to_string)
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn get_ext(path: &Path) -> String {
    path.extension()
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn load_text_template(custom_path: Option<&Path>) -> Option<String> {
    if let Some(path) = custom_path {
        return fs::read_to_string(path).ok();
    }

    for root in current_roots() {
        for relative in [
            Path::new("frontend")
                .join("templates")
                .join("copy-text.template.txt"),
            Path::new("templates").join("copy-text.template.txt"),
        ] {
            let candidate = root.join(relative);
            if let Ok(template) = fs::read_to_string(candidate) {
                return Some(template);
            }
        }
    }

    None
}

fn render_json(records: &[ParseRecord], compact: bool) -> Result<String, String> {
    if records.len() == 1 && records[0].icon_exported_to.is_none() {
        if compact {
            serde_json::to_string(&records[0].envelope)
        } else {
            serde_json::to_string_pretty(&records[0].envelope)
        }
    } else if compact {
        serde_json::to_string(records)
    } else {
        serde_json::to_string_pretty(records)
    }
    .map_err(|err| err.to_string())
}

fn render_text(records: &[ParseRecord], template: Option<&str>) -> String {
    records
        .iter()
        .map(|record| render_text_record(record, template))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

fn render_text_record(record: &ParseRecord, template: Option<&str>) -> String {
    if let Some(template) = template {
        return render_template_record(record, template);
    }

    let data = &record.envelope.data;
    let mut lines = vec![
        format!("FilePath: {}", record.path),
        format!("Success: {}", record.envelope.success),
        format!("PackageName: {}", data.package_name),
        format!("AppName: {}", data.app_name),
        format!("Channel: {}", data.channel),
        format!("MinSdkVersion: {}", data.min_sdk_version),
        format!("TargetSdkVersion: {}", data.target_sdk_version),
        format!(
            "CompileSdkVersion: {}",
            data.compile_sdk_version
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string())
        ),
        format!("VersionCode: {}", data.version_code),
        format!(
            "VersionName: {}",
            data.version_name.as_deref().unwrap_or("null")
        ),
        format!("IconUrl: {}", empty_text(&data.icon_url)),
        format!(
            "IconExportedTo: {}",
            empty_text(record.icon_exported_to.as_deref().unwrap_or(""))
        ),
        format!("Permissions: {}", join_or_empty(&data.permissions)),
        format!("ABIs: {}", join_or_empty(&data.abis)),
        format!("Signers: {}", data.signers.len()),
        format!("Warnings: {}", join_or_empty(&record.envelope.warnings)),
        format!(
            "ErrorCode: {}",
            empty_text(record.envelope.error_code.as_deref().unwrap_or(""))
        ),
        format!(
            "ErrorMessage: {}",
            empty_text(record.envelope.error_message.as_deref().unwrap_or(""))
        ),
    ];

    if !data.signers.is_empty() {
        for (index, signer) in data.signers.iter().enumerate() {
            lines.push(format!(
                "Signer#{}: scheme={}, sha256={}, issuer={}, subject={}",
                index + 1,
                empty_text(&signer.scheme),
                empty_text(&signer.cert_sha256),
                empty_text(&signer.issuer),
                empty_text(&signer.subject)
            ));
        }
    }

    lines.join("\n")
}

fn render_template_record(record: &ParseRecord, template: &str) -> String {
    let fields = template_fields(record);
    replace_template_placeholders(
        &template
            .split('\n')
            .filter(|line| !line.trim().starts_with("# "))
            .collect::<Vec<_>>()
            .join("\n")
            .replace('\r', ""),
        &fields,
    )
}

fn join_or_empty(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

fn list_to_template_lines(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn signers_to_template_lines(record: &ParseRecord) -> String {
    let signers = &record.envelope.data.signers;
    if signers.is_empty() {
        return "-".to_string();
    }

    signers
        .iter()
        .enumerate()
        .map(|(index, signer)| {
            [
                format!("Signer #{}", index + 1),
                format!("  scheme: {}", empty_text(&signer.scheme)),
                format!("  certSha256: {}", empty_text(&signer.cert_sha256)),
                format!("  issuer: {}", empty_text(&signer.issuer)),
                format!("  subject: {}", empty_text(&signer.subject)),
                format!("  validFrom: {}", empty_text(&signer.valid_from)),
                format!("  validTo: {}", empty_text(&signer.valid_to)),
            ]
            .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn template_fields(record: &ParseRecord) -> BTreeMap<String, String> {
    let data = &record.envelope.data;
    BTreeMap::from([
        (
            "file_name".to_string(),
            get_file_name(Path::new(&record.path)),
        ),
        ("path".to_string(), record.path.clone()),
        ("ext".to_string(), get_ext(Path::new(&record.path))),
        (
            "status".to_string(),
            if record.envelope.success {
                "success"
            } else {
                "error"
            }
            .to_string(),
        ),
        ("packname".to_string(), data.package_name.clone()),
        ("product_name".to_string(), data.app_name.clone()),
        ("app_name".to_string(), data.app_name.clone()),
        ("channel".to_string(), data.channel.clone()),
        (
            "min_sdk_version".to_string(),
            data.min_sdk_version.to_string(),
        ),
        (
            "target_sdk_version".to_string(),
            data.target_sdk_version.to_string(),
        ),
        (
            "compile_sdk_version".to_string(),
            data.compile_sdk_version
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string()),
        ),
        ("version_code".to_string(), data.version_code.to_string()),
        (
            "version_name".to_string(),
            data.version_name.as_deref().unwrap_or("null").to_string(),
        ),
        (
            "permissions".to_string(),
            list_to_template_lines(&data.permissions),
        ),
        ("abis".to_string(), list_to_template_lines(&data.abis)),
        ("signers".to_string(), signers_to_template_lines(record)),
        (
            "warnings".to_string(),
            list_to_template_lines(&record.envelope.warnings),
        ),
        (
            "error_code".to_string(),
            record
                .envelope
                .error_code
                .as_deref()
                .unwrap_or("-")
                .to_string(),
        ),
        (
            "error_message".to_string(),
            record
                .envelope
                .error_message
                .as_deref()
                .unwrap_or("-")
                .to_string(),
        ),
    ])
}

fn replace_template_placeholders(template: &str, fields: &BTreeMap<String, String>) -> String {
    let mut rendered = template.to_string();
    for (key, value) in fields {
        rendered = rendered.replace(&format!("#{key}#"), value);
    }
    rendered
}

fn empty_text(value: &str) -> &str {
    if value.trim().is_empty() {
        "none"
    } else {
        value
    }
}

fn write_output(out: Option<&Path>, rendered: &str) -> Result<(), String> {
    if let Some(path) = out {
        fs::write(path, rendered).map_err(|err| format!("Failed to write output: {err}"))?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn export_icon_file(
    envelope: &ApkInfoEnvelope,
    apk_path: &Path,
    out_dir: &Path,
) -> Result<String, String> {
    let icon_path = file_url_to_path(&envelope.data.icon_url)
        .ok_or_else(|| "Icon URL is not a local file.".to_string())?;
    fs::create_dir_all(out_dir).map_err(|err| format!("Failed to create icon directory: {err}"))?;

    let ext = icon_path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("png");
    let stem = apk_path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("app");
    let target = out_dir.join(format!("{}-icon.{}", sanitize_filename(stem), ext));
    fs::copy(&icon_path, &target).map_err(|err| format!("Failed to export icon: {err}"))?;
    Ok(target.to_string_lossy().to_string())
}

fn file_url_to_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.strip_prefix("file:///")?;
    let decoded = percent_decode(trimmed);
    if cfg!(windows) {
        Some(PathBuf::from(decoded.replace('/', "\\")))
    } else {
        Some(PathBuf::from(format!("/{decoded}")))
    }
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&raw[i + 1..i + 3], 16) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn sanitize_filename(raw: &str) -> String {
    let cleaned = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    if cleaned.is_empty() {
        "app".to_string()
    } else {
        cleaned
    }
}

fn build_doctor_report() -> DoctorReport {
    let cwd = env::current_dir()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let exe = env::current_exe()
        .ok()
        .map(|path| path.to_string_lossy().to_string());
    let aapt = find_tool(aapt_file_name()).map(|path| path.to_string_lossy().to_string());
    let bundletool = find_bundletool().map(|path| path.to_string_lossy().to_string());
    let java = find_java().map(|path| path.to_string_lossy().to_string());
    let tools_dir = find_tools_dir().map(|path| path.to_string_lossy().to_string());
    let apk_ready = true;
    let aab_ready = bundletool.is_some() && java.is_some();
    let mut warnings = Vec::new();
    if aapt.is_none() {
        warnings.push("AAPT_NOT_FOUND_RUST_FALLBACK_AVAILABLE".to_string());
    }
    if bundletool.is_none() {
        warnings.push("BUNDLETOOL_NOT_FOUND_AAB_UNAVAILABLE".to_string());
    }
    if java.is_none() {
        warnings.push("JAVA_NOT_FOUND_AAB_UNAVAILABLE".to_string());
    }

    DoctorReport {
        ok: true,
        apk_ready,
        aab_ready,
        cwd,
        exe,
        aapt,
        bundletool,
        java,
        tools_dir,
        warnings,
    }
}

fn find_tool(file_name: &str) -> Option<PathBuf> {
    for root in current_roots() {
        let candidate = root.join("tools").join("android").join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if file_name == aapt_file_name() {
        if let Ok(raw) = env::var("APK_INFO_AAPT") {
            let path = PathBuf::from(raw);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_bundletool() -> Option<PathBuf> {
    for root in current_roots() {
        let candidate = root.join("tools").join("android").join("bundletool.jar");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if let Ok(raw) = env::var("APK_INFO_BUNDLETOOL") {
        let path = PathBuf::from(raw);
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

fn find_java() -> Option<PathBuf> {
    if let Ok(java_home) = env::var("JAVA_HOME") {
        let candidate = PathBuf::from(java_home).join("bin").join(if cfg!(windows) {
            "java.exe"
        } else {
            "java"
        });
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    let java_name = if cfg!(windows) { "java.exe" } else { "java" };
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(java_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_tools_dir() -> Option<PathBuf> {
    for root in current_roots() {
        let candidate = root.join("tools").join("android");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

fn current_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(dir) = env::current_dir() {
        out.extend(dir.ancestors().map(Path::to_path_buf));
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            out.extend(parent.ancestors().map(Path::to_path_buf));
        }
    }
    out
}

fn aapt_file_name() -> &'static str {
    if cfg!(windows) {
        "aapt.exe"
    } else {
        "aapt"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apk_info_backend::model::{ApkInfoData, SignerInfo};

    fn sample_record() -> ParseRecord {
        let mut data = ApkInfoData::placeholder();
        data.package_name = "com.example.demo".to_string();
        data.app_name = "Demo".to_string();
        data.min_sdk_version = 23;
        data.target_sdk_version = 35;
        data.compile_sdk_version = Some(35);
        data.version_code = 7;
        data.version_name = Some("1.2.3".to_string());
        data.permissions = vec!["android.permission.INTERNET".to_string()];
        data.abis = vec!["arm64-v8a".to_string()];
        data.signers = vec![SignerInfo {
            scheme: "v2".to_string(),
            cert_sha256: "ABCDEF".to_string(),
            issuer: "unknown".to_string(),
            subject: "CN=Demo".to_string(),
            valid_from: String::new(),
            valid_to: String::new(),
        }];

        ParseRecord {
            path: "D:/tmp/demo.apk".to_string(),
            envelope: ApkInfoEnvelope::ok(data, vec!["CHANNEL_NOT_FOUND".to_string()]),
            icon_exported_to: None,
        }
    }

    #[test]
    fn template_renderer_uses_gui_placeholders_and_skips_comments() {
        let template =
            "# comment line\nPackage: #packname#\nPermissions:\n#permissions#\n#signers#";
        let rendered = render_text_record(&sample_record(), Some(template));

        assert!(!rendered.contains("# comment line"));
        assert!(rendered.contains("Package: com.example.demo"));
        assert!(rendered.contains("- android.permission.INTERNET"));
        assert!(rendered.contains("certSha256: ABCDEF"));
    }

    #[test]
    fn doctor_report_exposes_readiness_flags() {
        let report = build_doctor_report();

        assert!(report.ok);
        assert!(report.apk_ready);
        if !report.aab_ready {
            assert!(!report.warnings.is_empty());
        }
    }
}
