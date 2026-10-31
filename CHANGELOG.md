# Changelog

What changed in each release of **TrackCut**, the keyboard-driven tool for
splitting long recordings into tracks.

The public version of this page — with the download for each release — lives at
<https://mayorana.ch/en/apps/trackcut/releases>. It is generated from this file
by `scripts/changelog_to_json.py`, so this file is the only place a release note
is written.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Each heading is dated on the day its tag was pushed. Releases that carried only
build or packaging work say so rather than being hidden: the version numbers a
user sees in the update prompt should all be accounted for.

## [0.1.45] - 2026-10-31

### Added

- Added detailed documentation and trigger result files for the TrackCut skill,
  providing guidance on how to split long recordings into tracks and manage
  various audio processing tasks.

## [0.1.44] - 2026-10-30

### Added

- Added detailed documentation and trigger result files for the TrackCut skill,
  providing guidance on how to split long recordings into tracks and manage
  various audio processing tasks.

## [0.1.43] - 2026-10-30

### Changed

- Packaging only — no user-visible change.

## [0.1.42] - 2026-10-29

### Added

- Reduce noise, in the export bar beside the loudness choice: light, medium or
  strong. It takes steady background noise (hiss, hum, room tone) out of the
  exported tracks. TrackCut learns the noise from the recording's quietest
  moments, so the music or voice over it keeps its level. A/B lets you hear
  the result before you export. A video's clips get it too. It needs a
  re-encoding format: Original copies the sound exactly as it is.

## [0.1.41] - 2026-10-28

### Added

- TrackCut now shows the occasional message from mayorana.ch, under the bar at
  the top, until you close it.

### Changed

- The update banner and the occasional message from mayorana.ch now sit just
  under the bar at the top of the window instead of covering it, and look the
  same in every mayorana app. Download opens the new build in your browser and
  shows its SHA-256 when one is published, to compare with the file you
  downloaded.
- The Pro window looks and reads the same in every mayorana app. Once you're Pro
  it names your plan and email, and pressing Enter in the licence number field
  activates it.
## [0.1.40] - 2026-10-28

### Changed

- Packaging only — no user-visible change.

## [0.1.39] - 2026-10-28

### Changed

- The bar across the top of the window now looks and works the same in every
  mayorana app: the name and version on the left, and on the right Open folder…,
  URL…, MCP, Feedback, the light/dark switch and the Pro badge in the corner.
- Open folder…, URL…, MCP, Feedback and Pro moved there from the top of the file
  list, which now starts with your files. The Pro badge opens the licence, as
  clicking the name used to.

## [0.1.38] - 2026-10-28

### Fixed

- A few rare failures that used to close TrackCut now show as an error
  instead: a recording that can't be sought in, a download whose output
  can't be read, and the files it writes in a folder it can't create.

## [0.1.37] - 2026-10-27

### Added

- Added support for Codex MCP: a new step in the setup dialog shows the command
  to register TrackCut as a Codex server.

## [0.1.36] - 2026-10-27

### Changed

- Packaging only — no user-visible change.

## [0.1.35] - 2026-10-25

### Fixed

- Installing silently, as IT does for an unattended update with `/VERYSILENT`,
  no longer hangs when TrackCut is already installed: setup waited on a
  question nobody could see. It now replaces the installed version without
  asking.

## [0.1.34] - 2026-10-25

### Changed

- Activating a licence now shows a small spinner while it is checked, then
  a window that says you're Pro, with the AI credits left and the renewal
  date in small type below. Removing the licence and checking it again no
  longer sit beside it; Manage subscription is a small link. Press Esc or
  the × to close it.

### Fixed

- On Windows, downloading from a link whose title has accents, emoji or
  non-Latin letters no longer fails with "yt-dlp stopped without producing
  a file": the file was saved, but TrackCut couldn't find it to open it.

## [0.1.33] - 2026-10-23

### Changed

- The new-version banner and the usage-statistics notice now look the same
  as in GitAgent: a blue bar across the top, or the bottom for the notice,
  with a Download link and a × to hide it.

## [0.1.32] - 2026-10-23

### Changed

- Dialogs now have a title bar with a × to close them, and text fields have
  the same size and typeface as in GitAgent.
- An open dialog now dims the light/dark switch in the corner along with the
  rest of the window, instead of leaving it lit on top.
- The buttons along the top of the window now all have the same height,
  borders included, and the light/dark switch matches them.

### Fixed

- On Windows, downloading from a URL now works on networks that inspect
  HTTPS traffic, such as company networks behind Zscaler: the downloader now
  trusts the certificates Windows trusts.

## [0.1.31] - 2026-10-23

### Changed

