use std::{
    borrow::Cow,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use anyhow::Result;

pub const BUILD_TARGET: &str = env!("TARGET");

pub fn qualify_with_target(toolchain: &str) -> Cow<'_, str> {
    let suffix = format!("-{BUILD_TARGET}");
    if toolchain.ends_with(&suffix) {
        return toolchain.into();
    }
    format!("{toolchain}{suffix}").into()
}

pub fn download_file(url: &str, dest: &Path) -> Result<()> {
    let mut resp = ureq::get(url).call()?;
    let mut reader = resp.body_mut().as_reader();
    let mut dest = File::create(dest)?;
    std::io::copy(&mut reader, &mut dest)?;
    Ok(())
}

// https://stackoverflow.com/a/65192210
pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn with_tmp(path: &Path) -> PathBuf {
    let mut path = path.as_os_str().to_owned();
    path.push(".tmp");
    path.into()
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
