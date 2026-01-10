use std::{borrow::Cow, path::Path};

use anyhow::Result;
use tokio::{fs::File, io::AsyncWriteExt};

pub const BUILD_TARGET: &str = env!("TARGET");

pub fn normalize_toolchain(toolchain: &str) -> Cow<'_, str> {
    let suffix = format!("-{BUILD_TARGET}");
    if toolchain.ends_with(&suffix) {
        return toolchain.into();
    }
    format!("{toolchain}{suffix}").into()
}

pub async fn download_file(url: &str, dest: &Path) -> Result<()> {
    let resp = reqwest::get(url).await?;
    let mut dest = File::create(dest).await?;
    let bytes = resp.bytes().await?;
    dest.write_all(&bytes).await?;
    Ok(())
}
