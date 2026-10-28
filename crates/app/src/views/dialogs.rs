//! The dialogs: deleting a recording, downloading from a link, pasting a track list.

use super::*;

/// "Move to the Trash?" for a recording in the file list. Enter confirms, Esc cancels.
#[component]
pub(super) fn DeleteDialog() -> Element {
    let app = use_context::<state::App>();
    let Some(path) = (app.confirm_delete)() else { return rsx! {} };
    let name = key_of(&path);
    let edits = app.cutlist.read().recordings.get(&name).is_some_and(|e| !e.splits.is_empty());
    let close = move || {
        let mut confirm = app.confirm_delete;
        confirm.set(None);
        app.focus_root();
    };
    let confirm = move || {
        let target = app.confirm_delete.peek().clone();
        close();
        if let Some(p) = target {
            app.delete_recording(&p);
        }
    };
    rsx! {
        div { class: "modal-backdrop", onclick: move |_| close(),
            div {
                class: "modal",
                tabindex: "0",
                onclick: move |e| e.stop_propagation(),
                onmounted: move |e| async move {
                    let _ = e.set_focus(true).await;
                },
                onkeydown: move |e| {
                    e.stop_propagation();
                    match e.key() {
                        Key::Enter => confirm(),
                        Key::Escape => close(),
                        _ => {}
                    }
                },
                div { class: "modal-head",
                    h2 { "Move to the Trash?" }
                    button { class: "modal-close", title: "Close", onclick: move |_| close(), "×" }
                }
                div { class: "modal-body",
                    p { class: "delete-name", "{name}" }
                    p { class: "dim",
                        "The file goes to the system Trash, so you can still restore it from there."
                        if edits { " Its splits and titles in TrackCut are removed." }
                        " Tracks you already exported are not touched."
                    }
                    div { class: "modal-actions",
                        button { onclick: move |_| close(), "Cancel" kbd { "Esc" } }
                        button { class: "danger", onclick: move |_| confirm(), "Move to Trash" kbd { "Enter" } }
                    }
                }
            }
        }
    }
}

/// Progress of the URL download, under the sidebar header.
#[component]
pub(super) fn DownloadStatus() -> Element {
    let app = use_context::<state::App>();
    let Some(phase) = app.download.read().as_ref().map(|d| d.phase) else { return rsx! {} };
    let (label, frac) = match phase {
        Phase::Installing(p) => (format!("Setting up the downloader (first time)… {}%", (p * 100.0) as u32), Some(p)),
        Phase::Starting => ("Fetching video info…".to_string(), None),
        Phase::Downloading(Some(p)) => (format!("Downloading… {}%", (p * 100.0) as u32), Some(p)),
        Phase::Downloading(None) => ("Downloading…".to_string(), None),
        Phase::Converting(Some(p)) => (format!("Converting to WAV… {}%", (p * 100.0) as u32), Some(p)),
        Phase::Converting(None) => ("Converting to WAV…".to_string(), None),
    };
    rsx! {
        div { class: "download",
            div { class: "download-row",
                span { class: "dim", "{label}" }
                button { onclick: move |_| app.cancel_download(), "Cancel" }
            }
            div { class: if frac.is_none() { "progress indeterminate" } else { "progress" },
                div { style: "width: {frac.unwrap_or(1.0) * 100.0}%" }
            }
        }
    }
}

/// The last download's outcome and yt-dlp's output, under the progress. Kept after the download
/// ends (a failure opens it) until closed.
#[component]
pub(super) fn DownloadLogPanel() -> Element {
    let app = use_context::<state::App>();
    let Some(log) = app.download_log.read().clone() else { return rsx! {} };
    let mut state = app.download_log;
    rsx! {
        div { class: "download-log",
            div { class: "download-row",
                match &log.outcome {
                    Some(Ok(name)) => rsx! { span { class: "ok", title: "{name}", "✓ Downloaded" } },
                    Some(Err(_)) => rsx! { span { class: "bad", "✗ Download failed" } },
                    None => rsx! { span { class: "dim", "Downloader output" } },
                }
                span { class: "spacer" }
                button {
                    onclick: move |_| {
                        if let Some(l) = state.write().as_mut() {
                            l.open = !l.open;
                        }
                    },
                    if log.open { "Hide log" } else { "Show log" }
                }
                if log.outcome.is_some() {
                    button { title: "Close", onclick: move |_| state.set(None), "×" }
                }
            }
            if let Some(Err(e)) = &log.outcome {
                div { class: "download-error", "{e}" }
            }
            if log.open {
                // Reversed in a bottom-anchored column: stays scrolled to the newest line.
                div { class: "log",
                    for (i, line) in log.lines.iter().enumerate().rev() {
                        div { key: "{i}", "{line}" }
                    }
                }
            }
        }
    }
}

