//! How to use TrackCut from Claude: what to paste, and where.
//!
//! The command is this app's own executable, read at runtime rather than written down, so
//! the snippet is right wherever it was installed — and in a development build, which is
//! what someone trying it from a checkout runs.

use crate::state;
use dioxus::prelude::*;

#[component]
pub fn McpDialog() -> Element {
    let app = use_context::<state::App>();
    if !(app.mcp_dialog)() {
        return rsx! {};
    }
    rsx! {
        desktop_kit::ui::ConnectWindow {
            lead: "Ask Claude to split your recordings: find the tracks, confirm or move splits, name the tracks \
                   from a tracklist, leave out the talk between songs, export, or download a recording from a \
                   link. It edits the same cutlists as this window, and what it changes shows up here within a \
                   second.",
            try_it: "\u{201c}What\u{2019}s in my recordings folder?\u{201d}, or \u{201c}Split live.mp3, name the tracks \
                     from this setlist and export them as MP3.\u{201d}",
            on_close: move |_| {
                let mut dialog = app.mcp_dialog;
                dialog.set(false);
                app.focus_root();
            },
        }
    }
}
