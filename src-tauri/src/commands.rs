//! IPC commands exposed to the frontend. See
//! `.claude/skills/tauri-ipc-contract/SKILL.md` before changing the shape of
//! [`AnalysisResult`] — the frontend types in `src/lib/api.ts` must move with it.

use std::path::PathBuf;

use tauri::Manager;

use crate::analysis::{self, AnalysisResult};

/// Decodes and analyzes an audio file.
///
/// Runs on a blocking task rather than the async/event thread — decode + signal analysis
/// is CPU-bound and can take seconds on large high-res files, and must never freeze the
/// UI. Percentage progress events are deferred until a stage slow enough to need them
/// exists — measured ~1.6s in release for decode+signal+spectral+transcode combined on a
/// 7-minute 24-bit FLAC, see `.claude/CONTEXT.md` — well under the threshold that would
/// justify it.
#[tauri::command]
pub async fn analyze_file(path: String) -> Result<AnalysisResult, String> {
    tauri::async_runtime::spawn_blocking(move || analysis::analyze(&PathBuf::from(path)))
        .await
        .map_err(|e| format!("analysis task panicked: {e}"))?
}

/// Grants the webview's `asset://` protocol permission to read exactly this file, so the
/// frontend can play it via a native `<audio>` element (streamed/seekable, never loaded
/// whole into JS memory — see tauri-ipc-contract skill on payload size). Scope is per-file
/// and additive; nothing else on disk becomes readable by calling this.
#[tauri::command]
pub fn authorize_playback(app: tauri::AppHandle, path: String) -> Result<(), String> {
    app.asset_protocol_scope().allow_file(&path).map_err(|e| format!("cannot authorize playback: {e}"))
}

/// Writes a pre-serialized report (the frontend's `AnalysisResult` as JSON, produced by
/// `JSON.stringify` client-side) to a path the user chose via a save dialog. No need to
/// re-derive/re-serialize the analysis on the Rust side — the frontend already has the
/// exact object that was rendered.
#[tauri::command]
pub fn export_report(path: String, json: String) -> Result<(), String> {
    std::fs::write(path, json).map_err(|e| format!("cannot write report: {e}"))
}
