//! The waveform: the overview and detail views, their markers and labels, and playing it.

use super::*;

/// Split markers between `start` and `end`. In the detail view they carry the number of the
/// track that starts there.
#[component]
pub(super) fn Markers(start: u64, end: u64, detail: bool) -> Element {
    let app = use_context::<state::App>();
    let cutlist = app.cutlist.read();
    let cur = (app.cur_split)();
    let Some(key) = app.selected_path().map(|p| key_of(&p)) else { return rsx! {} };
    let Some(edit) = cutlist.recordings.get(&key) else { return rsx! {} };
    let _ = app.selected.read(); // re-render on file change
    if end <= start {
        return rsx! {};
    }
    let span = (end - start) as f64;
    rsx! {
        for (i, s) in edit.splits.iter().enumerate().filter(|(_, s)| s.at >= start && s.at <= end) {
            {
                let pct = (s.at - start) as f64 / span * 100.0;
                let mut class = String::from("marker");
                class.push_str(if s.state == SplitState::Confirmed { " confirmed" } else { " suggested" });
                if cur == Some(i) {
                    class.push_str(" current");
                }
                rsx! {
                    div { key: "{i}", class: "{class}", style: "left: {pct}%" }
                }
            }
        }
    }
}

/// Each track's number and name where it starts (or at the left edge if it started earlier),
/// coloured like its split: the titles typed in the list show up on the waveform.
#[component]
pub(super) fn TrackLabels(total: u64, start: u64, end: u64) -> Element {
    let app = use_context::<state::App>();
    let cutlist = app.cutlist.read();
    let _ = app.selected.read();
    let Some(edit) = app.selected_path().and_then(|p| cutlist.recordings.get(&key_of(&p))) else { return rsx! {} };
    if end <= start {
        return rsx! {};
    }
    let span = (end - start) as f64;
    let names = edit.names(total);
    let labels: Vec<(usize, &str, String, f64, f64)> = edit
        .tracks(total)
        .iter()
        .filter(|t| t.end > start && t.start < end)
        .map(|t| {
            let class = match t.index.checked_sub(1).and_then(|i| edit.splits.get(i)).map(|s| s.state) {
                None => "track-label start",
                Some(SplitState::Confirmed) => "track-label confirmed",
                Some(SplitState::Suggested) => "track-label suggested",
            };
            let text = match &names[t.index] {
                Some(name) => format!("{} · {name}", t.index + 1),
                None => format!("{}", t.index + 1),
            };
            let a = t.start.max(start) - start;
            let b = t.end.min(end) - start;
            (t.index, class, text, a as f64 / span * 100.0, (b - a) as f64 / span * 100.0)
        })
        .collect();
    rsx! {
        for (k, class, text, left, width) in labels {
            div { key: "{k}", class: "track-label-slot", style: "left: {left}%; width: {width}%",
                span { class: "{class}", title: "{text}", "{text}" }
            }
        }
    }
}

