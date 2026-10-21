//! `trackcut mcp` — TrackCut's recordings, served to an MCP client over stdio.
//!
//! No window, no Dioxus, no player: the same scan, detection, cutlist and export the window
//! uses, driven from tool calls instead. A recording is named by its path; its edits live in
//! its folder's `trackcut.cutlist.json`, which the window may have open too. So every change
//! here reads the cutlist, applies the change and writes it back at once, and the window
//! loads what changed (`App::reload_if_changed`).
//!
//! Requests are handled one at a time, on the main thread, so an export or a download holds
//! the next call until it is done. Fine for one client; the place to change if that stops
//! being true is `serve`.

use crate::cleaning::{cleaned_path, write_cleaned};
use crate::download::{download_now, Kind};
use crate::exporting::export_now;
use crate::review::{detect_once, keep_clear, suggest, tag, SNAP_SECS};
use crate::state::{key_of, prefs};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use trackcut_audio::scan::load_or_scan;
use trackcut_audio::Scan;
use trackcut_core::cutlist::Cutlist;
use trackcut_core::edit::{RecordingEdit, Split, SplitState};
use trackcut_core::export::{ExportSettings, Normalize, Profile, VideoProfile};
use trackcut_core::license::FREE_TRACKS;
use trackcut_core::time::fmt_precise;
use trackcut_core::{detect, is_recording, is_video, list_recordings, tracklist, Status};

/// The newest protocol revision this server speaks. A client asking for an older one gets
/// its own back; everything used here exists in all of them.
const PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "TrackCut splits long recordings (MP3, WAV, or the sound of an MP4/MOV \
video) into tracks at silences, then exports one file per track. Its edits for a folder are kept \
in the folder, so recordings are named by their absolute path. Start with list_recordings, which \
defaults to the folder open in the TrackCut window. get_recording shows a recording's splits and \
tracks; the first time, it scans the file and suggests splits at silences, which can take a few \
seconds for a long file. Suggested splits are unreviewed: tell the user where they fall and let \
them confirm, or confirm them when asked (edit_splits). Times are in seconds. A track's title \
comes from set_tracks or paste_tracklist; a track left out (talk, applause) is not exported. \
export writes the kept tracks into a folder named after the recording, next to it; the free \
version writes the first tracks only. apply_cuts writes a copy without the cut parts. download \
fetches a recording from a URL. Recordings are the user's files: ask before exporting into their \
folders or downloading.";

pub fn serve() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            write(&mut stdout, &error(Value::Null, -32700, "not JSON"));
            continue;
        };
        // A notification (no id) asks for nothing back: `initialized`, cancellations, and
        // whatever later revisions add.
        let Some(id) = message.get("id").cloned() else {
            continue;
        };
        write(&mut stdout, &handle(id, &message));
    }
}

fn handle(id: Value, message: &Value) -> Value {
    let method = message["method"].as_str().unwrap_or("");
    let params = &message["params"];
    match method {
        "initialize" => {
            let asked = params["protocolVersion"].as_str().unwrap_or(PROTOCOL);
            let version = if asked <= PROTOCOL { asked } else { PROTOCOL };
            result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "trackcut", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": INSTRUCTIONS,
                }),
            )
        }
        "ping" => result(id, json!({})),
        "tools/list" => result(id, json!({ "tools": tools() })),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or("");
            let (text, is_error) = match call(name, &params["arguments"]) {
                Ok(value) => (pretty(&value), false),
                Err(why) => (why, true),
            };
            result(id, json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
        }
        _ => error(id, -32601, &format!("no method {method}")),
    }
}

fn write(stdout: &mut std::io::Stdout, reply: &Value) {
    // One message per line, and nothing else ever on stdout: the client reads it as the
    // protocol, so a stray print is a corrupt message.
    let mut bytes = serde_json::to_vec(reply).unwrap_or_default();
    bytes.push(b'\n');
    let _ = stdout.write_all(&bytes);
    let _ = stdout.flush();
}

fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

// ── Tool definitions ─────────────────────────────────────────────────────────────────────

