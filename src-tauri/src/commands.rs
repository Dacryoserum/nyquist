//! IPC commands exposed to the frontend. See
//! `.claude/skills/tauri-ipc-contract/SKILL.md` before changing the shape of
//! [`AnalysisResult`] — the frontend types in `src/lib/api.ts` must move with it.

use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::analysis::{self, AnalysisResult};
use crate::player::{PlaybackState, Player};

/// Serializes memory-heavy analyses and protects playback publication from stale requests.
#[derive(Default)]
pub(crate) struct AnalysisRequests {
    pipeline: Mutex<()>,
    latest_playback: Mutex<Option<String>>,
}

impl AnalysisRequests {
    fn begin_playback(&self, request_id: String, reset: impl FnOnce()) {
        let mut latest = self.latest_playback.lock().unwrap();
        *latest = Some(request_id);
        reset();
    }

    fn is_current(&self, request_id: &str) -> bool {
        self.latest_playback.lock().unwrap().as_deref() == Some(request_id)
    }

    fn publish_playback(&self, request_id: &str, publish: impl FnOnce()) -> Result<(), String> {
        let latest = self.latest_playback.lock().unwrap();
        if latest.as_deref() != Some(request_id) {
            return Err("analysis superseded by a newer file".to_string());
        }
        publish();
        Ok(())
    }
}

/// Completed stages, not a percentage of elapsed time: parallel stages have unequal costs.
#[derive(Clone, Serialize)]
struct AnalysisProgress<'a> {
    request_id: &'a str,
    completed_stages: usize,
    total_stages: usize,
}

/// Analyzes off the event thread and emits request-scoped stage progress.
/// Comparison requests never change playback. Only the latest primary request may publish
/// samples, and queued obsolete requests are discarded before allocating decoded audio.
#[tauri::command]
pub async fn analyze_file(
    app: tauri::AppHandle,
    path: String,
    request_id: String,
    load_playback: bool,
) -> Result<AnalysisResult, String> {
    if load_playback {
        let requests = app.state::<AnalysisRequests>();
        // Stop the old file immediately, including when the new analysis later fails.
        requests.begin_playback(request_id.clone(), || app.state::<Player>().unload());
    }

    tauri::async_runtime::spawn_blocking(move || {
        let requests = app.state::<AnalysisRequests>();
        // No analysis data lives behind this gate. A failed worker may poison it, but a
        // subsequent independent file can safely run after the guard is recovered.
        let _pipeline = requests
            .pipeline
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if load_playback && !requests.is_current(&request_id) {
            return Err("analysis superseded by a newer file".to_string());
        }
        let completed = AtomicUsize::new(0);
        let report_progress = || {
            let _ = app.emit(
                "analysis-progress",
                AnalysisProgress {
                    request_id: &request_id,
                    completed_stages: completed.fetch_add(1, Ordering::Relaxed),
                    total_stages: analysis::PROGRESS_STAGE_COUNT,
                },
            );
        };
        report_progress();
        let (result, decoded) =
            analysis::analyze_with_progress(&PathBuf::from(path), &report_progress)?;

        if load_playback {
            // Keep the guard through publication: a newer request cannot start between
            // the freshness check and replacing the loaded samples.
            requests.publish_playback(&request_id, || {
                // An unavailable audio device must not discard an otherwise valid report.
                let _ = app.state::<Player>().load(decoded);
            })?;
        }
        Ok(result)
    })
    .await
    .map_err(|e| format!("analysis task panicked: {e}"))?
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_obsolete_analysis_cannot_replace_the_latest_playback() {
        let requests = AnalysisRequests::default();
        let resets = AtomicUsize::new(0);
        requests.begin_playback("first".into(), || {
            resets.fetch_add(1, Ordering::Relaxed);
        });
        requests.begin_playback("second".into(), || {
            resets.fetch_add(1, Ordering::Relaxed);
        });
        assert!(!requests.is_current("first"));
        assert!(requests.is_current("second"));
        assert!(requests
            .publish_playback("first", || panic!("stale samples published"))
            .is_err());
        let published = AtomicUsize::new(0);
        requests
            .publish_playback("second", || {
                published.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();
        assert_eq!(published.load(Ordering::Relaxed), 1);
        assert_eq!(resets.load(Ordering::Relaxed), 2);
    }
}