/// Transparent layer over a waveform: click/drag to seek, grab split markers to move them,
/// scroll to pan, ⌘-scroll to zoom. Positions come from client coordinates and the layer's
/// own rect, so markers under the cursor don't change the reference element.
#[component]
pub(super) fn Interact(start: u64, end: u64, center: bool, edit_splits: bool) -> Element {
    let app = use_context::<state::App>();
    let mut rect = use_signal(|| (0.0f64, 1.0f64));
    let mut mounted = use_signal(|| None::<Rc<MountedData>>);
    let mut hover = use_signal(|| false);

    let refresh = move || {
        if let Some(m) = mounted.peek().clone() {
            spawn(async move {
                if let Ok(r) = m.get_client_rect().await {
                    rect.set((r.origin.x, r.size.width));
                }
            });
        }
    };
    let to_frame = move |cx: f64| {
        let (left, width) = *rect.peek();
        start + (((cx - left) / width.max(1.0)).clamp(0.0, 1.0) * (end - start) as f64) as u64
    };
    let grab = move |cx: f64| {
        if !edit_splits || end <= start {
            return None;
        }
        let frames_per_px = (end - start) as f64 / rect.peek().1.max(1.0);
        app.split_near(to_frame(cx), (GRAB_PX * frames_per_px) as u64)
    };

    rsx! {
        div {
            class: if hover() { "seek-layer grab" } else { "seek-layer" },
            onmounted: move |e| {
                mounted.set(Some(e.data()));
                refresh();
            },
            onresize: move |_| refresh(),
            onmousedown: move |e| {
                let x = e.client_coordinates().x;
                match grab(x) {
                    Some(i) => app.begin_drag(i),
                    None => app.seek(to_frame(x), center),
                }
            },
            onmousemove: move |e| {
                let x = e.client_coordinates().x;
                let held = e.held_buttons().contains(MouseButton::Primary);
                if app.dragging() {
                    if held {
                        app.drag_to(to_frame(x));
                    } else {
                        app.end_drag();
                    }
                } else if held {
                    app.seek(to_frame(x), false);
                } else {
                    let h = grab(x).is_some();
                    if h != *hover.peek() {
                        hover.set(h);
                    }
                }
            },
            onmouseup: move |_| app.end_drag(),
            onmouseleave: move |_| {
                app.end_drag();
                hover.set(false);
            },
            onwheel: move |e| {
                let d = e.delta().strip_units();
                let span = (end - start) as f64;
                if e.modifiers().meta() || e.modifiers().ctrl() {
                    app.zoom(if d.y > 0.0 { 1.25 } else { 0.8 });
                } else {
                    let dx = if d.x.abs() > d.y.abs() { d.x } else { d.y };
                    app.scroll_view((dx / rect.peek().1.max(1.0) * span) as i64);
                }
            },
        }
    }
}

#[component]
pub(super) fn Overview(scan: ScanRef) -> Element {
    let app = use_context::<state::App>();
    let info = &scan.0.info;
    let total = info.total_samples.max(1);
    let v = (app.view)();
    let span = v.span_secs * info.sample_rate as f64;
    let left = v.start as f64 / total as f64 * 100.0;
    let width = (span / total as f64 * 100.0).min(100.0 - left);
    rsx! {
        div { class: "overview",
            Wave { scan: scan.clone(), start: 0, end: total }
            Silences { total, start: 0, end: total }
            Parts { total, start: 0, end: total, labels: false }
            AbBand { start: 0, end: total }
            Markers { start: 0, end: total, detail: false }
            div { class: "window", style: "left: {left}%; width: {width}%" }
            Playhead { start: 0, end: total }
            Interact { start: 0, end: total, center: true, edit_splits: false }
        }
    }
}

#[component]
pub(super) fn Detail(scan: ScanRef) -> Element {
    let app = use_context::<state::App>();
    // A video is split on a plain timeline: its picture is on top, and no waveform is drawn.
    let waveform = !app.selected_path().is_some_and(|p| trackcut_core::is_video(&p));
    let rate = scan.0.info.sample_rate as f64;
    let v = (app.view)();
    let start = v.start;
    let end = start + (v.span_secs * rate) as u64;

    let step = tick_step(v.span_secs);
    let t0 = start as f64 / rate;
    let first = (t0 / step).ceil() as i64;
    let ticks: Vec<(f64, String)> = (first..)
        .map(|k| k as f64 * step)
        .take_while(|t| *t <= t0 + v.span_secs)
        .map(|t| {
            let label = if step < 1.0 { fmt_precise(t).trim_end_matches('0').to_string() } else { fmt_short(t) };
            ((t - t0) / v.span_secs * 100.0, label)
        })
        .filter(|(pct, _)| *pct < 96.0) // a label at the far right edge would be clipped
        .collect();

    rsx! {
        div { class: "detail",
            div { class: "ruler",
                for (pct, label) in ticks {
                    div { class: "tick", style: "left: {pct}%", "{label}" }
                }
            }
            div { class: "detail-wave",
                if waveform {
                    Wave { scan: scan.clone(), start, end }
                }
                Silences { total: scan.0.info.total_samples, start, end }
                Parts { total: scan.0.info.total_samples, start, end, labels: true }
                AbBand { start, end }
                Markers { start, end, detail: true }
                TrackLabels { total: scan.0.info.total_samples, start, end }
                Playhead { start, end }
                Interact { start, end, center: false, edit_splits: true }
            }
        }
    }
}

