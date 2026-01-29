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

/// Creates a soft link from `link` to `original` (symlink on Unix, junction on Windows).
/// Both paths are expected to be absolute.
pub fn soft_link(original: &Path, link: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs as ofs;

        use anyhow::Context;
        use pathdiff::diff_paths;

        let rel_target =
            diff_paths(original, link.parent().unwrap()).context("malformed FS link path")?;
        ofs::symlink(&rel_target, link)?;
    }

    #[cfg(windows)]
    junction::create(original, link)?;

    Ok(())
}

pub fn soft_link_target(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();

    #[cfg(unix)]
    let target = fs::read_link(path)?;

    #[cfg(windows)]
    let target = junction::get_target(path)?;

    Ok(target)
}

pub fn soft_unlink(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::remove_file(path)?;

    #[cfg(windows)]
    fs::remove_dir(path)?;

    Ok(())
}

impl HashEncoder {
    pub const ALPHABET: [u8; 32] = *b"0123456789abcdefhjkmnqprstuvwxyz";

    pub fn encode(hash: u64) -> String {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss
        )]
        let width = ((size_of::<u64>() as f32) * 8. / 32_f32.log2()).ceil() as usize;
        let raw = base_x::encode(Self::ALPHABET.as_ref(), &hash.to_be_bytes());
        format!("{raw:0>width$}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_encode() {
        let original = [1, 112_358_777, 1_618_033_988, 2_718_281_828, u64::MAX - 1];
        let encoded = original.map(HashEncoder::encode);
        assert_eq!(
            encoded,
            [
                "0000000000001",
                "00000003b4xbt",
                "0000001h72fa4",
                "0000002j0bc34",
                "fzzzzzzzzzzzy"
            ]
        );
        assert_eq!(
            encoded.iter().map(String::len).collect::<Vec<_>>(),
            [13, 13, 13, 13, 13]
        );
    }
}
