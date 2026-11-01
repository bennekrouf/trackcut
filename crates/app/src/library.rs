//! TrackCut's recordings in the media library every Mayorana app shares: each recording of the
//! open folder with where it stands (polishing while it is cut, ready once its tracks are
//! exported) and its files, so PolishCast and PublishKit show it too. Read from the folder, so
//! it is right however a recording changed, here or through `trackcut mcp`.

use crate::exporting::export_dir_of;
use desktop_kit::library::{self as shared, Item, Kind, Role, Status};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use trackcut_core::cutlist::Cutlist;

fn id_of(path: &Path) -> String {
    format!("trackcut:{}", path.display())
}

/// The tracks exported for a recording, in its export folder.
fn tracks_of(path: &Path) -> Vec<PathBuf> {
    let Some(dir) = export_dir_of(path) else { return Vec::new() };
    let mut tracks: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| {
            r.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && Kind::of(p) == Some(Kind::Audio))
                .collect()
        })
        .unwrap_or_default();
    tracks.sort();
    tracks
}

/// A recording as the library shows it: ready once tracks are exported, polishing until then.
pub fn item_of(path: &Path) -> Option<Item> {
    if !path.is_file() {
        return None;
    }
    let title = path.file_stem()?.to_string_lossy().into_owned();
    let tracks = tracks_of(path);
    let (status, dated) = match tracks.iter().max_by_key(|t| modified(t)) {
        Some(newest) => (Status::Ready, newest.clone()),
        None => (Status::Polishing, path.to_path_buf()),
    };
    let kind = if trackcut_core::is_video(path) { Kind::Video } else { Kind::Audio };
    let mut item = Item::new(id_of(path), "trackcut", kind, title, status).with_file(path, Role::Source);
    for track in tracks {
        item = item.with_file(track, Role::Audio);
    }
    if let Some(at) = modified(&dated) {
        item.date = shared::day_of(at);
    }
    item.open = path.to_string_lossy().into_owned();
    Some(item)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The folder's recordings TrackCut has worked on (those with edits in its cutlist), and none
/// that are gone from it.
pub fn sync_folder(dir: &Path) {
    let edited: HashSet<String> = Cutlist::load(dir).map(|c| c.recordings.into_keys().collect()).unwrap_or_default();
    let mut keep = HashSet::new();
    for recording in trackcut_core::list_recordings(dir).unwrap_or_default() {
        if !edited.contains(&crate::state::key_of(&recording.path)) {
            continue;
        }
        if let Some(item) = item_of(&recording.path) {
            keep.insert(item.id.clone());
            let _ = shared::upsert(item);
        }
    }
    // Only this folder's: recordings of other folders stay as they were last seen.
    for item in shared::load_all() {
        let here = Path::new(&item.open).parent() == Some(dir);
        if item.app == "trackcut" && here && !keep.contains(&item.id) {
            let _ = shared::remove(&item.id);
        }
    }
}

/// Changes whenever a recording, an export folder or the cutlist changes.
pub fn stamp(dir: &Path) -> Vec<(PathBuf, Option<SystemTime>)> {
    let mut stamp: Vec<(PathBuf, Option<SystemTime>)> = std::fs::read_dir(dir)
        .map(|r| r.filter_map(|e| e.ok()).map(|e| (e.path(), e.metadata().and_then(|m| m.modified()).ok())).collect())
        .unwrap_or_default();
    stamp.sort();
    stamp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_is_ready_once_its_tracks_are_exported() {
        let dir = std::env::temp_dir().join(format!("trackcut-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Live set")).unwrap();
        let recording = dir.join("Live set.mp3");
        std::fs::write(&recording, b"sound").unwrap();
        let item = item_of(&recording).unwrap();
        assert_eq!((item.status, item.kind, item.title.as_str()), (Status::Polishing, Kind::Audio, "Live set"));

        std::fs::write(dir.join("Live set").join("01 Intro.mp3"), b"track").unwrap();
        std::fs::write(dir.join("Live set").join("notes.txt"), b"not a track").unwrap();
        let item = item_of(&recording).unwrap();
        assert_eq!(item.status, Status::Ready);
        assert_eq!(item.files.iter().map(|f| f.role).collect::<Vec<_>>(), [Role::Source, Role::Audio]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
