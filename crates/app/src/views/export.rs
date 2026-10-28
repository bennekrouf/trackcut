//! Reviewing the tracks found and exporting them.

use super::*;

/// Review progress, the selected split, and the silence-detection settings.
#[component]
pub(super) fn ReviewBar(scan: ScanRef) -> Element {
    let app = use_context::<state::App>();
    let rate = scan.0.info.sample_rate as f64;
    let cutlist = app.cutlist.read();
    let cur = (app.cur_split)();
    let _ = app.selected.read();
    let Some(edit) = app.selected_path().and_then(|p| cutlist.recordings.get(&key_of(&p)).cloned()) else {
        return rsx! {};
    };
    let (ok, todo) = edit.counts();
    let p = edit.detect;
    let selected = cur.and_then(|i| edit.splits.get(i).map(|s| (i, s.clone())));
    let done = edit.status == Status::Done;

    rsx! {
        div { class: "review",
            div { class: "review-status",
                span { class: "count", "{edit.splits.len() + 1} tracks" }
                span { class: "count ok", "{ok} confirmed" }
                span { class: if todo > 0 { "count todo" } else { "count" }, "{todo} to review" }
                match selected {
                    Some((i, s)) => rsx! {
                        span { class: "current-split",
                            "Split {i + 1} at {fmt_precise(s.at as f64 / rate)}"
                            if let Some(sil) = s.silence_secs {
                                " · {sil:.1} s silence"
                            }
                            match edit.silence_at(s.at).map(|j| edit.silences[j].keep) {
                                Some(false) => rsx! { " · silence cut (G keeps it)" },
                                Some(true) => rsx! { " · silence kept (G cuts it)" },
                                None => rsx! {},
                            }
                            if s.state == SplitState::Suggested { " · suggested" } else { " · confirmed" }
                        }
                    },
                    None if todo > 0 => rsx! { span { class: "current-split dim", "Press Tab or Enter to review the next split" } },
                    None if done => rsx! { span { class: "current-split ok", "✓ Marked done" } },
                    None => rsx! { span { class: "current-split dim", "All splits reviewed · ⌘Enter to mark done" } },
                }
            }
            div { class: "detect",
                span { class: "dim", "Silence below" }
                button { onclick: move |_| app.set_detect_params(|p| p.threshold_db -= 3.0), "−" }
                span { class: "value", "{p.threshold_db:.0} dB" }
                button { onclick: move |_| app.set_detect_params(|p| p.threshold_db = (p.threshold_db + 3.0).min(-6.0)), "+" }
                span { class: "dim", "for at least" }
                button { onclick: move |_| app.set_detect_params(|p| p.min_silence_secs = (p.min_silence_secs - 0.25).max(0.25)), "−" }
                span { class: "value", "{p.min_silence_secs:.2} s" }
                button { onclick: move |_| app.set_detect_params(|p| p.min_silence_secs += 0.25), "+" }
                button { class: "primary", onclick: move |_| app.redetect(), title: "Replace unreviewed suggestions; confirmed splits are kept", "Re-detect" }
            }
        }
    }
}

