use std::{borrow::Cow, path::Path};

use anyhow::Result;
use tokio::{fs::File, io::AsyncWriteExt};

pub const BUILD_TARGET: &str = env!("TARGET");

pub fn qualify_with_target(toolchain: &str) -> Cow<'_, str> {
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

pub struct HashEncoder;

impl HashEncoder {
    pub const ALPHABET: [u8; 32] = *b"0123456789abcdefhjkmnqprstuvwxyz";

    pub fn encode(hash: u64) -> String {
        base_x::encode(Self::ALPHABET.as_ref(), &hash.to_le_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_encode() {
        assert_eq!(HashEncoder::encode(112_358_777), "7kxdk0s000000");
    }
}
