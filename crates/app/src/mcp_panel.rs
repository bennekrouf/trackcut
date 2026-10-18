//! How to use TrackCut from Claude: what to paste, and where.
//!
//! The command is this app's own executable, read at runtime rather than written down, so
//! the snippet is right wherever it was installed — and in a development build, which is
//! what someone trying it from a checkout runs.

use crate::state;
use dioxus::prelude::*;

/// This executable, as a client should start it.
fn command() -> String {
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "trackcut".into())
}

/// The `mcpServers` entry for Claude Desktop's config file. Built with serde so a Windows
/// path's backslashes arrive escaped.
pub fn desktop_config(command: &str) -> String {
    let config = serde_json::json!({
        "mcpServers": { "trackcut": { "command": command, "args": ["mcp"] } }
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

/// The one line that adds it to Claude Code, for every project.
pub fn code_command(command: &str) -> String {
    format!("claude mcp add trackcut --scope user -- \"{command}\" mcp")
}

/// Where Claude Desktop keeps its config, for the person who would rather open it themselves
/// than go through the app's menu.
fn desktop_config_path() -> &'static str {
    if cfg!(target_os = "windows") {
        "%APPDATA%\\Claude\\claude_desktop_config.json"
    } else if cfg!(target_os = "macos") {
        "~/Library/Application Support/Claude/claude_desktop_config.json"
    } else {
        "~/.config/Claude/claude_desktop_config.json"
    }
}

#[component]
pub fn McpDialog() -> Element {
    let app = use_context::<state::App>();
    if !(app.mcp_dialog)() {
        return rsx! {};
    }
    let command = command();
    let desktop = desktop_config(&command);
    let code = code_command(&command);
    let close = move || {
        let mut dialog = app.mcp_dialog;
        dialog.set(false);
        app.focus_root();
    };

    rsx! {
        div { class: "modal-backdrop", onclick: move |_| close(),
            div {
                class: "modal modal-wide",
                tabindex: "-1",
                onclick: move |e| e.stop_propagation(),
                onmounted: move |e| async move {
                    let _ = e.set_focus(true).await;
                },
                onkeydown: move |e| {
                    if e.key() == Key::Escape {
                        close();
                    }
                },
                h2 { "Use TrackCut from Claude" }
                p { class: "dim",
                    "Ask Claude to split your recordings: find the tracks, confirm or move splits, name the \
                     tracks from a tracklist, leave out the talk between songs, export, or download a \
                     recording from a link. It edits the same cutlists as this window, and what it changes \
                     shows up here within a second."
                }

                div { class: "mcp-step",
                    div { class: "mcp-step-title", span { class: "mcp-num", "1" } "Claude Desktop" }
                    p { class: "dim",
                        "Settings \u{2192} Developer \u{2192} Edit Config, paste this in and save, then quit \
                         Claude and open it again. If the file already has an "
                        code { "mcpServers" }
                        " section, add just the "
                        code { "trackcut" }
                        " entry to it."
                    }
                    Snippet { text: desktop.clone() }
                    p { class: "dim mcp-path", "The file: {desktop_config_path()}" }
                }

                div { class: "mcp-step",
                    div { class: "mcp-step-title", span { class: "mcp-num", "2" } "Claude Code" }
                    p { class: "dim", "Run this once in a terminal:" }
                    Snippet { text: code.clone() }
                }

                div { class: "mcp-step",
                    div { class: "mcp-step-title", span { class: "mcp-num", "3" } "Try it" }
                    p { class: "dim",
                        "Ask: \u{201c}What\u{2019}s in my recordings folder?\u{201d}, or \u{201c}Split live.mp3, name \
                         the tracks from this setlist and export them as MP3.\u{201d}"
                    }
                }

                p { class: "mcp-note",
                    "Claude in the browser (claude.ai) can\u{2019}t use this: it only connects to servers on the \
                     internet, and TrackCut runs on your computer. Any other app that speaks MCP can, with the \
                     same command: "
                    code { "{command} mcp" }
                }

                div { class: "modal-actions",
                    button { class: "primary", onclick: move |_| close(), "Done" }
                }
            }
        }
    }
}

/// A block of text to paste somewhere else, with its own Copy button.
#[component]
fn Snippet(text: String) -> Element {
    let mut copied = use_signal(|| false);
    let to_copy = text.clone();
    rsx! {
        div { class: "mcp-snippet-wrap",
            pre { class: "mcp-snippet", "{text}" }
            button {
                class: "mcp-copy",
                onclick: move |_| {
                    // The native clipboard: the web view's needs a secure origin and a user
                    // gesture check that differ per platform.
                    let ok = arboard::Clipboard::new().and_then(|mut c| c.set_text(to_copy.clone())).is_ok();
                    copied.set(ok);
                },
                if copied() { "Copied ✓" } else { "Copy" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_entry_starts_this_app_in_mcp_mode() {
        let config: serde_json::Value =
            serde_json::from_str(&desktop_config("/Applications/TrackCut.app/Contents/MacOS/trackcut")).unwrap();
        let entry = &config["mcpServers"]["trackcut"];
        assert_eq!(entry["command"], "/Applications/TrackCut.app/Contents/MacOS/trackcut");
        assert_eq!(entry["args"], serde_json::json!(["mcp"]));
    }

    #[test]
    fn a_windows_path_survives_the_trip_through_json() {
        let path = r"C:\Program Files\TrackCut\trackcut.exe";
        let config: serde_json::Value = serde_json::from_str(&desktop_config(path)).unwrap();
        assert_eq!(config["mcpServers"]["trackcut"]["command"], path);
    }

    #[test]
    fn the_code_command_quotes_a_path_with_spaces() {
        assert_eq!(
            code_command("/Apps/Track Cut/trackcut"),
            "claude mcp add trackcut --scope user -- \"/Apps/Track Cut/trackcut\" mcp"
        );
    }
}
