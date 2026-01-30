use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::{Ctx as AppCtx, SetupSubcmd};

pub struct Ctx {
    tempdir: tempfile::TempDir,
}

impl Ctx {
    /// Create a new test context with isolated temp dir and an empty `home`.
    pub fn new() -> Result<Self> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        fs::create_dir_all(tempdir_path.join("home"))?;

        Ok(Self { tempdir })
    }

    /// Like [`Self::new`], but also runs setup.
    pub fn setup() -> Result<Self> {
        let ctx = Self::new()?;
        SetupSubcmd {}.run(&ctx.app_ctx())?;
        Ok(ctx)
    }

    pub fn dir(&self) -> &Path {
        self.tempdir.path()
    }

    pub fn home(&self) -> PathBuf {
        self.dir().join("home")
    }

    pub fn app_ctx(&self) -> AppCtx {
        AppCtx::new(self.home())
    }
}