fn tools() -> Value {
    let path = json!({ "type": "string", "description": "Absolute path of the recording." });
    let ids = json!({
        "type": "array", "items": { "type": "integer", "minimum": 0 },
        "description": "Split ids, as get_recording lists them.",
    });
    json!([
        {
            "name": "list_recordings",
            "description": "The recordings in a folder (MP3, WAV, MP4, M4V, MOV), with each one's \
                            status and how many splits it has, and the folder's export settings.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": { "type": "string", "description": "Absolute path. Defaults to the folder open in the TrackCut window." },
                },
            },
        },
        {
            "name": "get_recording",
            "description": "One recording: its length, its splits (suggested or confirmed) and its \
                            tracks with their times, titles and whether each is exported. Scans \
                            the file and suggests splits at silences the first time.",
            "inputSchema": { "type": "object", "properties": { "path": path }, "required": ["path"] },
        },
        {
            "name": "detect",
            "description": "Suggests splits again with new silence settings. Confirmed splits stay; \
                            the unreviewed suggestions are replaced.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": path,
                    "threshold_db": { "type": "number", "description": "Quieter than this counts as silence (dBFS; default -45). Raise it (e.g. -35) for a noisy room." },
                    "min_silence_secs": { "type": "number", "description": "Shortest silence that splits (default 1.5)." },
                    "min_track_secs": { "type": "number", "description": "Shortest track: closer suggestions keep the longer silence (default 30)." },
                },
                "required": ["path"],
            },
        },
        {
            "name": "edit_splits",
            "description": "Confirms, moves, removes and adds splits, in one go. Split ids are the \
                            ones get_recording returned before this call. Returns the recording \
                            as it is after.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": path,
                    "confirm": ids,
                    "confirm_all": { "type": "boolean", "description": "Confirm every suggested split." },
                    "remove": ids,
                    "move": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": { "id": { "type": "integer" }, "to": { "type": "number", "description": "Seconds." } },
                            "required": ["id", "to"],
                        },
                    },
                    "add": { "type": "array", "items": { "type": "number" }, "description": "New confirmed splits, in seconds." },
                    "snap": { "type": "boolean", "description": format!("Move added and moved splits to the quietest point within {SNAP_SECS} s. Default true.") },
                },
                "required": ["path"],
            },
        },
        {
            "name": "set_tracks",
            "description": "Sets track titles, and leaves tracks out of the export or puts them \
                            back. Tracks are numbered as get_recording lists them, left-out ones \
                            included.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": path,
                    "tracks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "track": { "type": "integer", "minimum": 1 },
                                "title": { "type": "string", "description": "Empty clears it (the file is then called Track N)." },
                                "keep": { "type": "boolean", "description": "false leaves it out of the export." },
                            },
                            "required": ["track"],
                        },
                    },
                },
                "required": ["path", "tracks"],
            },
        },
        {
            "name": "paste_tracklist",
            "description": "Gives the exported tracks the titles in a pasted tracklist, one per \
                            line, in order. Numbering and times on each line (\"1. Intro (4:32)\") \
                            are removed. Left-out tracks get none.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": path, "text": { "type": "string" } },
                "required": ["path", "text"],
            },
        },
        {
            "name": "set_status",
            "description": "Marks a recording to do, in progress, done or flagged, as the dot in the \
                            window's file list shows it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": path,
                    "status": { "type": "string", "enum": ["todo", "in_progress", "done", "flagged"] },
                },
                "required": ["path", "status"],
            },
        },
        {
            "name": "export",
            "description": format!(
                "Writes one file per kept track into a folder named after the recording, next to \
                 it, with title, album and track tags; a video's tracks are MP4 clips. Uses the \
                 folder's export settings unless given here (just for this export). The free \
                 version writes the first {FREE_TRACKS} tracks of a recording; TrackCut Pro writes \
                 them all."
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": path,
                    "profile": {
                        "type": "string",
                        "description": "original (a copy, no re-encoding), flac, wav16, mp3-v0, mp3-v2, mp3-v4, mp3-320, mp3-256, mp3-192 or mp3-128.",
                    },
                    "normalize": {
                        "type": "string",
                        "description": "off, album:-14 or album:-16 (one gain for the whole recording), track:-14 or track:-16 (each track to that LUFS). Needs a re-encoding profile.",
                    },
                    "video_quality": { "type": "string", "enum": ["high", "standard", "small"], "description": "For a video's clips." },
                },
                "required": ["path"],
            },
        },
        {
            "name": "apply_cuts",
            "description": "Writes a copy of the recording without its cut silence and left-out \
                            tracks, next to it (\"name (cleaned).wav\", or .mp4 for a video), with \
                            the tracks' titles carried over. The original is not touched.",
            "inputSchema": { "type": "object", "properties": { "path": path }, "required": ["path"] },
        },
        {
            "name": "download",
            "description": "Downloads a recording from a URL (YouTube or another site yt-dlp \
                            supports) into ~/Music/TrackCut/Downloads: the sound as a WAV or MP3, \
                            or with video: true the video as an MP4. Installs yt-dlp on first use.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string" },
                    "video": { "type": "boolean", "description": "The video too, not only its sound." },
                },
                "required": ["url"],
            },
        },
    ])
}

