use apk_info_backend::model::{ApkInfoData, ApkInfoEnvelope};
use apk_info_backend::parser::parse_apk_tauri;
use base64::Engine;
use tauri::{path::BaseDirectory, Manager};

#[tauri::command]
async fn parse_apk(file_path: String) -> ApkInfoEnvelope {
    tauri::async_runtime::spawn_blocking(move || parse_apk_tauri(file_path))
        .await
        .unwrap_or_else(|err| {
            ApkInfoEnvelope::err(
                "PARSE_WORKER_FAILED",
                format!("解析任务执行失败: {err}"),
                ApkInfoData::placeholder(),
                Vec::new(),
            )
        })
}

#[tauri::command]
fn pick_files() -> Vec<String> {
    rfd::FileDialog::new()
        .add_filter("APK / AAB", &["apk", "aab"])
        .pick_files()
        .unwrap_or_default()
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

#[tauri::command]
fn read_icon_data_url(file_path: String) -> Option<String> {
    let bytes = std::fs::read(&file_path).ok()?;
    let lower = file_path.to_lowercase();
    let mime = if lower.ends_with(".webp") {
        "image/webp"
    } else {
        "image/png"
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{mime};base64,{encoded}"))
}

#[tauri::command]
fn export_icon_with_dialog(
    source_file_path: String,
    suggested_file_name: String,
) -> Result<Option<String>, String> {
    let extension = std::path::Path::new(&source_file_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let filter_ext = if extension == "webp" { "webp" } else { "png" };

    let target = rfd::FileDialog::new()
        .set_file_name(&suggested_file_name)
        .add_filter("Image", &[filter_ext])
        .save_file();

    let Some(target_path) = target else {
        return Ok(None);
    };

    let bytes = std::fs::read(&source_file_path).map_err(|e| format!("读取图标失败: {e}"))?;
    std::fs::write(&target_path, bytes).map_err(|e| format!("写入导出文件失败: {e}"))?;
    Ok(Some(target_path.to_string_lossy().to_string()))
}

fn configure_bundled_aapt(app: &tauri::App) {
    let Ok(path) = app
        .path()
        .resolve("tools/android/aapt.exe", BaseDirectory::Resource)
    else {
        return;
    };

    if path.is_file() {
        std::env::set_var("APK_INFO_AAPT", path);
    }
}

fn configure_bundled_bundletool(app: &tauri::App) {
    let Ok(path) = app
        .path()
        .resolve("tools/android/bundletool.jar", BaseDirectory::Resource)
    else {
        return;
    };

    if path.is_file() {
        std::env::set_var("APK_INFO_BUNDLETOOL", path);
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            configure_bundled_aapt(app);
            configure_bundled_bundletool(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            parse_apk,
            pick_files,
            read_icon_data_url,
            export_icon_with_dialog
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ApkInfoQuick tauri app");
}
