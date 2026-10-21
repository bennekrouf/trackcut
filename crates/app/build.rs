//! Build script: embeds the Windows app icon into the .exe resource.
//!
//! Looks for `assets/icon.ico` (release CI generates it from `assets/icon.png`). If
//! present, the icon is compiled into the executable so it shows in the taskbar, Start
//! menu and Explorer. If absent, the build still succeeds, icon-less, with a
//! `cargo:warning` so the gap is visible in the build log.
//!
//! The Windows part is a no-op on other targets.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(target_os = "windows")]
    {
        let icon_path = std::path::Path::new("assets/icon.ico");
        if !icon_path.exists() {
            println!(
                "cargo:warning=assets/icon.ico not found — the .exe will ship without an embedded icon. \
                 Generate it from assets/icon.png (see the release workflow)."
            );
            return;
        }

        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("FileDescription", "TrackCut");
        res.set("ProductName", "TrackCut");
        res.set("CompanyName", "Mayorana");
        res.set("LegalCopyright", "© 2026 Mayorana");
        if let Err(e) = res.compile() {
            // rc.exe / windres isn't on every Windows runner; warn and ship icon-less rather
            // than failing the build.
            println!("cargo:warning=Failed to embed Windows icon resource: {e} (the build continues without it)");
        }
    }
}
