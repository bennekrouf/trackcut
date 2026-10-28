use crate::ab::AbState;
use crate::download::Phase;
use crate::exporting::source_facts;
use crate::exporting::ExportStatus;
use crate::review;
use crate::state::{self, key_of, ScanState};
use desktop_kit::ui::{use_theme, Banners, Header, ProButton, ProWindow};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use std::fmt::Write;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use trackcut_audio::{Bitrate, Scan};
use trackcut_core::edit::SplitState;
use trackcut_core::export::{plan, Normalize, Profile, VideoProfile};
use trackcut_core::license::StatusExt;
use trackcut_core::loudness::Loudness;
use trackcut_core::time::{fmt_precise, fmt_short};
use trackcut_core::tracklist;
use trackcut_core::Status;
mod dialogs;
use dialogs::*;
mod waveform;
use waveform::*;
mod export;
use export::*;
mod sidebar;
use sidebar::*;

const CSS: &str = include_str!("../assets/style.css");
/// The native title bar, as in every mayorana app: name, version and licence,
/// then the selected recording after a dash.
pub fn window_title(licence: &desktop_kit::licence::Status, recording: Option<&str>) -> String {
    let mut title = format!("TrackCut {}{}", env!("CARGO_PKG_VERSION"), licence.title_badge());
    if let Some(name) = recording {
        title.push_str(" \u{2014} ");
        title.push_str(name);
    }
    title
}
/// Waveform resolution; the SVG is stretched to the element width.
const COLS: usize = 1600;
/// How close (in pixels) a click must be to grab a split marker.
const GRAB_PX: f64 = 6.0;

#[component]
pub fn App() -> Element {
    let app = use_context_provider(state::App::new);
    let theme = use_theme();

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

    let sidebar_width = (app.sidebar_width)();
    let sidebar_drag = app.sidebar_drag;

    let onkeydown = move |e: KeyboardEvent| {
        if app.paste.peek().is_some()
            || app.editing_title.peek().is_some()
            || app.url_dialog.peek().is_some()
            || *app.licence_dialog.peek()
            || *app.mcp_dialog.peek()
            || *app.renaming.peek()
            || app.confirm_delete.peek().is_some()
        {
            return; // a text field has the keyboard
        }
        let m = e.modifiers();
        let cmd = m.meta() || m.ctrl();
        let step = if m.alt() {
            0.01
        } else if m.shift() {
            0.5
        } else {
            5.0
        };
        let nudge = if m.shift() { 0.1 } else { 0.01 };
        let handled = match e.code() {
            // Playback and navigation
            Code::Space => {
                app.toggle();
                true
            }
            Code::ArrowLeft => {
                app.nudge(-step);
                true
            }
            Code::ArrowRight => {
                app.nudge(step);
                true
            }
            Code::ArrowUp => {
                app.select_offset(-1);
                true
            }
            Code::ArrowDown => {
                app.select_offset(1);
                true
            }
            Code::Home => {
                app.seek(0, true);
                true
            }
            Code::End => {
                app.seek(u64::MAX, true);
                true
            }
            Code::Equal | Code::NumpadAdd => {
                app.zoom(0.5);
                true
            }
            Code::Minus | Code::NumpadSubtract => {
                app.zoom(2.0);
                true
            }
            Code::KeyO if cmd => {
                open_folder(app);
                true
            }
            Code::KeyU if cmd => {
                app.ask_url();
                true
            }
            // Split review
            Code::Tab => {
                app.step_split(!m.shift());
                true
            }
            Code::Enter | Code::NumpadEnter if cmd => {
                app.finish_recording();
                true
            }
            Code::Enter | Code::NumpadEnter => {
                app.confirm_and_next();
                true
            }
            Code::Backspace | Code::Delete => {
                app.delete_split();
                true
            }
            Code::Comma => {
                app.nudge_split(-nudge);
                true
            }
            Code::Period => {
                app.nudge_split(nudge);
                true
            }
            Code::KeyS if !cmd => {
                app.snap_split();
                true
            }
            Code::KeyM if !cmd => {
                app.add_split_at_playhead();
                true
            }
            Code::KeyC if !cmd => {
                app.replay_split();
                true
            }
            Code::KeyZ if cmd && m.shift() => {
                app.redo();
                true
            }
            Code::KeyZ if cmd => {
                app.undo();
                true
            }
            // Tracks
            Code::KeyT if !cmd => {
                app.edit_title(None);
                true
            }
            Code::KeyX if !cmd => {
                app.toggle_drop(None);
                true
            }
            Code::KeyP if !cmd => {
                app.toggle_preview();
                true
            }
            Code::KeyR if !cmd => {
                let mut renaming = app.renaming;
                renaming.set(true);
                true
            }
            Code::KeyG if !cmd => {
                app.toggle_silence();
                true
            }
            Code::KeyE if cmd && m.shift() => {
                app.export_done();
                true
            }
            Code::KeyE if cmd => {
                app.export_current();
                true
            }
            Code::KeyF if !cmd => {
                app.toggle_flag();
                true
            }
            Code::KeyA if !cmd => {
                app.ab_toggle();
                true
            }
            Code::Escape => {
                if *app.ab.peek() != AbState::Off {
                    app.ab_stop();
                } else {
                    let (mut cur, mut closed) = (app.cur_split, app.closed_track);
                    cur.set(None);
                    closed.set(None);
                }
                true
            }
            _ => false,
        };
        if handled {
            e.prevent_default();
        }
    };

    rsx! {
        style { {CSS} }
        Header {
            theme,
            on_mcp: move |_| {
                let mut dialog = app.mcp_dialog;
                dialog.set(true);
            },
            actions: rsx! {
                button { class: "kit-btn", title: "Open a folder of recordings (⌘O)", onclick: move |_| open_folder(app), "Open folder…" }
                button {
                    class: "kit-btn",
                    title: "Download a YouTube video's audio and open it (⌘U)",
                    disabled: app.download.read().is_some(),
                    onclick: move |_| app.ask_url(),
                    "URL…"
                }
            },
            pro: rsx! { ProButton { pro: app.pro(), hint: crate::pro::hint() } },
        }
        div {
            class: if sidebar_drag() { "app resizing" } else { "app" },
            style: "grid-template-columns: {sidebar_width}px 1fr",
            tabindex: "0",
            onkeydown,
            // Dragging the file list's edge: follow the mouse anywhere in the window.
            onmousemove: move |e| {
                if sidebar_drag() {
                    let held = e.held_buttons().contains(MouseButton::Primary);
                    app.set_sidebar_width(e.client_coordinates().x, !held);
                }
            },
            onmouseup: move |e| {
                if sidebar_drag() {
                    app.set_sidebar_width(e.client_coordinates().x, true);
                }
            },
            onmounted: move |e| {
                let mut root = app.root;
                root.set(Some(e.data()));
                app.focus_root();
            },
            Sidebar {}
            main { class: "main", Editor {} }
            UrlDialog {}
            DeleteDialog {}
            ProWindow {
                pro: app.pro(),
                pitch: crate::pro::pitch(),
                on_close: move |_| app.focus_root(),
            }
            crate::mcp_panel::McpDialog {}
        }
        Banners { statistics: true }
    }
}

