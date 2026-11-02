//! What the window keeps doing in the background: the shared library, the recording to open
//! at start, the player's clock, the preview's skipped ranges, and the licence. Called once
//! from `App`, in the same order on every render.

use super::*;

/// `open_recording` opens a recording handed over from PolishCast or PublishKit.
pub(super) fn use_background(app: state::App, mut open_recording: impl FnMut(String) + Copy + 'static) {
    // The open folder's recordings in the shared library, kept as they are on disk.
    use_future(move || async move {
        let mut last = None;
        loop {
            let folder = app.folder.peek().clone();
            if let Some(dir) = folder {
                let stamp = tokio::task::spawn_blocking({
                    let dir = dir.clone();
                    move || (crate::library::stamp(&dir), dir)
                })
                .await
                .ok();
                if stamp != last {
                    let _ = tokio::task::spawn_blocking(move || crate::library::sync_folder(&dir)).await;
                    last = stamp;
                }
            }
            if let Some(note) =
                tokio::task::spawn_blocking(|| desktop_kit::handoff::take("trackcut", "open")).await.ok().flatten()
            {
                if let Some(target) = note["open"].as_str() {
                    open_recording(target.to_string());
                }
            }
            tokio::time::sleep(Duration::from_millis(1500)).await;
        }
    });

    use_hook(move || match std::env::args().nth(1) {
        Some(arg) => app.open(std::path::Path::new(&arg)),
        None => {
            if let Some(dir) = state::prefs::last_folder() {
                app.open(&dir);
            }
        }
    });

    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_millis(16)).await;
            app.tick();
        }
    });

    // Re-send the preview's skipped ranges only when they actually change (a marker drag
    // writes the cutlist on every mouse move).
    let skips = use_memo(move || app.wanted_skips());
    use_effect(move || app.player().set_skips(skips()));

    // The licence is checked with api0 at start, then once a day.
    use_future(move || async move {
        let mut licence = app.licence;
        licence.set(desktop_kit::licence::refresh(true).await);
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            licence.set(desktop_kit::licence::refresh(false).await);
        }
    });
}
