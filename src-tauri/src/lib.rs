mod commands;
// pub so integration tests (tests/*.rs) and the CLI binary (bin/nyquist-cli.rs) can
// exercise decode/analysis directly, without going through the Tauri IPC layer.
pub mod analysis;
pub mod bit_depth;
pub mod decode;
pub mod dynamic_range;
pub mod metadata;
pub mod sample_rate;
pub mod signal_analysis;
pub mod spectral;
pub mod tags;
pub mod transcode_detect;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::analyze_file,
            commands::authorize_playback,
            commands::export_report
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
