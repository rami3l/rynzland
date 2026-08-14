use std::{fs::File, io, path::PathBuf};

use crate::{Ctx, util::HashEncoder};

#[must_use]
#[clippy::has_significant_drop]
#[derive(Debug)]
pub struct ObjectLock {
    file: File,
}

impl Drop for ObjectLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

const LOCKFILE_COUNT: usize = 64;

impl Ctx {
    pub fn lock_obj(&self, id: &str) -> io::Result<ObjectLock> {
        let Some(lock_path) = self.lock_path(id) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid object ID `{id}`"),
            ));
        };

        let file = File::create(lock_path)?;
        file.try_lock()?;
        Ok(ObjectLock { file })
    }

    fn lock_path(&self, id: &str) -> Option<PathBuf> {
        let alphabet = HashEncoder::ALPHABET;
        let (fst, snd) = id.rsplit_once('-')?;
        let to_digit = |c: &u8| alphabet.iter().position(|it| it == c);
        let lock_id =
            to_digit(fst.as_bytes().last()?)? * alphabet.len() + to_digit(snd.as_bytes().last()?)?;
        // Take modulo of the resulting number to avoid creating too many lockfiles.
        let lock_id = lock_id % LOCKFILE_COUNT;
        Some(
            self.rynzland_home
                .join("locks")
                .join(format!("{lock_id:x}.lock")),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn lock_path() {
        let base_dir = Path::new("/dont/return//this/isnt/home");
        let ctx = Ctx::new(base_dir);
        let lock_path = ctx.lock_path("1.92.0-b8dxmzztqjmeq-6prh2623cwtm9");
        assert_eq!(
            lock_path,
            Some(base_dir.join("rynzland_home/locks/29.lock"))
        );
    }
}