fn call(name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "list_recordings" => list(args),
        "get_recording" => {
            let rec = Rec::from(args)?;
            rec.change(|_, _| Ok(()))?;
            rec.describe()
        }
        "detect" => detect_again(args),
        "edit_splits" => edit_splits(args),
        "set_tracks" => set_tracks(args),
        "paste_tracklist" => {
            let rec = Rec::from(args)?;
            let text = text(args, "text")?;
            let titles = tracklist::parse(&text);
            if titles.is_empty() {
                return Err("the tracklist has no titles in it".into());
            }
            let set = rec.change(|edit, _| Ok(tracklist::apply(edit, &titles)))?;
            let mut v = rec.describe()?;
            v["titles_set"] = json!(set);
            v["titles_in_tracklist"] = json!(titles.len());
            Ok(v)
        }
        "set_status" => {
            let rec = Rec::from(args)?;
            let status = status(&text(args, "status")?)?;
            rec.change(|edit, _| {
                edit.status = status;
                Ok(())
            })?;
            Ok(json!({ "path": rec.path, "status": status_name(status) }))
        }
        "export" => export(args),
        "apply_cuts" => apply_cuts(args),
        "download" => {
            let url = text(args, "url")?;
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Err("url should start with https://".into());
            }
            let kind = if args["video"].as_bool().unwrap_or(false) { Kind::Video } else { Kind::Audio };
            let file = download_now(&url, kind)?;
            Ok(json!({
                "path": file,
                "folder": file.parent(),
                "note": "Call get_recording with this path to split it.",
            }))
        }
        _ => Err(format!("no tool {name}")),
    }
}

// ── Arguments ────────────────────────────────────────────────────────────────────────────

fn text(args: &Value, key: &str) -> Result<String, String> {
    args[key].as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from).ok_or(format!("{key} is required"))
}

fn status(v: &str) -> Result<Status, String> {
    match v.trim().to_lowercase().replace([' ', '-'], "_").as_str() {
        "todo" | "to_do" => Ok(Status::Todo),
        "in_progress" => Ok(Status::InProgress),
        "done" => Ok(Status::Done),
        "flagged" => Ok(Status::Flagged),
        _ => Err(format!("status is todo, in_progress, done or flagged, not {v}")),
    }
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Todo => "todo",
        Status::InProgress => "in_progress",
        Status::Done => "done",
        Status::Flagged => "flagged",
        Status::Exported => "exported",
    }
}

fn profile(v: &str) -> Result<Profile, String> {
    let v = v.trim().to_lowercase();
    let parsed = match v.as_str() {
        "original" => Some(Profile::Original),
        "flac" => Some(Profile::Flac),
        "wav16" | "wav" => Some(Profile::Wav16),
        n => n
            .strip_prefix("mp3-v")
            .and_then(|q| q.parse().ok())
            .map(|quality| Profile::Mp3Vbr { quality })
            .or_else(|| n.strip_prefix("mp3-").and_then(|k| k.parse().ok()).map(|kbps| Profile::Mp3Cbr { kbps })),
    };
    parsed.filter(|p| Profile::CHOICES.contains(p)).ok_or(format!(
        "no profile {v}; use original, flac, wav16, mp3-v0, mp3-v2, mp3-v4, mp3-320, mp3-256, mp3-192 or mp3-128"
    ))
}

fn normalize(v: &str) -> Result<Normalize, String> {
    let v = v.trim().to_lowercase();
    let parsed = match v.split_once(':') {
        None if v == "off" => Some(Normalize::Off),
        Some(("album", t)) => t.trim().parse().ok().map(|target| Normalize::Album { target }),
        Some(("track", t)) => t.trim().parse().ok().map(|target| Normalize::Track { target }),
        _ => None,
    };
    parsed
        .filter(|n| Normalize::CHOICES.contains(n))
        .ok_or(format!("no normalize {v}; use off, album:-14, album:-16, track:-14 or track:-16"))
}

