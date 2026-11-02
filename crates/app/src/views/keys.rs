//! The keyboard on the Tracks pane: playing, moving the playhead, splitting, and editing
//! tracks, unless a text field or a dialog has the keyboard.

use super::*;

pub(super) fn on_key(app: state::App, pane: Signal<Pane>, e: KeyboardEvent) {
    if *pane.peek() != Pane::Tracks
        || app.paste.peek().is_some()
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
}
