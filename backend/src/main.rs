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
    cwd: String,
    exe: Option<String>,
    aapt: Option<String>,
    bundletool: Option<String>,
    java: Option<String>,
    tools_dir: Option<String>,
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
            out,
            export_icon,
        } => {
            let apk_paths = collect_apk_paths(&inputs, recursive);
            if apk_paths.is_empty() {
                return Err("No APK files found.".to_string());
            }

            let mut records = Vec::new();
            for path in apk_paths {
                let envelope = parse_apk_to_envelope(&path);
                let icon_exported_to = if envelope.success {
                    export_icon
                        .as_ref()
                        .and_then(|dir| export_icon_file(&envelope, &path, dir).ok())
                } else {
                    None
                };
                records.push(ParseRecord {
                    path: path.to_string_lossy().to_string(),
                    envelope,
                    icon_exported_to,
                });
            }

            let rendered = if text {
                render_text(&records)
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
            Ok(if report.ok { 0 } else { 1 })
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

fn render_text(records: &[ParseRecord]) -> String {
    records
        .iter()
        .map(render_text_record)
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

fn render_text_record(record: &ParseRecord) -> String {
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

fn join_or_empty(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
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

    DoctorReport {
        ok: aapt.is_some(),
        cwd,
        exe,
        aapt,
        bundletool,
        java,
        tools_dir,
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
