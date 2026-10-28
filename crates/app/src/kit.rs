//! Names TrackCut to desktop-kit, the code every mayorana desktop app shares (usage
//! statistics, the update check and the licence rule), and lends it TrackCut's HTTP client.

use std::future::Future;
use std::pin::Pin;

use desktop_kit::http::{Request, Response};

/// First thing in `main`: until then, nothing in desktop-kit records or sends.
pub fn init() {
    desktop_kit::init(desktop_kit::App {
        id: "trackcut",
        name: "TrackCut",
        version: env!("CARGO_PKG_VERSION"),
        // Still the app's old name: the folder holds the install's identity and the
        // person's answer, and a new folder would forget a "turn it off".
        telemetry_folder: "splitter",
        http_get,
    });
}

/// ureq blocks, so the request runs on a blocking thread. Certificates are checked by the
/// operating system, as for every TrackCut request (see `http`).
fn http_get(req: Request) -> Pin<Box<dyn Future<Output = Option<Response>> + Send>> {
    Box::pin(async move {
        tokio::task::spawn_blocking(move || {
            let agent: ureq::Agent =
                crate::http::config().timeout_global(Some(req.timeout)).http_status_as_error(false).build().into();
            let mut response = agent.get(&req.url).header("User-Agent", &req.user_agent).call().ok()?;
            let status = response.status().as_u16();
            let body = response.body_mut().read_to_string().ok()?;
            Some(Response { status, body })
        })
        .await
        .ok()?
    })
}
