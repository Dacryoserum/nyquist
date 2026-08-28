//! IPC commands exposed to the frontend. See
//! `.claude/skills/tauri-ipc-contract/SKILL.md` before changing the shape of
//! [`AnalysisResult`] — the frontend types in `src/lib/api.ts` must move with it.

use std::path::PathBuf;

use tauri::Manager;

use crate::analysis::{self, AnalysisResult};
use crate::player::{PlaybackState, Player};

/// Decodes and analyzes an audio file.
///
/// Runs on a blocking task rather than the async/event thread — decode + signal analysis
/// is CPU-bound and can take seconds on large high-res files, and must never freeze the
/// UI. Percentage progress events are deferred until a stage slow enough to need them
/// exists — measured ~1.6s in release for decode+signal+spectral+transcode combined on a
/// 7-minute 24-bit FLAC, see `.claude/CONTEXT.md` — well under the threshold that would
/// justify it.
#[tauri::command]
pub async fn analyze_file(app: tauri::AppHandle, path: String) -> Result<AnalysisResult, String> {
    let (result, decoded) = tauri::async_runtime::spawn_blocking(move || {
        analysis::analyze_with_audio(&PathBuf::from(path))
    })
    .await
    .map_err(|e| format!("analysis task panicked: {e}"))??;

    // Playback is loaded from the very samples that produced the report, so the player's
    // clock and the report's clock are the same clock — see player.rs on why that is the
    // whole point. A machine with no audio output still gets its analysis: the load failure
    // is swallowed here and surfaces as "no track loaded" on the next player call.
    let player = app.state::<Player>();
    player.unload();
    let _ = player.load(decoded);

    Ok(result)
}

/// Transport controls for the loaded track.
///
/// Each one returns the resulting [`PlaybackState`], so the UI never has to guess what an
/// action did or issue a second call to find out. Position comes from the sample index handed
/// to the audio device, which is why it cannot drift from what is being heard — the defect
/// that motivated replacing the `<audio>` element.
#[tauri::command]
pub fn player_play(app: tauri::AppHandle) -> Result<PlaybackState, String> {
    app.state::<Player>().play()
}

#[tauri::command]
pub fn player_pause(app: tauri::AppHandle) -> Result<PlaybackState, String> {
    app.state::<Player>().pause()
}

/// Seconds from the start of the track, in the same units as `file_info.duration_seconds`
/// and the spectrogram's time axis. There is only one timeline now.
#[tauri::command]
pub fn player_seek(app: tauri::AppHandle, seconds: f64) -> Result<PlaybackState, String> {
    app.state::<Player>().seek(seconds)
}

#[tauri::command]
pub fn player_set_volume(app: tauri::AppHandle, volume: f32) -> Result<PlaybackState, String> {
    app.state::<Player>().set_volume(volume)
}

/// Polled by the UI to drive the playhead. Cheap: a couple of atomic loads.
#[tauri::command]
pub fn player_state(app: tauri::AppHandle) -> Result<PlaybackState, String> {
    app.state::<Player>().state()
}

/// Writes a pre-serialized report (the frontend's `AnalysisResult` as JSON, produced by
/// `JSON.stringify` client-side) to a path the user chose via a save dialog. No need to
/// re-derive/re-serialize the analysis on the Rust side — the frontend already has the
/// exact object that was rendered.
#[tauri::command]
pub fn export_report(path: String, json: String) -> Result<(), String> {
    std::fs::write(path, json).map_err(|e| format!("cannot write report: {e}"))
}
