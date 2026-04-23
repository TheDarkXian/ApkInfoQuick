use apk_info_backend::model::ApkInfoEnvelope;
use apk_info_backend::parser::parse_apk_tauri;

#[tauri::command]
fn parse_apk(file_path: String) -> ApkInfoEnvelope {
    parse_apk_tauri(file_path)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![parse_apk])
        .run(tauri::generate_context!())
        .expect("failed to run ApkInfoQuick tauri app");
}

