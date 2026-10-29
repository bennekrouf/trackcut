//! The recordings sidebar, a recording's details and the keyboard shortcuts.

use super::*;

#[component]
pub(super) fn Sidebar() -> Element {
    let app = use_context::<state::App>();
    let recordings = app.recordings.read();
    let selected = (app.selected)();
    let scans = app.scans.read();
    let cutlist = app.cutlist.read();
    let exports = app.exports.read();
    let folder = app.folder.read().as_ref().and_then(|f| f.file_name()).map(|n| n.to_string_lossy().into_owned());
    let active = exports.values().filter(|s| s.is_active()).count();
    let done = recordings
        .iter()
        .filter(|r| cutlist.recordings.get(&key_of(&r.path)).is_some_and(|e| e.status == Status::Done))
        .count();

    // Keep the selected file visible, and show it, and the licence, in the window title.
    use_effect(move || {
        let i = (app.selected)();
        let name = i.and_then(|i| app.recordings.peek().get(i).map(|r| r.name()));
        dioxus::desktop::window().set_title(&window_title(&app.licence.read(), name.as_deref()));
        document::eval(
            "requestAnimationFrame(() => document.querySelector('.item.selected')?.scrollIntoView({block: 'nearest'}))",
        );
    });

    rsx! {
        aside { class: "sidebar",
            div {
                class: "sidebar-edge",
                title: "Drag to resize · double-click to fit the longest name",
                onmousedown: move |e| {
                    e.prevent_default();
                    let mut drag = app.sidebar_drag;
                    drag.set(true);
                },
                ondoubleclick: move |_| app.fit_sidebar(),
            }
            DownloadStatus {}
            DownloadLogPanel {}
            if let Some(name) = folder {
                div { class: "folder", title: "{name}", "{name}" }
            }
            div { class: "queue",
                for (i, rec) in recordings.iter().enumerate() {
                    {
                        let edit = cutlist.recordings.get(&key_of(&rec.path));
                        let status = edit.map(|e| e.status).unwrap_or_default();
                        let (dot, dot_title) = match status {
                            Status::Done | Status::Exported => ("dot done", "done"),
                            Status::InProgress => ("dot active", "in progress"),
                            Status::Flagged => ("dot flagged", "flagged"),
                            Status::Todo => ("dot", "to do"),
                        };
                        let counts = edit.filter(|e| !e.splits.is_empty()).map(|e| e.counts());
                        let export = exports.get(&key_of(&rec.path)).cloned();
                        rsx! {
                            div {
                                key: "{rec.path.display()}",
                                class: if selected == Some(i) { "item selected" } else { "item" },
                                onclick: move |_| app.select(i),
                                span { class: "{dot}", title: "{dot_title}" }
                                span { class: "item-name", title: "{rec.name()}", "{rec.name()}" }
                                {
                                    let path = rec.path.clone();
                                    rsx! {
                                        button {
                                            class: "item-trash",
                                            title: "Move to the Trash…",
                                            onclick: move |e| {
                                                e.stop_propagation();
                                                let mut confirm = app.confirm_delete;
                                                confirm.set(Some(path.clone()));
                                            },
                                            "🗑"
                                        }
                                    }
                                }
                                match (export, scans.get(&rec.path)) {
                                    (Some(ExportStatus::Queued), _) => rsx! { span { class: "item-meta busy", "queued" } },
                                    (Some(ExportStatus::Measuring), _) => rsx! { span { class: "item-meta busy", "measuring…" } },
                                    (Some(ExportStatus::LearningNoise), _) => rsx! { span { class: "item-meta busy", "learning noise…" } },
                                    (Some(ExportStatus::Installing(_)), _) => rsx! { span { class: "item-meta busy", "installing ffmpeg…" } },
                                    (Some(ExportStatus::Running { done, total, .. }), _) => rsx! {
                                        span { class: "item-meta busy", "exporting {done}/{total}" }
                                    },
                                    (Some(ExportStatus::Failed(e)), _) => rsx! { span { class: "item-meta bad", title: "{e}", "export failed" } },
                                    (_, Some(ScanState::Scanning(p))) => rsx! { span { class: "item-meta busy", "{(p * 100.0) as u32}%" } },
                                    (_, Some(ScanState::Failed(e))) => rsx! { span { class: "item-meta bad", title: "{e}", "error" } },
                                    (_, Some(ScanState::Ready(s))) => rsx! {
                                        if let Some((ok, todo)) = counts {
                                            span {
                                                class: if todo == 0 { "item-meta ok" } else { "item-meta todo" },
                                                title: "{ok + todo + 1} tracks: {ok} splits confirmed, {todo} to review",
                                                if todo == 0 { "{ok + 1} tracks ✓" } else { "{todo} to review" }
                                            }
                                        } else {
                                            span { class: "item-meta", "{fmt_short(s.info.duration_secs())}" }
                                        }
                                    },
                                    (_, None) => rsx! { span { class: "item-meta", "…" } },
                                }
                            }
                        }
                    }
                }
            }
            if recordings.is_empty() {
                div { class: "hint", "Open a folder containing MP3 or WAV recordings, or MP4/MOV videos (⌘O), or a YouTube link (⌘U)." }
            }
            div { class: "sidebar-foot",
                if active > 0 {
                    span { class: "dim", "Exporting {active} file(s)…" }
                    button { onclick: move |_| app.cancel_exports(), "Cancel" }
                } else {
                    button {
                        class: "primary",
                        disabled: done == 0,
                        title: "Export every file marked done (⌘Enter marks one)",
                        onclick: move |_| app.export_done(),
                        "Export {done} done file(s)"
                        kbd { "⇧⌘E" }
                    }
                }
            }
        }
    }
}