- Buttons now have the same size, rounded corners and typeface as in
  GitAgent, so they look alike across the Mayorana apps.

### Fixed

- The Apply cuts button next to a long file name no longer squeezes onto
  three lines; the name shortens instead.

## [0.1.30] - 2026-10-23

### Fixed

- On company networks that inspect secure connections (Zscaler and similar
  proxies), activating TrackCut Pro failed with "invalid peer certificate:
  UnknownIssuer". TrackCut now trusts the certificates your computer trusts,
  as your browser does, so the licence check, update check and the yt-dlp,
  Deno and ffmpeg downloads work there too. If the proxy's certificate isn't
  installed on the computer, the message now says so and suggests asking IT
  to install it.

## [0.1.29] - 2026-10-22

### Changed

- The window title now shows your licence status next to the app name and
  version: a Pro marker when Pro is active, or Pro inactive when it has lapsed.
  The selected recording still appears after a dash.

## [0.1.28] - 2026-10-22

### Changed

- One licence number for every Mayorana app on this computer. Paste it in any
  of them and the others are Pro too the next time they start, with nothing to
  paste again; a number already pasted in one app is shared the same way.
  Removing the licence in the Pro window removes it from all of them.

## [0.1.27] - 2026-10-21

### Added

- Free AI credits every month, without Pro. Enter your email under Get free
  AI credits in the Pro window and the licence number arrives by email; paste
  it in the same window. The window then shows how many credits are left this
  month. The free version's limits stay as they are.

## [0.1.26] - 2026-10-21

### Changed

- TrackCut Pro now comes with a monthly plan on mayorana.ch instead of a
  one-time licence key. Click TrackCut at the top of the file list and paste
  the licence number from the plan's email (like ABCDE-FGHJK-LMNPQ-RSTUV).
  TrackCut checks it when it starts and once a day, and Pro keeps working for
  a week without a connection.
- The Pro window shows your plan, the AI credits left this month and when the
  plan renews. Manage subscription opens the page where you change or cancel
  it.

## [0.1.25] - 2026-10-20

### Added

- A Feedback link beside the version number opens the contact form with
  TrackCut and your version already filled in, so an idea or a problem
  reaches us in two clicks.

## [0.1.24] - 2026-10-19

### Changed

- Packaging only — no user-visible change.

## [0.1.23] - 2026-10-19

### Fixed

- On Windows, TrackCut no longer fails to start with "VCRUNTIME140_1.dll was
  not found" on PCs without the Microsoft Visual C++ Redistributable. Nothing
  extra needs to be installed any more.

## [0.1.22] - 2026-10-18

### Added

- Use TrackCut from Claude. The new **MCP** button at the top of the file list
  shows what to paste into Claude Desktop, or the one line to run for Claude
  Code, already pointing at where TrackCut is installed. Claude can then list
  a folder's recordings, find a recording's tracks, confirm, move, add or
  remove splits, re-run silence detection with other settings, name the
  tracks or paste a tracklist, leave tracks out, mark a recording done,
  export it, apply the cuts, and download a recording from a link. What
  Claude changes in the folder open in the window shows up there within a
  second.

## [0.1.21] - 2026-10-16

### Added

- Light mode. TrackCut now matches your computer's light or dark setting and
  follows it when it changes. The sun/moon button at the top right switches
  between the two. The video frame stays black in both.

## [0.1.20] - 2026-10-14

### Changed

- Splitter is now called TrackCut. Your Pro licence, settings, downloaded
  tools and the edits saved in your folders all carry over: a folder's
  `splitter.cutlist.json` is still read, and becomes `trackcut.cutlist.json`
  the first time you change something in it. On macOS the update installs as
  TrackCut.app; once it runs, you can delete Splitter.app from Applications.

## [0.1.19] - 2026-10-07

### Changed

- TrackCut now uses the PolyForm Noncommercial License 1.0.0 and has updated
  branding. This means the software is free for personal use, but commercial use
  requires a license. The company name has changed to Mayorana, and the
  copyright notice has been updated accordingly.

## [0.1.18] - 2026-10-06

### Added

- TrackCut now shares anonymous usage statistics: whether it is installed and
  opened, its version and your operating system. It is on by default; a note at
  the bottom of the window tells you once, and nothing is sent before you have
  seen it. **Turn off** there stops it for good and deletes anything not yet
  sent. Your files, data and accounts are never part of it. It also stays off if
  `DO_NOT_TRACK`, `DISABLE_UPDATE_CHECK` or `MAYORANA_NO_TELEMETRY` is set.

## [0.1.17] - 2026-10-05

### Changed

- Packaging only — no user-visible change.

