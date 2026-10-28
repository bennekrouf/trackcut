//! What TrackCut Pro adds: every track of a recording exported. The window,
//! the badge and the licence checks are desktop-kit's, the same in every
//! mayorana app; the rule is `trackcut_core::license`.

use dioxus::prelude::*;
use trackcut_core::license::FREE_TRACKS;

/// The badge's tooltip while TrackCut isn't Pro.
pub fn hint() -> String {
    format!("The free version exports the first {FREE_TRACKS} tracks of a recording; Pro exports them all")
}

/// What the free version does and what Pro adds, in the Pro window.
pub fn pitch() -> Element {
    rsx! {
        p {
            "The free version exports the first {FREE_TRACKS} tracks of each recording. "
            "TrackCut Pro exports all of them. It comes with a monthly plan on mayorana.ch."
        }
    }
}