/// The recording's name. ✎ (or R) renames the file, and with it the export folder.
#[component]
pub(super) fn FileName(path: std::path::PathBuf) -> Element {
    let app = use_context::<state::App>();
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let mut draft = use_signal(|| stem.clone());
    let mut renaming = app.renaming;
    let original = use_signal(|| stem.clone());
    let mut close = move |save: bool| {
        if !*renaming.peek() {
            return;
        }
        renaming.set(false);
        let name = draft.peek().clone();
        // Start from the current name next time (after a rename this component is remounted).
        draft.set(original.peek().clone());
        if save && !name.trim().is_empty() {
            app.rename_recording(&name);
        }
        app.focus_root();
    };
    if !renaming() {
        return rsx! {
            div { class: "file-name",
                h1 { class: "title", "{stem}" span { class: "dim", "{ext}" } }
                button {
                    class: "rename",
                    title: "Rename the file and its export folder (R)",
                    onclick: move |_| renaming.set(true),
                    "✎"
                }
                span { class: "spacer" }
                ApplyCuts {}
            }
        };
    }
    rsx! {
        div { class: "file-name",
            input {
                class: "title-input name-input",
                value: "{draft}",
                onmounted: move |e| async move {
                    let _ = e.set_focus(true).await;
                },
                oninput: move |e| draft.set(e.value()),
                onkeydown: move |e| {
                    e.stop_propagation();
                    match e.key() {
                        Key::Enter => close(true),
                        Key::Escape => close(false),
                        _ => {}
                    }
                },
                onblur: move |_| close(true),
            }
            span { class: "dim", "{ext}" }
            span { class: "dim hint", "Enter renames the file and its export folder · Esc cancels" }
        }
    }
}

#[component]
pub(super) fn Info(scan: ScanRef) -> Element {
    let i = &scan.0.info;
    let bitrate = match &i.bitrate {
        Bitrate::Cbr(k) => format!("CBR {k} kbps"),
        Bitrate::Vbr { min, avg, max } => format!("VBR {min}–{max} kbps (avg {avg})"),
        Bitrate::Pcm { kbps, .. } => format!("{kbps} kbps"),
        Bitrate::Avg(k) => format!("~{k} kbps"),
        Bitrate::Unknown => "bitrate unknown".into(),
        Bitrate::None => String::new(),
    };
    let channels = match i.channels {
        1 => "mono".to_string(),
        2 => "stereo".to_string(),
        n => format!("{n} ch"),
    };
    let size = format!("{:.1} MB", i.file_size as f64 / 1_048_576.0);
    // A video without sound has no audio format to describe.
    let chips = match i.silent {
        true => vec![i.codec.clone(), fmt_short(i.duration_secs()), size],
        false => vec![
            i.codec.clone(),
            bitrate,
            format!("{:.1} kHz", i.sample_rate as f64 / 1000.0),
            channels,
            fmt_short(i.duration_secs()),
            size,
        ],
    };
    rsx! {
        div { class: "info",
            for chip in chips {
                span { class: "chip", "{chip}" }
            }
        }
    }
}

#[component]
pub(super) fn Keys() -> Element {
    let review = [
        ("Tab ⇧Tab", "next / prev split"),
        ("Enter", "keep + next"),
        ("⌫", "delete + next"),
        (", .", "nudge 10 ms (⇧ 100 ms)"),
        ("S", "snap to silence"),
        ("M", "add split"),
        ("C", "listen again"),
        ("⌘Z", "undo"),
        ("⌘Enter", "file done"),
    ];
    let tracks = [
        ("T", "title"),
        ("R", "rename file"),
        ("P", "preview cuts"),
        ("X", "cut / keep this part"),
        ("G", "cut / keep silence"),
        ("A", "A/B compare"),
        ("F", "flag file"),
        ("⌘E", "export"),
        ("⇧⌘E", "export done files"),
    ];
    let nav = [
        ("Space", "play / pause"),
        ("← →", "±5 s (⇧ 0.5 s, ⌥ 10 ms)"),
        ("↑ ↓", "prev / next file"),
        ("+ −", "zoom"),
        ("drag marker", "move split"),
    ];
    rsx! {
        div { class: "keys",
            for (k, what) in review {
                span { kbd { "{k}" } " {what}" }
            }
        }
        div { class: "keys",
            for (k, what) in tracks.into_iter().chain(nav) {
                span { kbd { "{k}" } " {what}" }
            }
        }
    }
}