## [0.1.16] - 2026-10-05

### Added

- Videos without sound can be cut. They open like any other video, with the
  picture shown and a flat timeline, and you place every split yourself with
  **M**, since there is no silence to find. Their clips, and the copy that
  **Apply cuts** makes, are picture only.
- **URL…** with **Video** now downloads videos posted without sound, as is
  common on X, instead of failing. Asking for **Audio** on such a video now
  says to pick **Video** instead.

## [0.1.15] - 2026-09-28

### Changed

- Packaging only — no user-visible change.

## [0.1.14] - 2026-09-27

### Added

- The version you are running now shows next to the TrackCut name at the top
  of the sidebar and in the window title, so it is at hand when you report a
  problem or check whether an update installed.
- A **Get Pro…** button at the top of the file list opens the licence window,
  where you buy TrackCut Pro or paste your key. It reads **Pro ✓** once your
  licence is active. Clicking **TrackCut** still opens the same window.

## [0.1.13] - 2026-09-27

### Changed

- Without TrackCut Pro, an export now writes the first 10 tracks of each
  recording. Splitting, reviewing and naming are not limited: when a recording
  has more tracks, the export bar says how many and **TrackCut Pro exports all**
  opens the licence window. A vinyl side, an EP or a short set still fits in
  the free version; with Pro, every track is exported, as before.

## [0.1.12] - 2026-09-27

### Added

- TrackCut Pro licences. Click **TrackCut** at the top of the file list to
  paste the licence key from your purchase email. The key is checked on your
  computer, with no account and nothing sent anywhere, and a **Pro** badge
  shows once it is active. **Remove from this computer** frees it for another
  one.

## [0.1.11] - 2026-09-27

### Changed

- Packaging only — no user-visible change.

## [0.1.10] - 2026-09-25

### Changed

- Packaging only — no user-visible change.

## [0.1.9] - 2026-09-24

### Changed

- Packaging only — no user-visible change.

## [0.1.8] - 2026-09-24

### Changed

- Packaging only — no user-visible change.

## [0.1.7] - 2026-09-24

### Fixed

- The video preview works on Windows web views that refuse to load media from
  a local address. When that happens the preview falls back to the app's own
  channel, so the picture still shows instead of an error.

### Changed

- The notes for every release, with its download, are now published at
  [mayorana.ch/en/apps/trackcut/releases](https://mayorana.ch/en/apps/trackcut/releases).

## [0.1.6] - 2026-09-24

### Added

- A download log in the sidebar: what yt-dlp printed for the last URL download,
  kept after it ends. It opens by itself when a download fails, so the reason
  can be read instead of guessed.

## [0.1.5] - 2026-09-24

### Added

- Videos: MP4, M4V and MOV files with AAC audio are listed next to the
  recordings and split on their sound, with the picture shown where a recording
  shows its overview waveform.
- Video export: one MP4 clip per kept track, cut exactly on the splits, with
  title, album and track number. The clips are re-encoded by ffmpeg, which is
  fetched on first use (about 30 MB, SHA-256 checked) and never bundled.
- **URL…** can download the video instead of only its audio: picture and sound
  are fetched separately and merged into one MP4.

## [0.1.4] - 2026-09-24

### Added

- An update banner: the app checks mayorana.ch for a newer version and links
  straight to the download for the platform it is running on.

### Changed

- First release with a macOS build, signed with an Apple Developer ID and
  notarized.

## [0.1.3] - 2026-09-24

### Added

- Silence at track edges is cut from the export by default. Detection now tags
  every quiet stretch, lead-in and tail included; where one touches a track's
  edge it is trimmed, keeping 0.25 s on each side so quiet attacks and fades
  survive. **G** keeps a silence or cuts it again. Splits don't move, and the
  track list shows each track's length after the cut.

### Fixed

- Pressing Enter in the **URL…** dialog no longer crashes the app.

## [0.1.2] - 2026-09-24

### Added

- First downloadable release. Silence detection suggests the splits, you review
  each one by ear from the keyboard, then export: MP3 without re-encoding
  (gapless), or MP3/FLAC/WAV with EBU R128 loudness, ReplayGain tags and
  optional normalisation. Titles, a batch export queue and A/B listening
  included.

### Changed

- **URL…** downloads through yt-dlp and Deno instead of ffmpeg. Both are fetched
  on first use and checked against their published SHA-256 sums, and yt-dlp
  updates itself at most once a day. The AAC or MP3 stream is fetched directly
  and AAC is decoded to WAV, so nothing needs installing.

## [0.1.1] - 2026-09-24

### Changed

- Tagged, but the release build failed and nothing was published. The first
  downloadable version is 0.1.2.