/// Paste a tracklist, export, and the export's progress/result.
#[component]
pub(super) fn ExportBar(scan: ScanRef) -> Element {
    let app = use_context::<state::App>();
    let cutlist = app.cutlist.read();
    let _ = app.selected.read();
    let Some(path) = app.selected_path() else { return rsx! {} };
    let key = key_of(&path);
    let Some(edit) = cutlist.recordings.get(&key) else { return rsx! {} };
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut planned = plan(edit, scan.0.info.total_samples, &stem, &cutlist.export);
    let kept = planned.len();
    let dropped = edit.splits.len() + 1 - kept;
    // Without Pro the export stops after the first tracks; the size, loudness and count
    // below describe what is actually written.
    planned.truncate(trackcut_core::license::exported_tracks(kept, app.licence.read().exports_all()));
    let count = planned.len();
    let beyond_free = kept - count;
    let status = app.exports.read().get(&key).cloned();
    let running = status.as_ref().is_some_and(|s| s.is_active());
    let profile = cutlist.export.profile;
    let normalize = cutlist.export.normalize;
    let norm_idx = Normalize::CHOICES.iter().position(|n| *n == normalize).unwrap_or(0);
    let map = app.loudness.read().get(&path).cloned();
    let set: Option<Loudness> = map.map(|m| m.ranges(&planned.iter().map(|t| (t.start, t.end)).collect::<Vec<_>>()));
    let info = &scan.0.info;
    let facts = source_facts(&path, &scan.0);
    // What Original really writes for this file (FLAC for the audio of a video).
    let effective = profile.effective(&facts);
    let secs: f64 = planned.iter().map(|t| (t.end - t.start) as f64).sum::<f64>() / info.sample_rate as f64;
    let bytes = if effective == Profile::Original {
        // Exact for the source's own format: its share of the file.
        (info.file_size as f64 * secs / info.duration_secs().max(1e-9)) as u64
    } else {
        profile.estimate_bytes(&facts, secs)
    };
    let size = fmt_bytes(bytes);
    // A video is exported as video clips only: its own formats, no A/B of audio formats.
    let video = trackcut_core::is_video(&path);
    let video_profile = cutlist.export.video;
    let video_idx = VideoProfile::CHOICES.iter().position(|p| *p == video_profile).unwrap_or(0);
    let warnings = if video { Vec::new() } else { profile.warnings(&facts) };
    let ext = match video {
        true => video_profile.extension().to_string(),
        false => effective.extension(path.extension().and_then(|e| e.to_str()).unwrap_or("mp3")).to_string(),
    };
    let selected_idx = Profile::CHOICES.iter().position(|p| *p == profile).unwrap_or(0);
    // Only a byte copy can't change level.
    let fixed_level = !video && effective == Profile::Original;

    rsx! {
        div { class: "export-bar",
            button { onclick: move |_| {
                    let mut paste = app.paste;
                    paste.set(Some(String::new()));
                },
                "Paste tracklist…"
            }
            if video {
                select {
                    class: "profile",
                    title: "Clips are cut exactly on the splits and re-encoded (H.264 and AAC)",
                    value: "{video_idx}",
                    onchange: move |e| {
                        if let Some(p) = e.value().parse::<usize>().ok().and_then(|i| VideoProfile::CHOICES.get(i)) {
                            app.set_video_profile(*p);
                        }
                        app.focus_root();
                    },
                    for (i, p) in VideoProfile::CHOICES.iter().enumerate() {
                        option { key: "{i}", value: "{i}", selected: i == video_idx, "{p.label()}" }
                    }
                }
            } else {
                select {
                    class: "profile",
                    title: "Export format",
                    value: "{selected_idx}",
                    onchange: move |e| {
                        if let Some(p) = e.value().parse::<usize>().ok().and_then(|i| Profile::CHOICES.get(i)) {
                            app.set_profile(*p);
                        }
                        app.focus_root();
                    },
                    for (i, p) in Profile::CHOICES.iter().enumerate() {
                        option { key: "{i}", value: "{i}", selected: i == selected_idx, "{p.label()}" }
                    }
                }
                button {
                    class: if profile.is_lossy() { "ab-button" } else { "ab-button dim" },
                    title: "Loop 12 s from the playhead and switch between the original and this format (A)",
                    disabled: profile == Profile::Original,
                    onclick: move |_| app.ab_toggle(),
                    "A/B"
                    kbd { "A" }
                }
            }
            select {
                class: "profile",
                title: if fixed_level {
                    "A byte copy can't change level; tracks get ReplayGain tags instead"
                } else {
                    "Loudness adjustment (never pushes true peak above -1 dBTP)"
                },
                disabled: fixed_level,
                value: "{norm_idx}",
                onchange: move |e| {
                    if let Some(n) = e.value().parse::<usize>().ok().and_then(|i| Normalize::CHOICES.get(i)) {
                        app.set_normalize(*n);
                    }
                    app.focus_root();
                },
                for (i, n) in Normalize::CHOICES.iter().enumerate() {
                    option { key: "{i}", value: "{i}", selected: i == norm_idx, "{n.label()}" }
                }
            }
            span { class: "dim",
                if beyond_free > 0 {
                    if video { "{count} of {kept} clips" } else { "{count} of {kept} tracks · ≈ {size}" }
                } else if video {
                    "{count} clips"
                } else {
                    "{count} tracks · ≈ {size}"
                }
                if dropped > 0 { " · {dropped} left out" }
                " → {stem}/*.{ext}"
            }
            if beyond_free > 0 {
                button {
                    class: "pro-upsell",
                    title: "The free version exports the first {trackcut_core::license::FREE_TRACKS} tracks of a recording",
                    onclick: move |_| {
                        let mut dialog = app.licence_dialog;
                        dialog.set(true);
                    },
                    "TrackCut Pro exports all {kept} →"
                }
            }
            match set {
                Some(Loudness { lufs: Some(l), peak_db }) => rsx! {
                    span {
                        class: if peak_db > -1.0 { "loud hot" } else { "loud" },
                        title: "Integrated loudness and true peak of the tracks being exported",
                        "{l:.1} LUFS · peak {peak_db:.1} dBTP"
                    }
                },
                Some(_) => rsx! {},
                None => rsx! { span { class: "dim", "measuring loudness…" } },
            }
            span { class: "spacer" }
            match status {
                Some(ExportStatus::Queued) => rsx! { span { class: "dim", "Queued…" } },
                Some(ExportStatus::Measuring) => rsx! { span { class: "dim", "Measuring loudness…" } },
                Some(ExportStatus::Installing(p)) => rsx! {
                    span { class: "dim", "Installing ffmpeg for video export… {(p * 100.0) as u32}%" }
                },
                Some(s @ ExportStatus::Running { done, total, .. }) => rsx! {
                    span { class: "dim", "Exporting {done + 1}/{total}…" }
                    div { class: "progress small", div { style: "width: {s.fraction().unwrap_or(0.0) * 100.0}%" } }
                },
                Some(ExportStatus::Finished { dir, count }) => rsx! {
                    span { class: "ok", if video { "✓ Exported {count} clips" } else { "✓ Exported {count} tracks" } }
                    button { onclick: move |_| app.reveal_export(&dir), "Show in Finder" }
                },
                Some(ExportStatus::Cancelled) => rsx! { span { class: "dim", "Export cancelled" } },
                Some(ExportStatus::Failed(e)) => rsx! { span { class: "bad", title: "{e}", "Export failed" } },
                None => rsx! {},
            }
            button {
                class: "primary",
                disabled: count == 0 || running,
                onclick: move |_| app.export_current(),
                "Export"
                kbd { "⌘E" }
            }
        }
        if !warnings.is_empty() {
            div { class: "warnings",
                for (i, w) in warnings.into_iter().enumerate() {
                    div { key: "{i}", "⚠ {w}" }
                }
            }
        }
    }
}

