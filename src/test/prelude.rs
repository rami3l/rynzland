use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::{Ctx as AppCtx, SetupSubcmd};

pub struct Ctx {
    temp_dir: tempfile::TempDir,
}

impl Ctx {
    /// Create a new test context with isolated temp dir and an empty `home`.
    pub fn new() -> Result<Self> {
        let temp_dir = tempfile::Builder::new().prefix("rynzland_test").tempdir()?;
        let tempdir_path = temp_dir.path();
        fs::create_dir_all(tempdir_path.join("home"))?;

        Ok(Self { temp_dir })
    }

    /// Like [`Self::new`], but also runs setup.
    pub fn setup() -> Result<Self> {
        let ctx = Self::new()?;
        SetupSubcmd {}.run(&ctx.app_ctx())?;
        Ok(ctx)
    }

    pub fn dir(&self) -> &Path {
        self.temp_dir.path()
    }

    pub fn home(&self) -> PathBuf {
        self.dir().join("home")
    }

    pub fn app_ctx(&self) -> AppCtx {
        AppCtx::new(self.home())
    }
}
