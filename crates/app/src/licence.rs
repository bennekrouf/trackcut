//! The TrackCut Pro licence on this computer: the licence number, kept in the config
//! folder with api0's last answer about it, and the calls that ask api0 again. What the
//! answer unlocks is `trackcut_core::license`.
//!
//! ```text
//! GET  https://gateway.api0.ai/v1/licence          the plan, its apps, credits left
//! POST https://gateway.api0.ai/v1/licence/portal   { url } to change or cancel it
//! Authorization: Bearer <licence>
//! X-Client-App: splitter
//! ```

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use trackcut_core::license::Status;
use trackcut_core::license::{self, Answer, FreeError, Saved, Shared, PRODUCT};

/// Where "Buy TrackCut Pro" leads.
pub const BUY_URL: &str = "https://mayorana.ch/en/apps/trackcut";

const GATEWAY: &str = "https://gateway.api0.ai";

fn file() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("splitter").join("licence.json"))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn load_own() -> Option<Saved> {
    serde_json::from_str(&std::fs::read_to_string(file()?).ok()?).ok()
}

/// The licence file every Mayorana app shares: `~/Library/Application Support/mayorana/licence`
/// on a Mac, `%APPDATA%\mayorana\licence` on Windows, `~/.config/mayorana/licence` on Linux.
fn shared_file() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("mayorana").join("licence"))
}

fn load_shared() -> Shared {
    match shared_file().map(std::fs::read_to_string) {
        Some(Ok(text)) => license::shared_from(&text),
        _ => Shared::Missing,
    }
}

/// Writes `text` as the shared licence: a number, or nothing once removed.
fn save_shared(text: &str) {
    let Some(f) = shared_file() else { return };
    // Named after this app, so two apps writing at once don't share it.
    let tmp = f.with_extension(format!("{PRODUCT}.tmp"));
    let _ = std::fs::create_dir_all(f.parent().unwrap()).and_then(|_| std::fs::write(&tmp, text)).and_then(|_| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, &f)
    });
}

/// This app's licence, or the one typed in another Mayorana app.
fn load() -> Option<Saved> {
    let shared = load_shared();
    if shared == Shared::Removed {
        if let Some(f) = file() {
            let _ = std::fs::remove_file(f);
        }
    }
    let (saved, share) = license::reconcile(shared, load_own());
    if let (Some(saved), true) = (&saved, share) {
        save_shared(&saved.licence);
    }
    saved
}

fn save(saved: &Saved) -> Result<(), String> {
    let f = file().ok_or("No settings folder to keep the licence in.")?;
    let json = serde_json::to_vec_pretty(saved).map_err(|e| e.to_string())?;
    // Written beside the target and renamed over it, so a crash can't leave half a file.
    let tmp = f.with_extension("tmp");
    std::fs::create_dir_all(f.parent().unwrap())
        .and_then(|_| std::fs::write(&tmp, json))
        .and_then(|_| std::fs::rename(&tmp, &f))
        .map_err(|e| format!("The licence couldn't be saved: {e}"))
}

/// The licence as of the last check, without asking api0.
pub fn current() -> Status {
    license::status_at(load().as_ref(), now())
}