#[component]
pub(super) fn TrackList(scan: ScanRef) -> Element {
    let app = use_context::<state::App>();
    let current = use_focus_track();
    // Keep the current row visible.
    use_effect(move || {
        let _ = current();
        document::eval(
            "requestAnimationFrame(() => document.querySelector('.track.current')?.scrollIntoView({block: 'nearest'}))",
        );
    });

    let rate = scan.0.info.sample_rate as f64;
    let total = scan.0.info.total_samples;
    let cutlist = app.cutlist.read();
    let editing = (app.editing_title)();
    let _ = app.selected.read();
    let Some(edit) = app.selected_path().and_then(|p| cutlist.recordings.get(&key_of(&p)).cloned()) else {
        return rsx! {};
    };
    let names = edit.names(total);
    let tracks: Vec<_> = edit
        .tracks(total)
        .into_iter()
        .map(|t| (t.index, t.audio_start, t.audio_end, t.meta.clone(), names[t.index].clone().unwrap_or_default()))
        .collect();
    let count = tracks.len();
    let current = current();
    let map = app.selected_path().and_then(|p| app.loudness.read().get(&p).cloned());

    rsx! {
        div { class: "tracks",
            for (k, a, b, meta, name) in tracks {
                {
                    // Track k starts at split k-1.
                    let state = k.checked_sub(1).and_then(|i| edit.splits.get(i)).map(|s| s.state);
                    let (badge, badge_class) = match state {
                        None => ("start", "badge"),
                        Some(SplitState::Confirmed) => ("confirmed", "badge ok"),
                        Some(SplitState::Suggested) => ("suggested", "badge todo"),
                    };
                    let mut class = String::from("track");
                    if current == Some(k) {
                        class.push_str(" current");
                    }
                    if meta.drop || b <= a {
                        class.push_str(" dropped-row");
                    }
                    rsx! {
                        div {
                            key: "{k}",
                            class: "{class}",
                            onclick: move |_| app.play_track(k),
                            span { class: "num", "{k + 1}" }
                            span { class: "t", "{fmt_precise(a as f64 / rate)}" }
                            span { class: "len", "{fmt_short((b - a) as f64 / rate)}" }
                            if editing == Some(k) {
                                TitleEditor {
                                    key: "{k}",
                                    track: k,
                                    count,
                                    initial: meta.title.clone(),
                                    suggested: name.clone(),
                                }
                            } else {
                                div { class: "title-cell",
                                    span {
                                        class: if meta.title.is_empty() { "title empty" } else { "title" },
                                        if !meta.title.is_empty() {
                                            "{meta.title}"
                                        } else if !name.is_empty() {
                                            "{name}"
                                        } else {
                                            "untitled"
                                        }
                                    }
                                    button {
                                        class: "rename",
                                        title: "Rename (T)",
                                        onclick: move |e| {
                                            e.stop_propagation();
                                            app.edit_title(Some(k));
                                        },
                                        "✎"
                                    }
                                }
                            }
                            {
                                match map.as_ref().map(|m| m.range(a, b)) {
                                    Some(Loudness { lufs: Some(l), peak_db }) => rsx! {
                                        span { class: "lufs", title: "Integrated loudness", "{l:.1}" }
                                        span {
                                            class: if peak_db > -1.0 { "peak hot" } else { "peak" },
                                            title: "True peak (dBTP)",
                                            "{peak_db:.1}"
                                        }
                                    },
                                    Some(_) => rsx! { span { class: "lufs dim", "silent" } span { class: "peak" } },
                                    None => rsx! { span { class: "lufs dim", "…" } span { class: "peak" } },
                                }
                            }
                            span { class: "{badge_class}", "{badge}" }
                            button {
                                class: "drop-toggle",
                                title: if meta.drop { "Include in the export (X)" } else { "Leave out of the export (X)" },
                                onclick: move |e| {
                                    e.stop_propagation();
                                    app.toggle_drop(Some(k));
                                },
                                if meta.drop { "↺" } else { "✕" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Inline title field. Enter saves and moves on to the next track, Esc cancels.
#[component]
pub(super) fn TitleEditor(track: usize, count: usize, initial: String, suggested: String) -> Element {
    let app = use_context::<state::App>();
    let mut draft = use_signal(|| initial.clone());
    let mut closed = use_signal(|| false);
    let suggestion = use_signal(|| suggested.clone());
    // `accept`: an empty field takes the suggested name (Tab), instead of staying untitled.
    let mut close = move |save: bool, accept: bool, next: Option<usize>| {
        if closed() {
            return;
        }
        closed.set(true);
        if save {
            let typed = draft();
            let title = if accept && typed.trim().is_empty() { suggestion() } else { typed };
            app.set_title(track, title);
        }
        let mut editing = app.editing_title;
        editing.set(next);
        if next.is_none() {
            app.focus_root();
        }
    };
    rsx! {
        input {
            class: "title-input",
            value: "{draft}",
            placeholder: if suggested.is_empty() { "Title".to_string() } else { format!("{suggested} (Tab to accept)") },
            onmounted: move |e| async move {
                let _ = e.set_focus(true).await;
            },
            oninput: move |e| draft.set(e.value()),
            onclick: move |e| e.stop_propagation(),
            onkeydown: move |e| {
                e.stop_propagation();
                let next = (track + 1 < count).then_some(track + 1);
                match e.key() {
                    Key::Enter => close(true, false, next),
                    // Tab: keep what's typed, or take the suggested name; then the next (⇧ previous) track.
                    Key::Tab => {
                        e.prevent_default();
                        close(true, true, if e.modifiers().shift() { track.checked_sub(1) } else { next });
                    }
                    Key::Escape => close(false, false, None),
                    _ => {}
                }
            },
            onblur: move |_| close(true, false, None),
        }
    }
}