fn video_quality(v: &str) -> Result<VideoProfile, String> {
    match v.trim().to_lowercase().as_str() {
        "high" => Ok(VideoProfile::CHOICES[0]),
        "standard" => Ok(VideoProfile::CHOICES[1]),
        "small" => Ok(VideoProfile::CHOICES[2]),
        _ => Err(format!("video_quality is high, standard or small, not {v}")),
    }
}

fn split_ids(args: &Value, key: &str, edit: &RecordingEdit) -> Result<Vec<u64>, String> {
    let Some(list) = args[key].as_array() else { return Ok(vec![]) };
    list.iter()
        .map(|v| {
            let id = v.as_u64().ok_or(format!("{key} holds split ids, which are numbers"))? as usize;
            edit.splits.get(id).map(|s| s.at).ok_or(format!(
                "no split {id}: it has {} (0 to {})",
                edit.splits.len(),
                edit.splits.len().saturating_sub(1)
            ))
        })
        .collect()
}

// ── One recording ────────────────────────────────────────────────────────────────────────

/// A recording named by a tool call, and its folder.
struct Rec {
    path: PathBuf,
    dir: PathBuf,
    key: String,
}

impl Rec {
    fn from(args: &Value) -> Result<Self, String> {
        let path = PathBuf::from(text(args, "path")?);
        if !path.is_absolute() {
            return Err("path must be absolute".into());
        }
        if !path.is_file() {
            return Err(format!("{} is not a file", path.display()));
        }
        if !is_recording(&path) {
            return Err(format!("{} is not a recording TrackCut opens (MP3, WAV, MP4, M4V, MOV)", path.display()));
        }
        let dir = path.parent().ok_or("the recording has no folder")?.to_owned();
        Ok(Self { key: key_of(&path), path, dir })
    }

    /// The scan: read from its cache next to TrackCut's other caches, or made (and cached).
    fn scan(&self) -> Result<Arc<Scan>, String> {
        load_or_scan(&self.path, &mut |_| {})
            .map(Arc::new)
            .map_err(|e| format!("could not read {}: {e:#}", self.path.display()))
    }

    /// The folder's cutlist as it is on disk now. One that can't be read is never written
    /// over, as in the window.
    fn cutlist(&self) -> Result<Cutlist, String> {
        Cutlist::load(&self.dir).map_err(|e| format!("could not read the cutlist in {}: {e}", self.dir.display()))
    }

    /// Applies `f` to the recording's edit, as on disk now, and saves it. The first time, the
    /// recording is detected first, as the window does when it opens it.
    fn change<R>(&self, f: impl FnOnce(&mut RecordingEdit, &Scan) -> Result<R, String>) -> Result<R, String> {
        let scan = self.scan()?;
        let mut cutlist = self.cutlist()?;
        let before = cutlist.clone();
        let edit = cutlist.recordings.entry(self.key.clone()).or_default();
        detect_once(edit, &scan);
        let r = f(edit, &scan)?;
        if cutlist != before {
            cutlist.save(&self.dir).map_err(|e| format!("could not save the cutlist: {e}"))?;
        }
        Ok(r)
    }

    fn describe(&self) -> Result<Value, String> {
        let scan = self.scan()?;
        let cutlist = self.cutlist()?;
        let edit = cutlist.recordings.get(&self.key).cloned().unwrap_or_default();
        Ok(describe(&self.path, &scan, &edit, &cutlist.export))
    }
}

fn secs(frames: u64, rate: u32) -> f64 {
    (frames as f64 / rate.max(1) as f64 * 1000.0).round() / 1000.0
}