/// Why a check didn't give an answer.
enum CheckError {
    /// api0 doesn't know this licence number.
    Unknown,
    /// Offline, api0 down, or an answer that made no sense.
    Unreachable(String),
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn ask(licence: &str) -> Result<Answer, CheckError> {
    let mut resp = agent()
        .get(format!("{GATEWAY}/v1/licence"))
        .header("Authorization", format!("Bearer {licence}"))
        .header("X-Client-App", PRODUCT)
        .call()
        .map_err(|e| CheckError::Unreachable(e.to_string()))?;
    match resp.status().as_u16() {
        200 => {
            let body = resp.body_mut().read_to_string().map_err(|e| CheckError::Unreachable(e.to_string()))?;
            serde_json::from_str(&body).map_err(|e| CheckError::Unreachable(e.to_string()))
        }
        401 => Err(CheckError::Unknown),
        code => Err(CheckError::Unreachable(format!("api0 answered {code}"))),
    }
}

/// Checks `typed` with api0 and, if api0 knows it, saves it. Blocking: call it off the UI
/// thread.
pub fn activate(typed: &str) -> Result<Status, String> {
    let licence = license::normalise(typed)
        .ok_or("This isn't a licence number. It has 20 letters and digits, like ABCDE-FGHJK-LMNPQ-RSTUV.")?;
    let answer = match ask(&licence) {
        Ok(a) => a,
        Err(CheckError::Unknown) => return Err("This licence number isn't known. Check it against the email.".into()),
        Err(CheckError::Unreachable(e)) => {
            return Err(format!("The licence couldn't be checked ({e}). Check the connection and try again."))
        }
    };
    let saved = Saved { licence, answer: Some(answer), checked: now() };
    save(&saved)?;
    save_shared(&saved.licence);
    Ok(license::status_at(Some(&saved), now()))
}

/// Checks the saved licence with api0 again, when `force`d or a day after the last check,
/// and keeps the answer. Offline, the last answer stands. Blocking.
pub fn refresh(force: bool) -> Status {
    let Some(mut saved) = load() else { return Status::Free };
    if !force && !license::check_due(&saved, now()) {
        return license::status_at(Some(&saved), now());
    }
    match ask(&saved.licence) {
        Ok(answer) => {
            saved.answer = Some(answer);
            saved.checked = now();
            let _ = save(&saved);
        }
        Err(CheckError::Unknown) => {
            saved.answer = None;
            let _ = save(&saved);
        }
        Err(CheckError::Unreachable(_)) => {}
    }
    license::status_at(Some(&saved), now())
}

/// The address of the page where the subscription is changed or cancelled. Blocking.
pub fn portal_url(licence: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Portal {
        url: String,
    }
    let failed = |e: String| format!("The subscription page couldn't be opened ({e}).");
    let mut resp = agent()
        .post(format!("{GATEWAY}/v1/licence/portal"))
        .header("Authorization", format!("Bearer {licence}"))
        .header("X-Client-App", PRODUCT)
        .send_empty()
        .map_err(|e| failed(e.to_string()))?;
    if resp.status().as_u16() != 200 {
        return Err(failed(format!("api0 answered {}", resp.status().as_u16())));
    }
    let body = resp.body_mut().read_to_string().map_err(|e| failed(e.to_string()))?;
    serde_json::from_str::<Portal>(&body).map(|p| p.url).map_err(|e| failed(e.to_string()))
}

/// `refresh`, off the UI thread.
pub async fn refresh_async(force: bool) -> Status {
    tokio::task::spawn_blocking(move || refresh(force)).await.unwrap_or_else(|_| current())
}

/// `activate`, off the UI thread.
pub async fn activate_async(typed: String) -> Result<Status, String> {
    tokio::task::spawn_blocking(move || activate(&typed)).await.map_err(|e| e.to_string())?
}

/// `portal_url`, off the UI thread.
pub async fn portal_url_async(licence: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || portal_url(&licence)).await.map_err(|e| e.to_string())?
}

/// Asks api0 to email `email` a licence for the free allowance of AI credits. The licence
/// number itself only goes by email. Blocking.
pub fn request_free_credits(email: &str) -> Result<String, FreeError> {
    if !license::plausible_email(email) {
        return Err(FreeError::Refused("Enter the email the licence should go to.".into()));
    }
    let failed = |e: String| FreeError::Refused(format!("The free AI credits couldn't be requested ({e})."));
    let mut resp = agent()
        .post(format!("{GATEWAY}/api/plans/free"))
        .header("X-Client-App", PRODUCT)
        .header("Content-Type", "application/json")
        .send(serde_json::json!({ "email": email.trim() }).to_string())
        .map_err(|e| failed(e.to_string()))?;
    let status = resp.status().as_u16();
    let body = resp.body_mut().read_to_string().unwrap_or_default();
    license::free_reply(status, &body)
}

/// `request_free_credits`, off the UI thread.
pub async fn request_free_credits_async(email: String) -> Result<String, FreeError> {
    tokio::task::spawn_blocking(move || request_free_credits(&email))
        .await
        .map_err(|e| FreeError::Refused(e.to_string()))?
}

/// Opens `url` in the default browser.
pub fn open_url(url: &str) {
    use std::process::Command;
    let _ = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else if cfg!(windows) {
        Command::new("cmd").args(["/c", "start", "", url]).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
}

/// Removes the licence from this computer (to move it to another one), for every Mayorana
/// app on it.
pub fn deactivate() {
    if let Some(f) = file() {
        let _ = std::fs::remove_file(f);
    }
    save_shared("");
}