#[component]
pub(super) fn Transport(scan: ScanRef) -> Element {
    let app = use_context::<state::App>();
    let rate = scan.0.info.sample_rate as f64;
    let total = scan.0.info.total_samples as f64 / rate;
    let pos = (app.pos)() as f64 / rate;
    let playing = (app.playing)();
    let preview = (app.preview)();
    let span = (app.view)().span_secs;
    rsx! {
        div { class: "transport",
            button {
                class: "play",
                onclick: move |_| app.toggle(),
                if playing { "❚❚" } else { "▶" }
            }
            span { class: "time", "{fmt_precise(pos)}" }
            span { class: "time dim", "/ {fmt_precise(total)}" }
            button {
                class: if preview { "preview on" } else { "preview" },
                title: "Play only what the export keeps: skip cut silence and left-out tracks (P)",
                onclick: move |_| app.toggle_preview(),
                if preview { "✂ Preview cuts: on" } else { "✂ Preview cuts" }
            }
            AbStatus {}
            span { class: "spacer" }
            button { onclick: move |_| app.zoom(2.0), "−" }
            span { class: "zoom", "{span:.0} s view" }
            button { onclick: move |_| app.zoom(0.5), "+" }
        }
    }
}

/// Tagged silence: cut from the export where it borders a track, kept (G), or inside a track.
#[component]
pub(super) fn Silences(total: u64, start: u64, end: u64) -> Element {
    let app = use_context::<state::App>();
    let cutlist = app.cutlist.read();
    let _ = app.selected.read();
    let Some(edit) = app.selected_path().and_then(|p| cutlist.recordings.get(&key_of(&p))) else { return rsx! {} };
    if end <= start {
        return rsx! {};
    }
    let span = (end - start) as f64;
    let bands: Vec<(usize, &str, f64, f64)> = edit
        .silences
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s, s.end.min(total)))
        .filter(|&(_, s, e)| e > start && s.start < end)
        .map(|(i, s, e)| {
            let edge = s.start == 0 || e >= total || edit.splits.iter().any(|p| s.start < p.at && p.at < e);
            let class = match (edge, s.keep) {
                (true, false) => "silence cut",
                (true, true) => "silence kept",
                (false, _) => "silence", // inside a track: never cut
            };
            let a = s.start.max(start) - start;
            let b = e.min(end) - start;
            (i, class, a as f64 / span * 100.0, (b - a) as f64 / span * 100.0)
        })
        .collect();
    rsx! {
        for (i, class, left, width) in bands {
            div { key: "{i}", class: "{class}", style: "left: {left}%; width: {width}%" }
        }
    }
}

/// Parts of the recording on the waveforms: left-out tracks in red ("✂ cut"), and the track
/// X and T act on, highlighted so it's clear what X will cut.
#[component]
pub(super) fn Parts(total: u64, start: u64, end: u64, labels: bool) -> Element {
    let app = use_context::<state::App>();
    let focus = use_focus_track();
    let cutlist = app.cutlist.read();
    let _ = app.selected.read();
    let Some(edit) = app.selected_path().and_then(|p| cutlist.recordings.get(&key_of(&p))) else { return rsx! {} };
    if end <= start {
        return rsx! {};
    }
    let focus = focus();
    let names = edit.names(total);
    let span = (end - start) as f64;
    let bands: Vec<(usize, &str, String, f64, f64)> = edit
        .tracks(total)
        .iter()
        .filter(|t| t.end > start && t.start < end && (t.meta.drop || focus == Some(t.index)))
        .map(|t| {
            let a = t.start.max(start) - start;
            let b = t.end.min(end) - start;
            let (class, label) = match (t.meta.drop, focus == Some(t.index)) {
                (true, true) => ("part cut focus", "✂ cut · X to keep".to_string()),
                (true, false) => ("part cut", "✂ cut".to_string()),
                _ => {
                    let name = names[t.index].clone().unwrap_or_else(|| format!("Track {}", t.index + 1));
                    ("part focus", format!("{name} · X to cut"))
                }
            };
            (t.index, class, label, a as f64 / span * 100.0, (b - a) as f64 / span * 100.0)
        })
        .collect();
    rsx! {
        for (k, class, label, left, width) in bands {
            div { key: "{k}", class: "{class}", style: "left: {left}%; width: {width}%",
                if labels {
                    span { class: "part-label", "{label}" }
                }
            }
        }
    }
}
