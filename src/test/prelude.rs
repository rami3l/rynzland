use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::SetupSubcmd;

pub struct Ctx {
    tempdir: tempfile::TempDir,
    _cwd: CwdCtx,
}

impl Ctx {
    /// Create a new test context with isolated temp dir and an empty `home`.
    pub fn new() -> Result<Self> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        fs::create_dir_all(tempdir_path.join("home"))?;

        Ok(Self {
            _cwd: CwdCtx::new(tempdir_path)?,
            tempdir,
        })
    }

    /// Like [`Self::new`], but also runs setup.
    pub fn setup() -> Result<Self> {
        let ctx = Self::new()?;
        SetupSubcmd {}.run()?;
        Ok(ctx)
    }

    pub fn dir(&self) -> &Path {
        self.tempdir.path()
    }

    pub fn home(&self) -> PathBuf {
        self.dir().join("home")
    }
}

struct CwdCtx {
    old_cwd: Option<PathBuf>,
}

impl CwdCtx {
    fn new(cwd: &Path) -> Result<Self> {
        let old_cwd = std::env::current_dir().ok();
        std::env::set_current_dir(cwd)?;
        Ok(Self { old_cwd })
    }
}

impl Drop for CwdCtx {
    fn drop(&mut self) {
        let Some(old_cwd) = &self.old_cwd else { return };
        _ = std::env::set_current_dir(old_cwd);
    }
}
