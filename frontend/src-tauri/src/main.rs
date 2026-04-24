use apk_info_backend::model::ApkInfoEnvelope;
use apk_info_backend::parser::parse_apk_tauri;
use base64::Engine;

#[tauri::command]
fn parse_apk(file_path: String) -> ApkInfoEnvelope {
    parse_apk_tauri(file_path)
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![parse_apk, pick_files, read_icon_data_url])
        .run(tauri::generate_context!())
        .expect("failed to run ApkInfoQuick tauri app");
}