fn describe(path: &Path, scan: &Scan, edit: &RecordingEdit, settings: &ExportSettings) -> Value {
    let rate = scan.info.sample_rate;
    let total = scan.info.total_samples;
    let splits: Vec<Value> = edit
        .splits
        .iter()
        .enumerate()
        .map(|(id, s)| {
            let mut v = json!({
                "id": id,
                "at": secs(s.at, rate),
                "time": fmt_precise(secs(s.at, rate)),
                "state": match s.state { SplitState::Suggested => "suggested", SplitState::Confirmed => "confirmed" },
            });
            if let Some(silence) = s.silence_secs {
                v["silence_secs"] = json!((silence * 10.0).round() / 10.0);
            }
            v
        })
        .collect();
    let names = edit.names(total);
    let tracks: Vec<Value> = edit
        .tracks(total)
        .iter()
        .map(|t| {
            let mut v = json!({
                "track": t.index + 1,
                "start": secs(t.start, rate),
                "end": secs(t.end, rate),
                "time": format!("{} – {}", fmt_precise(secs(t.start, rate)), fmt_precise(secs(t.end, rate))),
                "length": secs(t.audio_len(), rate),
                "exported": t.exported(),
            });
            if let Some(name) = &names[t.index] {
                v["name"] = json!(name);
            }
            if t.meta.drop {
                v["left_out"] = json!(true);
            }
            let trimmed = (t.audio_start - t.start) + (t.end - t.audio_end);
            if trimmed > 0 {
                v["silence_cut_secs"] = json!(secs(trimmed, rate));
            }
            v
        })
        .collect();
    let (confirmed, suggested) = edit.counts();
    let kept = tracks.iter().filter(|t| t["exported"] == json!(true)).count();
    let mut v = json!({
        "path": path,
        "kind": if is_video(path) { "video" } else { "audio" },
        "codec": scan.info.codec,
        "length": secs(total, rate),
        "time": fmt_precise(secs(total, rate)),
        "status": status_name(edit.status),
        "splits_confirmed": confirmed,
        "splits_suggested": suggested,
        "tracks_exported": kept,
        "detection": {
            "threshold_db": edit.detect.threshold_db,
            "min_silence_secs": edit.detect.min_silence_secs,
            "min_track_secs": edit.detect.min_track_secs,
        },
        "export": export_settings(settings, is_video(path)),
        "splits": splits,
        "tracks": tracks,
    });
    if scan.info.silent {
        v["note"] = json!("This video has no sound: nothing is detected, and every split is placed by hand.");
    }
    v
}

fn export_settings(s: &ExportSettings, video: bool) -> Value {
    if video {
        json!({ "format": s.video.label(), "loudness": s.normalize.label(), "naming": s.naming })
    } else {
        json!({ "format": s.profile.label(), "loudness": s.normalize.label(), "naming": s.naming })
    }
}

// ── Tools ────────────────────────────────────────────────────────────────────────────────

fn list(args: &Value) -> Result<Value, String> {
    let folder = match args["folder"].as_str().map(str::trim).filter(|s| !s.is_empty()) {
        Some(f) => PathBuf::from(f),
        None => prefs::last_folder().ok_or("no folder is open in the TrackCut window: pass folder")?,
    };
    if !folder.is_absolute() {
        return Err("folder must be absolute".into());
    }
    let recordings = list_recordings(&folder).map_err(|e| format!("could not read {}: {e}", folder.display()))?;
    let cutlist =
        Cutlist::load(&folder).map_err(|e| format!("could not read the cutlist in {}: {e}", folder.display()))?;
    let rows: Vec<Value> = recordings
        .iter()
        .map(|r| {
            let edit = cutlist.recordings.get(&key_of(&r.path));
            let (confirmed, suggested) = edit.map(|e| e.counts()).unwrap_or_default();
            json!({
                "path": r.path,
                "name": r.name(),
                "kind": if is_video(&r.path) { "video" } else { "audio" },
                "status": status_name(edit.map(|e| e.status).unwrap_or_default()),
                "opened": edit.is_some_and(|e| e.detected),
                "splits_confirmed": confirmed,
                "splits_suggested": suggested,
            })
        })
        .collect();
    Ok(json!({
        "folder": folder,
        "recordings": rows,
        "export": {
            "audio": export_settings(&cutlist.export, false),
            "video": export_settings(&cutlist.export, true),
        },
    }))
}

fn detect_again(args: &Value) -> Result<Value, String> {
    let rec = Rec::from(args)?;
    rec.change(|edit, scan| {
        let number = |key: &str| args[key].as_f64().map(|v| v as f32);
        if let Some(v) = number("threshold_db") {
            edit.detect.threshold_db = v.clamp(-90.0, -10.0);
        }
        if let Some(v) = number("min_silence_secs") {
            edit.detect.min_silence_secs = v.clamp(0.1, 30.0);
        }
        if let Some(v) = number("min_track_secs") {
            edit.detect.min_track_secs = v.clamp(0.0, 3600.0);
        }
        let suggestions = suggest(scan, &edit.detect);
        edit.apply_detection(suggestions, keep_clear(scan));
        edit.set_silences(tag(scan, &edit.detect));
        Ok(())
    })?;
    rec.describe()
}

