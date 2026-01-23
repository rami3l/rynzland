use std::{env::consts::EXE_SUFFIX, fs, path::Path};

use anyhow::Result;

use crate::util::{BUILD_TARGET, download_file};

/// Returns the following URL for the official rustup binary:
/// `https://static.rust-lang.org/rustup/archive/{rustup-version}/{target-triple}/rustup-init[.exe]`
///
/// See: <https://rust-lang.github.io/rustup/installation/other.html#manual-installation>
fn rustup_url(version: &str) -> String {
    format!(
        "https://static.rust-lang.org/rustup/archive/{version}/{BUILD_TARGET}/rustup-init{EXE_SUFFIX}"
    )
}

pub fn manifest_url(channel: &str) -> String {
    format!("https://static.rust-lang.org/dist/channel-rust-{channel}.toml")
}

pub async fn setup(dest: &Path) -> Result<()> {
    // Pin a pre-XDG rustup to simplify path config.
    let url = rustup_url("1.28.2");
    download_file(&url, dest).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = fs::metadata(dest)?.permissions();
        // read/write/execute for owner
        perms.set_mode(0o755);
        fs::set_permissions(dest, perms)?;
    }

    Ok(())
}
