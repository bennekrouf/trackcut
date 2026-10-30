---
name: trackcut
description: Drive TrackCut, the desktop app that splits long audio recordings (MP3, WAV, or the sound of an MP4 or MOV video) into tracks at silences and exports one tagged file per track, through its MCP tools. Use this whenever the user mentions TrackCut, or asks to split a concert, DJ set, album rip, podcast, lecture, vinyl or tape digitisation into tracks, name tracks from a tracklist, leave talk or applause out, remove silences from a recording, normalise loudness or reduce noise on export, or export tracks as FLAC, WAV or MP3. Also use it when TrackCut's tools are missing and the user wants to connect the app.
---

# TrackCut

TrackCut is a desktop app (macOS, Windows, Linux) that splits long recordings
into tracks. It suggests splits at silences, the user reviews them, and it
exports one file per kept track with title, album and track-number tags.
Videos are split on their sound and exported as video clips. A folder's edits
are kept in the folder itself, so recordings are named by their absolute path.

The person you are helping has long recordings and wants clean, named tracks
without scrubbing through hours of audio.

## Before you start

The tools come from TrackCut's own MCP server (`trackcut mcp`, over stdio).
If no tool named `list_recordings` from a `trackcut` server is available, the
app is not connected:

- Tell the user TrackCut is a separate desktop app, from
  <https://mayorana.ch/en/apps/trackcut>, and that this skill only works with
  it installed. Do not download or install it for them.
- Once installed, the **MCP** button in the sidebar header shows the exact
  config for Claude Desktop and the command for Claude Code and Codex.
  `references/connect.md` has the steps per client.

## How to work with it

1. **`list_recordings`** lists the recordings in a folder (by default the one
   open in the TrackCut window), with their status.
2. **`get_recording`** shows a recording's splits and tracks. The first time,
   it scans the file and suggests splits at silences, which takes a few
   seconds for a long file. Times are in seconds.
3. **Review the splits.** Suggested splits are unreviewed. Tell the user where
   they fall (as mm:ss, with the track lengths they make) and let them
   confirm, or confirm them when asked. `edit_splits` confirms, moves, removes
   and adds splits in one call; added and moved splits snap to the quietest
   point nearby unless `snap: false`.
   - Too many splits (a quiet passage inside a song)? `detect` again with a
     longer `min_silence_secs` or `min_track_secs`.
   - Too few (a noisy room, applause between songs)? `detect` with a higher
     `threshold_db`, such as -35.
   Confirmed splits are kept when detecting again.
4. **Name the tracks.** `paste_tracklist` takes a tracklist, one title per
   line in order ("1. Intro (4:32)" works; numbers and times are removed).
   `set_tracks` sets titles one by one and leaves tracks out of the export
   (`keep: false`) for talk, tuning or applause.
   If the track count and the tracklist don't match, say so and help find
   the missing or extra split before exporting.
5. **`set_status`** marks a recording to do, in progress, done or flagged.

## Writing files: ask first

Recordings are the user's own files. Ask before each of these:

- **`export`** writes one file per kept track into a folder named after the
  recording, next to it. Format (`profile`): `original` (a copy, no
  re-encoding), `flac`, `wav16`, or MP3 (`mp3-v0` … `mp3-128`). `normalize`
  sets loudness (`album:-14` keeps one gain for the whole set, `track:-14`
  levels each track) and `denoise` takes out steady hiss or hum; both need a
  re-encoding profile. The free version writes the first 10 tracks of a
  recording; TrackCut Pro writes them all. Say so before a long export.
- **`apply_cuts`** writes a cleaned copy without cut silence and left-out
  tracks, next to the original, which is not touched.
- **`download`** fetches a recording from a URL into
  `~/Music/TrackCut/Downloads` (sound, or `video: true` for the MP4). Only
  for recordings the user has the right to download, such as their own
  uploads; the app installs its downloader on first use.

Suggest `flac` for archiving and `mp3-v0` for listening, unless the user
prefers otherwise.
