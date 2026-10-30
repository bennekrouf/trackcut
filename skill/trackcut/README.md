# TrackCut skill

A skill for AI agents that use **TrackCut**, the desktop app that splits long recordings (MP3, WAV, or a video's sound) into tracks at silences and exports one tagged file per track. The
skill tells the agent how to use TrackCut's tools well: which to call first, what
needs your yes, sensible defaults, and how to report back.

## What you need

- **TrackCut**, installed from <https://mayorana.ch/en/apps/trackcut> (macOS,
  Windows or Linux). This skill does not include the app.
- A desktop agent connected to TrackCut's MCP server: Claude Desktop, Claude
  Code, Codex CLI or Cursor. `references/connect.md` has the steps. Web-only
  agents such as ChatGPT can't reach an app on your computer.

## What you can ask for

- Suggest splits at silences and tune detection for a noisy room
- Confirm, move, add and remove splits with you
- Name tracks from a pasted tracklist; leave out talk and applause
- Export FLAC, WAV or MP3, with loudness and noise settings
- Write a cleaned copy without the silences

## What waits for your yes

- Every export into your folders
- Writing a cleaned copy
- Downloading a recording from a URL

## Limits

The free version exports the first 10 tracks of a recording; TrackCut Pro exports them all.

## Install

- **Claude Code**: unzip into `~/.claude/skills/` so the folder is
  `~/.claude/skills/trackcut/`.
- **claude.ai or Claude Desktop**: upload the zip under Skills in Settings.
- **Other agents**: put the `trackcut` folder where the agent reads skills.

## Help

Questions, licences and bug reports: <https://mayorana.ch/en/contact>.
