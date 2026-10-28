//! The dialogs: the licence, deleting a recording, downloading from a link, pasting a track list.

use super::*;

/// "Move to the Trash?" for a recording in the file list. Enter confirms, Esc cancels.
#[component]
pub(super) fn LicenceDialog() -> Element {
    use crate::licence::{self, Status};
    let app = use_context::<state::App>();
    let mut problem = use_signal(|| Option::<String>::None);
    let mut busy = use_signal(|| false);
    let free_email = use_signal(String::new);
    let free_note = use_signal(|| Option::<String>::None);
    let free_hidden = use_signal(|| false);
    // Set by an Activate that made TrackCut Pro, so the window congratulates rather than
    // just reporting it.
    let activated = use_signal(|| false);
    // Escape closes the window wherever the focus is: the Pro view has no field to listen on.
    use_future(move || async move {
        let (mut dialog, mut activated) = (app.licence_dialog, activated);
        let mut keys = document::eval(
            "if (window._proEscape) window.removeEventListener('keydown', window._proEscape);\
             window._proEscape = (e) => { if (e.key === 'Escape') dioxus.send(true); };\
             window.addEventListener('keydown', window._proEscape);",
        );
        while keys.recv::<bool>().await.is_ok() {
            if dialog.peek().is_some() {
                dialog.set(None);
                activated.set(false);
                app.focus_root();
            }
        }
    });
    let Some(text) = (app.licence_dialog)() else { return rsx! {} };
    let status = app.licence.read().clone();
    let asks_number = matches!(status, Status::Free | Status::Unknown(_)) || status.free_credits().is_some();
    let offers_free = matches!(status, Status::Free | Status::Unknown(_)) && !free_hidden();
    let mut dialog = app.licence_dialog;
    let close = move || {
        let mut dialog = app.licence_dialog;
        dialog.set(None);
        let (mut problem, mut activated) = (problem, activated);
        problem.set(None);
        activated.set(false);
        app.focus_root();
    };
    let activate = move || {
        let typed = app.licence_dialog.peek().clone().unwrap_or_default();
        if typed.trim().is_empty() || *busy.peek() {
            return;
        }
        let mut busy = busy;
        busy.set(true);
        spawn(async move {
            let mut problem = problem;
            match licence::activate_async(typed).await {
                Ok(s) => {
                    let mut activated = activated;
                    activated.set(matches!(s, Status::Pro(_)));
                    let mut licence = app.licence;
                    licence.set(s);
                    let mut dialog = app.licence_dialog;
                    dialog.set(Some(String::new()));
                    problem.set(None);
                }
                Err(e) => problem.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let check_again = move |_| {
        busy.set(true);
        spawn(async move {
            let mut licence = app.licence;
            licence.set(licence::refresh_async(true).await);
            busy.set(false);
        });
    };
    let ask_free = move || {
        let email = free_email.peek().clone();
        if email.trim().is_empty() || *busy.peek() {
            return;
        }
        let mut busy = busy;
        busy.set(true);
        spawn(async move {
            let (mut problem, mut free_note, mut free_hidden) = (problem, free_note, free_hidden);
            match licence::request_free_credits_async(email).await {
                Ok(message) => {
                    free_note.set(Some(format!("{message} Paste the licence number above when it arrives.")));
                    problem.set(None);
                }
                Err(trackcut_core::license::FreeError::Unavailable) => free_hidden.set(true),
                Err(trackcut_core::license::FreeError::Refused(e)) => problem.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let manage = move |number: String| {
        let mut busy = busy;
        busy.set(true);
        spawn(async move {
            let mut problem = problem;
            match licence::portal_url_async(number).await {
                Ok(url) => licence::open_url(&url),
                Err(e) => problem.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let remove = move |_| {
        licence::deactivate();
        let mut licence = app.licence;
        licence.set(Status::Free);
    };

    if let Status::Pro(a) = &status {
        let number = a.licence.clone();
        return rsx! {
            div { class: "modal-backdrop", onclick: move |_| close(),
                div { class: "modal pro-done", onclick: move |e| e.stop_propagation(),
                    button { class: "modal-close", title: "Close", onclick: move |_| close(), "×" }
                    div { class: "pro-done-mark", "✓" }
                    h2 { class: "pro-done-title",
                        if activated() { "Congratulations, you're Pro!" } else { "You're Pro" }
                    }
                    p { class: "pro-done-sub", "Licence validated." }
                    p { class: "pro-done-meta",
                        "{a.credits.left} AI credits left this month"
                        br {}
                        "{a.renewal()}"
                    }
                    button {
                        class: "pro-done-manage",
                        disabled: busy(),
                        title: "Change or cancel the plan on mayorana.ch",
                        onclick: move |_| manage(number.clone()),
                        "Manage subscription"
                    }
                    if let Some(p) = problem() {
                        p { class: "pro-done-error", "{p}" }
                    }
                }
            }
        };
    }

    rsx! {
        div { class: "modal-backdrop", onclick: move |_| close(),
            div { class: "modal", onclick: move |e| e.stop_propagation(),
                div { class: "modal-head",
                    h2 { "TrackCut Pro" }
                    button { class: "modal-close", title: "Close", onclick: move |_| close(), "×" }
                }
                div { class: "modal-body",
                    match &status {
                        Status::Pro(a) => rsx! {
                            p { strong { "{a.plan.name}" } " plan for " strong { "{a.email}" } "." }
                            p { class: "dim", "{a.credits.left} AI credits left this month. {a.renewal()}." }
                        },
                        Status::NotPro(a) if a.is_free() => rsx! {
                            p { "Free — " strong { "{a.credits.left} AI credits" } " left this month, for " strong { "{a.email}" } ". {a.renewal()}." }
                            p { class: "dim",
                                "The free version exports the first {trackcut_core::license::FREE_TRACKS} tracks of each recording. "
                                "TrackCut Pro exports all of them; it comes with a monthly plan on mayorana.ch."
                            }
                        },
                        Status::NotPro(a) if !a.active => rsx! {
                            p { "The subscription for " strong { "{a.email}" } " isn't active." }
                            p { class: "match todo", "TrackCut runs as the free version. Renew it to have Pro again." }
                        },
                        Status::NotPro(a) => rsx! {
                            p { "The " strong { "{a.plan.name}" } " plan for " strong { "{a.email}" } " doesn't include TrackCut." }
                            p { class: "match todo", "TrackCut runs as the free version. A plan with TrackCut makes it Pro." }
                        },
                        Status::Offline(a) => rsx! {
                            p { strong { "{a.plan.name}" } " plan for " strong { "{a.email}" } "." }
                            p { class: "match todo",
                                "TrackCut hasn't been able to check the licence for a week, so it runs as the free version "
                                "until it can. Check the connection, then check again."
                            }
                        },
                        Status::Unknown(_) => rsx! {
                            p { class: "match todo", "The licence number on this computer isn't known. Check it against the email, or paste it again." }
                        },
                        Status::Free => rsx! {
                            p {
                                "The free version exports the first {trackcut_core::license::FREE_TRACKS} tracks of each recording. "
                                "TrackCut Pro exports all of them. It comes with a monthly plan on mayorana.ch."
                            }
                            p { class: "dim",
                                "Paste the licence number from the email you received when you subscribed. "
                                "It's checked with api0 now and once a day; Pro keeps working for a week without a connection."
                            }
                        },
                    }
                    if asks_number {
                        input {
                            class: "licence-key",
                            value: "{text}",
                            spellcheck: "false",
                            placeholder: "ABCDE-FGHJK-LMNPQ-RSTUV",
                            onmounted: move |e| async move {
                                let _ = e.set_focus(true).await;
                            },
                            oninput: move |e| {
                                problem.set(None);
                                dialog.set(Some(e.value()));
                            },
                            onkeydown: move |e| {
                                e.stop_propagation();
                                match e.key() {
                                    Key::Escape => close(),
                                    Key::Enter => {
                                        e.prevent_default();
                                        activate();
                                    }
                                    _ => {}
                                }
                            },
                        }
                    }
                    if offers_free {
                        div { class: "free-credits",
                            p { class: "dim", "No plan? Get free AI credits every month, without Pro. The licence number comes by email." }
                            div { class: "free-row",
                                input {
                                    class: "licence-key",
                                    r#type: "email",
                                    spellcheck: "false",
                                    placeholder: "you@example.com",
                                    value: "{free_email}",
                                    oninput: move |e| {
                                        let mut free_email = free_email;
                                        free_email.set(e.value());
                                    },
                                    onkeydown: move |e: KeyboardEvent| {
                                        e.stop_propagation();
                                        match e.key() {
                                            Key::Escape => close(),
                                            Key::Enter => {
                                                e.prevent_default();
                                                ask_free();
                                            }
                                            _ => {}
                                        }
                                    },
                                }
                                button { disabled: free_email.read().trim().is_empty() || busy(), onclick: move |_| ask_free(), "Get free AI credits" }
                            }
                            if let Some(n) = free_note() {
                                p { class: "dim", "{n}" }
                            }
                        }
                    }
                    if let Some(p) = problem() {
                        p { class: "match todo", "{p}" }
                    }
                    div { class: "modal-actions",
                        if status.licence().is_some() {
                            button { class: "left", title: "Removes the licence from every Mayorana app on this computer, e.g. to use it on another one", onclick: remove, "Remove from this computer" }
                        }
                        if let (Some(number), false) = (status.licence().map(str::to_owned), asks_number) {
                            button { disabled: busy(), title: "Change or cancel the plan on mayorana.ch", onclick: move |_| manage(number.clone()),
                                "Manage subscription…"
                            }
                            button { disabled: busy(), onclick: check_again, "Check again" }
                        }
                        if !matches!(status, Status::Pro(_)) {
                            a { class: "button-link", href: "{licence::BUY_URL}", target: "_blank",
                                if status.licence().is_some() && status.free_credits().is_none() { "Renew…" } else { "Buy TrackCut Pro…" }
                            }
                        }
                        button { onclick: move |_| close(), if asks_number { "Cancel" } else { "Close" } }
                        if asks_number {
                            button { class: "primary", disabled: text.trim().is_empty() || busy(), onclick: move |_| activate(),
                                if busy() {
                                    span { class: "pro-spinner", title: "Checking the licence" }
                                } else {
                                    "Activate"
                                    kbd { "Enter" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

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