fn open_folder(app: state::App) {
    spawn(async move {
        if let Some(dir) = rfd::AsyncFileDialog::new().set_title("Open a folder of recordings").pick_folder().await {
            app.open(dir.path());
        }
        app.focus_root();
    });
}

/// Identity-compared handle so waveform components only re-render when the scan changes.
#[derive(Clone)]
struct ScanRef(Arc<Scan>);

impl PartialEq for ScanRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[component]
fn Editor() -> Element {
    let app = use_context::<state::App>();
    let recordings = app.recordings.read();
    let Some(i) = (app.selected)() else {
        return rsx! {
            div { class: "empty", "No recording selected." }
            ErrorBanner {}
        };
    };
    let Some(rec) = recordings.get(i) else { return rsx! {} };
    let scans = app.scans.read();
    let body = match scans.get(&rec.path) {
        Some(ScanState::Ready(scan)) => {
            let scan = ScanRef(scan.clone());
            // A file is split as a video or as audio, never both: a video shows its picture and
            // a timeline without the waveform, and exports video clips.
            let video = trackcut_core::is_video(&rec.path).then(|| rec.path.clone());
            rsx! {
                Info { scan: scan.clone() }
                // A video shows its picture where a recording shows its overview waveform.
                if let Some(video) = video {
                    crate::video::VideoPreview {
                        key: "{video.display()}",
                        path: video,
                        rate: scan.0.info.sample_rate,
                    }
                } else {
                    Overview { scan: scan.clone() }
                }
                Detail { scan: scan.clone() }
                Transport { scan: scan.clone() }
                ReviewBar { scan: scan.clone() }
                ExportBar { scan: scan.clone() }
                TrackList { scan }
                PasteDialog {}
            }
        }
        Some(ScanState::Failed(e)) => rsx! { div { class: "empty bad", "Could not read this file: {e}" } },
        Some(ScanState::Scanning(p)) => rsx! {
            div { class: "empty",
                "Analysing… {(p * 100.0) as u32}%"
                div { class: "progress", div { style: "width: {p * 100.0}%" } }
            }
        },
        None => rsx! { div { class: "empty", "Waiting to analyse…" } },
    };

    rsx! {
        FileName { key: "{rec.name()}", path: rec.path.clone() }
        {body}
        ErrorBanner {}
        Keys {}
    }
}