/// ⌘U: paste a YouTube link to download its audio.
#[component]
pub(super) fn UrlDialog() -> Element {
    let app = use_context::<state::App>();
    // Remembered while the app runs: the next link is likely the same kind.
    let mut kind = use_signal(crate::download::Kind::default);
    let Some(text) = (app.url_dialog)() else { return rsx! {} };
    let mut dialog = app.url_dialog;
    let close = move || {
        let mut dialog = app.url_dialog;
        dialog.set(None);
        app.focus_root();
    };
    let go = move || {
        // Take the text first: a guard held by the `if let` would still be alive when `close`
        // writes the signal, which panics.
        let url = app.url_dialog.peek().clone();
        if let Some(url) = url {
            close();
            app.start_download(&url, kind());
        }
    };
    let dir = crate::download::downloads_dir();

    rsx! {
        div { class: "modal-backdrop", onclick: move |_| close(),
            div { class: "modal", onclick: move |e| e.stop_propagation(),
                div { class: "modal-head",
                    h2 { "Open from URL" }
                    button { class: "modal-close", title: "Close", onclick: move |_| close(), "×" }
                }
                div { class: "modal-body",
                    div { class: "segmented",
                        for (k, label) in [(crate::download::Kind::Audio, "Audio"), (crate::download::Kind::Video, "Video")] {
                            button {
                                key: "{label}",
                                class: if kind() == k { "on" } else { "" },
                                onclick: move |_| kind.set(k),
                                "{label}"
                            }
                        }
                    }
                    p { class: "dim",
                        match kind() {
                            crate::download::Kind::Audio => "The audio is downloaded, converted to WAV and saved in ",
                            crate::download::Kind::Video => "The video is downloaded as MP4 (H.264, up to 1080p) and saved in ",
                        }
                        "{dir.display()}. "
                        "Works with YouTube and most other video sites. "
                        "The first time, the downloader (yt-dlp, about 75 MB) is set up automatically."
                    }
                    input {
                        class: "url",
                        // Uncontrolled: binding `value` makes fast typing drop characters on re-render.
                        r#type: "url",
                        placeholder: "https://www.youtube.com/watch?v=…",
                        spellcheck: "false",
                        onmounted: move |e| async move {
                            let _ = e.set_focus(true).await;
                        },
                        oninput: move |e| dialog.set(Some(e.value())),
                        onkeydown: move |e| {
                            e.stop_propagation();
                            match e.key() {
                                Key::Escape => close(),
                                Key::Enter => go(),
                                _ => {}
                            }
                        },
                    }
                    div { class: "modal-actions",
                        button { onclick: move |_| close(), "Cancel" }
                        button { class: "primary", disabled: text.trim().is_empty(), onclick: move |_| go(),
                            "Download"
                            kbd { "Enter" }
                        }
                    }
                }
            }
        }
    }
}

/// Paste a tracklist; titles go to the kept tracks in order.
#[component]
pub(super) fn PasteDialog() -> Element {
    let app = use_context::<state::App>();
    let Some(text) = (app.paste)() else { return rsx! {} };
    let titles = tracklist::parse(&text);
    let cutlist = app.cutlist.read();
    let kept = app
        .selected_path()
        .and_then(|p| cutlist.recordings.get(&key_of(&p)))
        .map(|e| e.tracks(u64::MAX).iter().filter(|t| t.exported()).count())
        .unwrap_or(0);
    let mut paste = app.paste;
    let close = move || {
        let mut paste = app.paste;
        paste.set(None);
        app.focus_root();
    };
    let apply = move || {
        if let Some(text) = app.paste.peek().clone() {
            app.apply_tracklist(&text);
        }
        close();
    };
    let preview: Vec<(usize, String)> = titles.iter().take(8).cloned().enumerate().collect();

    rsx! {
        div { class: "modal-backdrop", onclick: move |_| close(),
            div { class: "modal", onclick: move |e| e.stop_propagation(),
                div { class: "modal-head",
                    h2 { "Paste tracklist" }
                    button { class: "modal-close", title: "Close", onclick: move |_| close(), "×" }
                }
                div { class: "modal-body",
                    p { class: "dim",
                        "One title per line. Track numbers, timestamps and durations are removed. "
                        "Titles go to the kept tracks in order."
                    }
                    textarea {
                        value: "{text}",
                        rows: "10",
                        placeholder: "1. Intro\n2. First song (4:12)\n…",
                        onmounted: move |e| async move {
                            let _ = e.set_focus(true).await;
                        },
                        oninput: move |e| paste.set(Some(e.value())),
                        onkeydown: move |e| {
                            e.stop_propagation();
                            let m = e.modifiers();
                            match e.key() {
                                Key::Escape => close(),
                                Key::Enter if m.meta() || m.ctrl() => apply(),
                                _ => {}
                            }
                        },
                    }
                    div { class: if titles.len() == kept { "match ok" } else { "match todo" },
                        "{titles.len()} titles for {kept} kept tracks"
                        if titles.len() > kept { " — the extra titles will be ignored" }
                        if titles.len() < kept && !titles.is_empty() { " — the last tracks keep their titles" }
                    }
                    ol { class: "preview",
                        for (i, t) in preview {
                            li { key: "{i}", "{t}" }
                        }
                        if titles.len() > 8 { li { class: "dim", "…" } }
                    }
                    div { class: "modal-actions",
                        button { onclick: move |_| close(), "Cancel" }
                        button { class: "primary", disabled: titles.is_empty(), onclick: move |_| apply(),
                            "Apply titles"
                            kbd { "⌘Enter" }
                        }
                    }
                }
            }
        }
    }
}
