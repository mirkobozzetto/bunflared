//! Finds cloudflared, or fetches Cloudflare's own build on the first run.

use std::path::PathBuf;
use std::process::Stdio;

use tokio::process::Command;

use crate::os;

const RELEASES: &str = "https://github.com/cloudflare/cloudflared/releases/latest/download";
const DOWNLOADS_PAGE: &str = "https://developers.cloudflare.com/tunnel/downloads/";

fn exe() -> String {
    format!("cloudflared{}", std::env::consts::EXE_SUFFIX)
}

fn asset() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("cloudflared-darwin-arm64.tgz"),
        ("macos", "x86_64") => Some("cloudflared-darwin-amd64.tgz"),
        ("linux", "x86_64") => Some("cloudflared-linux-amd64"),
        ("linux", "aarch64") => Some("cloudflared-linux-arm64"),
        ("windows", "x86_64") => Some("cloudflared-windows-amd64.exe"),
        _ => None,
    }
}

fn own_copy() -> Option<PathBuf> {
    Some(os::data_dir()?.join("bin").join(exe()))
}

/// The one on PATH first, then the copy bunflared fetched earlier.
pub fn find() -> Option<PathBuf> {
    let on_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(exe()))
            .find(|path| path.is_file())
    });
    on_path.or_else(|| own_copy().filter(|path| path.is_file()))
}

pub fn install_hint() -> String {
    if cfg!(target_os = "macos") {
        "brew install cloudflared".into()
    } else {
        format!("see {DOWNLOADS_PAGE}")
    }
}

/// Downloads cloudflared from its GitHub releases with the system's curl,
/// which ships with macOS, Linux distributions and Windows 10 and later.
pub async fn fetch() -> Result<PathBuf, String> {
    let asset = asset().ok_or("Cloudflare publishes no build for this system")?;
    let target = own_copy().ok_or("no folder to keep it in")?;
    let dir = target
        .parent()
        .ok_or("no folder to keep it in")?
        .to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let download = dir.join(format!("{asset}.part"));
    run(Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&download)
        .arg(format!("{RELEASES}/{asset}")))
    .await?;
    if asset.ends_with(".tgz") {
        run(Command::new("tar")
            .arg("-xzf")
            .arg(&download)
            .arg("-C")
            .arg(&dir))
        .await?;
        let _ = std::fs::remove_file(&download);
    } else {
        std::fs::rename(&download, &target).map_err(|err| err.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755))
            .map_err(|err| err.to_string())?;
    }
    target
        .is_file()
        .then_some(target)
        .ok_or_else(|| "the download held no cloudflared".into())
}

async fn run(command: &mut Command) -> Result<(), String> {
    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|err| format!("{program}: {err}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{program} failed ({status})"))
}
