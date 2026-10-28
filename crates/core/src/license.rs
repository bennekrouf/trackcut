//! TrackCut Pro: what a subscription licence unlocks, from api0's last answer about it.
//!
//! Pro comes with a monthly plan bought on mayorana.ch. The plan's licence number
//! (`ABCDE-FGHJK-LMNPQ-RSTUV`, emailed when it is bought) is checked with api0
//! (`GET https://gateway.api0.ai/v1/licence`) when TrackCut starts and then daily; the
//! answer names the plan, the apps it makes Pro, whether it is paid up and the AI credits
//! left. The app keeps the last answer, so Pro lasts [`GRACE`] after the last check that
//! got through. The network and the file are the app's; this is the rule.
//!
//! The rule every mayorana app shares is `desktop_kit::licence`, re-exported here; what is
//! TrackCut's own is the product id and the free version's track limit.

pub use desktop_kit::licence::{
    check_due, free_reply, normalise, plausible_email, reconcile, shared_from, Answer, Credits, FreeError, Plan, Saved,
    Shared, Status, CHECK_EVERY, GRACE,
};

/// The product id api0 knows TrackCut by. The app was called Splitter when plans were
/// first set up, so the id stays.
pub const PRODUCT: &str = "splitter";

/// How many tracks of one recording the free version exports. Splitting and reviewing
/// are never limited: a free user sees every track found, and Pro exports them all.
pub const FREE_TRACKS: usize = 10;

/// How many of a recording's `kept` tracks an export writes.
pub fn exported_tracks(kept: usize, pro: bool) -> usize {
    if pro {
        kept
    } else {
        kept.min(FREE_TRACKS)
    }
}

/// What `saved` amounts to for TrackCut at `now` (seconds since 1970).
pub fn status_at(saved: Option<&Saved>, now: u64) -> Status {
    desktop_kit::licence::status_at(saved, now, PRODUCT)
}

/// What TrackCut makes of a licence status.
pub trait StatusExt {
    /// What the window title adds after "TrackCut <version>".
    fn title_badge(&self) -> &'static str;
    /// Whether exports write every track.
    fn exports_all(&self) -> bool;
}

impl StatusExt for Status {
    fn title_badge(&self) -> &'static str {
        self.title_badge_for(PRODUCT)
    }

    fn exports_all(&self) -> bool {
        self.is_pro()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_free_version_exports_the_first_ten_tracks() {
        assert_eq!(exported_tracks(6, false), 6);
        assert_eq!(exported_tracks(10, false), 10);
        assert_eq!(exported_tracks(18, false), 10);
        assert_eq!(exported_tracks(18, true), 18);
    }

    #[test]
    fn pro_is_for_a_paid_plan_that_lists_splitter() {
        let answer = Answer { products: vec!["splitter".into()], active: true, ..Answer::default() };
        let saved = Saved { licence: "ABCDE-FGHJK-LMNPQ-RSTUV".into(), answer: Some(answer), checked: 1_000 };
        assert!(status_at(Some(&saved), 1_000).exports_all());
        assert!(!Status::Free.exports_all());
    }
}