fn edit_splits(args: &Value) -> Result<Value, String> {
    let rec = Rec::from(args)?;
    let snapping = args["snap"].as_bool().unwrap_or(true);
    rec.change(|edit, scan| {
        let rate = scan.info.sample_rate;
        let total = scan.info.total_samples;
        let l = &scan.loudness;
        let place = |secs: f64| -> Result<u64, String> {
            let at = (secs * rate as f64).round();
            if !(at > 0.0 && (at as u64) < total) {
                return Err(format!(
                    "{secs} s is not inside the recording (0 to {})",
                    fmt_precise(total as f64 / rate as f64)
                ));
            }
            let at = at as u64;
            Ok(if snapping { detect::snap(&l.db, l.window, rate, at, SNAP_SECS) } else { at })
        };

        // Every id names a split as it was before this call: resolve them all to positions
        // first, since removing or adding one renumbers the rest.
        let confirm = if args["confirm_all"].as_bool().unwrap_or(false) {
            edit.splits.iter().map(|s| s.at).collect()
        } else {
            split_ids(args, "confirm", edit)?
        };
        let remove = split_ids(args, "remove", edit)?;
        let mut moves = vec![];
        for m in args["move"].as_array().into_iter().flatten() {
            let id = m["id"].as_u64().ok_or("each move needs an id")? as usize;
            let from = edit.splits.get(id).map(|s| s.at).ok_or(format!("no split {id}"))?;
            let to = place(m["to"].as_f64().ok_or("each move needs to, in seconds")?)?;
            moves.push((from, to));
        }
        let add: Vec<u64> = args["add"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| v.as_f64().ok_or_else(|| "add holds seconds, which are numbers".to_string()).and_then(place))
            .collect::<Result<_, _>>()?;

        let index_of = |edit: &RecordingEdit, at: u64| edit.splits.iter().position(|s| s.at == at);
        for at in confirm {
            if let Some(i) = index_of(edit, at) {
                edit.confirm(i);
            }
        }
        for (from, to) in moves {
            if let Some(i) = index_of(edit, from) {
                let i = edit.move_to(i, to);
                edit.confirm(i);
            }
        }
        for at in remove {
            if let Some(i) = index_of(edit, at) {
                edit.remove(i);
            }
        }
        for at in add {
            if index_of(edit, at).is_none() {
                edit.insert(Split::confirmed(at));
            }
        }
        if edit.status == Status::Todo {
            edit.status = Status::InProgress;
        }
        Ok(())
    })?;
    rec.describe()
}

fn set_tracks(args: &Value) -> Result<Value, String> {
    let rec = Rec::from(args)?;
    let changes = args["tracks"].as_array().ok_or("tracks is required")?.clone();
    rec.change(|edit, _| {
        let count = edit.splits.len() + 1;
        for c in &changes {
            let n = c["track"].as_u64().ok_or("each track needs its number")? as usize;
            let meta =
                n.checked_sub(1).and_then(|i| edit.track_meta_mut(i)).ok_or(format!("no track {n}: it has {count}"))?;
            if let Some(title) = c["title"].as_str() {
                meta.title = title.trim().to_owned();
            }
            if let Some(keep) = c["keep"].as_bool() {
                meta.drop = !keep;
            }
        }
        Ok(())
    })?;
    rec.describe()
}

fn export(args: &Value) -> Result<Value, String> {
    let rec = Rec::from(args)?;
    // Detected and saved first, so a recording never opened exports what the window would.
    rec.change(|_, _| Ok(()))?;
    let scan = rec.scan()?;
    let cutlist = rec.cutlist()?;
    let mut settings = cutlist.export.clone();
    if let Some(p) = args["profile"].as_str() {
        settings.profile = profile(p)?;
    }
    if let Some(n) = args["normalize"].as_str() {
        settings.normalize = normalize(n)?;
    }
    if let Some(q) = args["video_quality"].as_str() {
        settings.video = video_quality(q)?;
    }
    let pro = crate::licence::current().exports_all();
    let done = export_now(&rec.path, scan, cutlist.recordings.get(&rec.key), &settings, pro)?;
    // Marked exported, as the window marks it.
    rec.change(|edit, _| {
        edit.status = Status::Exported;
        Ok(())
    })?;
    let mut v = json!({
        "folder": done.dir,
        "files": done.files,
        "format": if is_video(&rec.path) { settings.video.label() } else { settings.profile.label() },
        "loudness": settings.normalize.label(),
    });
    if done.kept > done.files.len() {
        v["note"] = json!(format!(
            "The free version of TrackCut exports the first {FREE_TRACKS} tracks of a recording: {} of {} were \
             written. TrackCut Pro exports them all: {}",
            done.files.len(),
            done.kept,
            crate::licence::BUY_URL
        ));
    }
    Ok(v)
}