/// Write a copy without the cut parts; it opens as a new entry and the original stays.
#[component]
fn ApplyCuts() -> Element {
    let app = use_context::<state::App>();
    // Subscribe to what `can_apply_cuts` peeks at.
    let _ = (app.cutlist.read(), app.scans.read(), app.selected.read());
    if let Some(a) = app.applying.read().as_ref() {
        let pct = (a.progress * 100.0) as u32;
        return rsx! {
            span { class: "applying dim", "Applying cuts… {pct}%" }
            div { class: "progress small", div { style: "width: {pct}%" } }
        };
    }
    let can = app.can_apply_cuts();
    rsx! {
        button {
            class: "primary",
            disabled: !can,
            title: if can {
                "Save a copy without the cut silence and left-out tracks, and open it. The original stays; ⌘Z right after removes the copy."
            } else {
                "Nothing is cut in this recording"
            },
            onclick: move |_| app.apply_cuts(),
            "✂ Apply cuts"
        }
    }
}

#[component]
fn ErrorBanner() -> Element {
    let app = use_context::<state::App>();
    let Some(err) = app.error.read().clone() else { return rsx! {} };
    rsx! {
        div {
            class: "error",
            onclick: move |_| {
                let mut e = app.error;
                e.set(None);
            },
            "{err}"
        }
    }
}

fn wave_path(cols: &[(f32, f32)]) -> String {
    let mut d = String::with_capacity(cols.len() * 24);
    for (i, &(_, hi)) in cols.iter().enumerate() {
        let _ = write!(d, "{}{i},{:.2}", if i == 0 { 'M' } else { 'L' }, 50.0 - hi * 49.0 - 0.3);
    }
    for (i, &(lo, _)) in cols.iter().enumerate().rev() {
        let _ = write!(d, "L{i},{:.2}", 50.0 - lo * 49.0 + 0.3);
    }
    d.push('Z');
    d
}

#[component]
fn Wave(scan: ScanRef, start: u64, end: u64) -> Element {
    let d = wave_path(&scan.0.peaks.columns(start, end, COLS));
    rsx! {
        svg {
            class: "wave",
            view_box: "0 0 {COLS} 100",
            preserve_aspect_ratio: "none",
            line { x1: "0", y1: "50", x2: "{COLS}", y2: "50", class: "axis" }
            path { d: "{d}" }
        }
    }
}

/// Only this component reads `pos`, so the 60 fps playhead updates touch nothing else.
#[component]
fn Playhead(start: u64, end: u64) -> Element {
    let app = use_context::<state::App>();
    let pos = (app.pos)();
    if end <= start || pos < start || pos > end {
        return rsx! {};
    }
    let pct = (pos - start) as f64 / (end - start) as f64 * 100.0;
    rsx! { div { class: "playhead", style: "left: {pct}%" } }
}

/// Tick spacing that gives at most ~10 labels across `span` seconds.
fn tick_step(span: f64) -> f64 {
    const STEPS: [f64; 13] = [0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0];
    STEPS.into_iter().find(|s| span / s <= 10.0).unwrap_or(1200.0)
}

/// The track X and T act on (a memo, so playback only re-renders when it changes).
fn use_focus_track() -> Memo<Option<usize>> {
    let app = use_context::<state::App>();
    use_memo(move || {
        let (pos, cur, closed) = ((app.pos)(), (app.cur_split)(), (app.closed_track)());
        let _ = app.selected.read();
        let cutlist = app.cutlist.read();
        app.selected_path()
            .and_then(|p| cutlist.recordings.get(&key_of(&p)).map(|e| review::track_in_focus(e, cur, closed, pos)))
    })
}

fn fmt_bytes(b: u64) -> String {
    match b {
        b if b >= 1_000_000_000 => format!("{:.1} GB", b as f64 / 1e9),
        b if b >= 1_000_000 => format!("{:.0} MB", b as f64 / 1e6),
        b => format!("{:.0} kB", b as f64 / 1e3),
    }
}

/// The compared window while A/B is on.
#[component]
fn AbBand(start: u64, end: u64) -> Element {
    let app = use_context::<state::App>();
    let (a, b, ready) = match (app.ab)() {
        AbState::Preparing { start, end, .. } => (start, end, false),
        AbState::Ready { start, end, .. } => (start, end, true),
        AbState::Off => return rsx! {},
    };
    if end <= start || b <= start || a >= end {
        return rsx! {};
    }
    let span = (end - start) as f64;
    let left = (a.max(start) - start) as f64 / span * 100.0;
    let width = (b.min(end) - a.max(start)) as f64 / span * 100.0;
    rsx! {
        div { class: if ready { "ab-window" } else { "ab-window preparing" }, style: "left: {left}%; width: {width}%" }
    }
}

/// Which side of the A/B is playing.
#[component]
fn AbStatus() -> Element {
    let app = use_context::<state::App>();
    match (app.ab)() {
        AbState::Off => rsx! {},
        AbState::Preparing { profile, .. } => rsx! {
            span { class: "ab-status", "Encoding a {profile.short()} preview…" }
        },
        AbState::Ready { profile, encoded, .. } => rsx! {
            span { class: "ab-status",
                span { class: if encoded { "ab-side" } else { "ab-side on" }, "A · Original" }
                span { class: if encoded { "ab-side on enc" } else { "ab-side" }, "B · {profile.short()}" }
                span { class: "dim", " A switch · Esc stop" }
            }
        },
    }
}

#[cfg(test)]
mod tests;
