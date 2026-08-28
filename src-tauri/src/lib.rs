mod commands;
// pub so integration tests (tests/*.rs) and the CLI binary (bin/nyquist-cli.rs) can
// exercise decode/analysis directly, without going through the Tauri IPC layer.
pub mod analysis;
pub mod bit_depth;
pub mod decode;
pub mod dynamic_range;
pub mod mdct_grid;
pub mod metadata;
pub mod player;
pub mod sample_rate;
pub mod signal_analysis;
pub mod spectral;
pub mod stereo;
pub mod tags;
pub mod transcode_detect;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Audio is played natively from the samples the analysis already decoded, rather
        // than handed to the webview's `<audio>` element. The element kept its own opinion
        // of how long the file was, which is where the wrong seeks, the drifting counter and
        // the early stops all came from. See player.rs.
        //
        // No fallible setup here: the audio device is opened when a file is loaded, so a
        // machine with no output device still analyses files and simply reports that
        // playback is unavailable.
        .manage(player::Player::new())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::analyze_file,
            commands::export_report,
            commands::player_play,
            commands::player_pause,
            commands::player_seek,
            commands::player_set_volume,
            commands::player_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
