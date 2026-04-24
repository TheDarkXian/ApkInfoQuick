use apk_info_backend::model::ApkInfoEnvelope;
use apk_info_backend::parser::parse_apk_tauri;

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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![parse_apk, pick_files])
        .run(tauri::generate_context!())
        .expect("failed to run ApkInfoQuick tauri app");
}