fn apply_cuts(args: &Value) -> Result<Value, String> {
    let rec = Rec::from(args)?;
    rec.change(|_, _| Ok(()))?;
    let scan = rec.scan()?;
    let cutlist = rec.cutlist()?;
    let edit = cutlist.recordings.get(&rec.key).cloned().unwrap_or_default();
    let total = scan.info.total_samples;
    if edit.skipped(total).is_empty() {
        return Err(
            "nothing is cut in this recording yet: no silence at the tracks' edges and no track left out".into()
        );
    }
    let (ranges, mut cleaned) = edit.cleaned(total);
    if ranges.is_empty() {
        return Err("every track is left out: there is nothing to keep".into());
    }
    let to = cleaned_path(&rec.path);
    let video = is_video(&rec.path).then_some(cutlist.export.video);
    let shift = write_cleaned(&rec.path, &scan, &ranges, &to, video, &mut |_| {})?;
    for s in &mut cleaned.splits {
        s.at += shift;
    }
    let mut cutlist = rec.cutlist()?;
    cutlist.recordings.insert(key_of(&to), cleaned);
    cutlist.save(&rec.dir).map_err(|e| format!("could not save the cutlist: {e}"))?;
    Ok(json!({ "path": to, "note": "The copy keeps the tracks and their titles; get_recording shows it." }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_tool_has_a_unique_name_and_an_object_schema() {
        let tools = tools();
        let tools = tools.as_array().unwrap();
        let names: HashSet<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names.len(), tools.len());
        for t in tools {
            assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
            for required in t["inputSchema"]["required"].as_array().into_iter().flatten() {
                let key = required.as_str().unwrap();
                assert!(t["inputSchema"]["properties"].get(key).is_some(), "{} requires {key} undescribed", t["name"]);
            }
        }
    }

    #[test]
    fn a_tool_that_does_not_exist_is_an_error() {
        assert!(call("nope", &json!({})).is_err());
    }

    #[test]
    fn export_choices_are_read_as_the_example_cli_names_them() {
        assert_eq!(profile("MP3-V2").unwrap(), Profile::Mp3Vbr { quality: 2 });
        assert_eq!(profile("mp3-320").unwrap(), Profile::Mp3Cbr { kbps: 320 });
        assert_eq!(profile("flac").unwrap(), Profile::Flac);
        assert!(profile("mp3-v7").is_err(), "not one of the window's choices");
        assert_eq!(normalize("album:-14").unwrap(), Normalize::Album { target: -14.0 });
        assert_eq!(normalize("off").unwrap(), Normalize::Off);
        assert!(normalize("album:-9").is_err());
        assert_eq!(video_quality("small").unwrap(), VideoProfile::Mp4 { crf: 28 });
        assert_eq!(status("in progress").unwrap(), Status::InProgress);
        assert!(status("exported").is_err(), "set by an export, not by hand");
    }

    #[test]
    fn a_path_must_be_an_absolute_recording() {
        assert!(Rec::from(&json!({ "path": "set.mp3" })).is_err());
        assert!(Rec::from(&json!({ "path": "/nowhere/set.mp3" })).is_err());
        let notes = std::env::temp_dir().join(format!("trackcut-mcp-{}.txt", std::process::id()));
        std::fs::write(&notes, "x").unwrap();
        assert!(Rec::from(&json!({ "path": notes })).is_err());
        let _ = std::fs::remove_file(notes);
    }

    #[test]
    fn split_ids_name_the_splits_before_the_call() {
        let mut edit = RecordingEdit::default();
        edit.insert(Split::confirmed(100));
        edit.insert(Split::suggested(200, 2.0));
        assert_eq!(split_ids(&json!({ "remove": [1, 0] }), "remove", &edit).unwrap(), [200, 100]);
        assert!(split_ids(&json!({ "remove": [2] }), "remove", &edit).is_err());
        assert!(split_ids(&json!({}), "remove", &edit).unwrap().is_empty());
    }
}
