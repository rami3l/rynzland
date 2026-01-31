use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    process::Command,
};

use crate::{
    Ctx,
    util::{self, CommandExt},
};
use anyhow::Result;
use gix_lock::Marker;
use tracing::info;

impl Ctx {
    /// Garbage collect all toolchain links in the directory specified by
    /// `self.rynzland_home` that are no longer referencing any of the given
    /// candidates. If `candidates` is `None`, then it defaults to all underlying
    /// toolchains.
    pub fn gc<S, I>(&self, candidates: impl Into<Option<I>>) -> Result<()>
    where
        S: AsRef<OsStr>,
        I: IntoIterator<Item = S>,
    {
        let candidates: Option<HashSet<_>> = candidates
            .into()
            .map(|cs| cs.into_iter().map(|it| it.as_ref().to_owned()).collect());
        if candidates.as_ref().is_some_and(HashSet::is_empty) {
            return Ok(());
        }

        // Now entering the critical section.
        let pool = self.rustup_home.join("toolchains");
        let _lock = Marker::acquire_to_hold_resource(
            pool.join("pool_gc.lock"),
            self.gc_lock_backoff,
            None,
        )?;

        let mut referenced = HashSet::new();
        let walker = self.rynzland_home.join("toolchains").read_dir()?;
        for entry in walker {
            if let Ok(target) = util::soft_link_target(entry?.path())
                && let Some(name) = target.file_name()
            {
                referenced.insert(name.to_owned());
            }
        }

        let rm = |tc: &OsString| {
            info!(
                "underlying toolchain {} is no longer referenced, removing...",
                tc.display(),
            );
            self.set_env_local(&mut Command::new(&self.rustup))
                .arg("uninstall")
                .arg(tc)
                .run_checked()
        };

        let Some(candidates) = &candidates else {
            for tc in referenced {
                rm(&tc)?;
            }
            return Ok(());
        };

        for tc in candidates.difference(&referenced) {
            rm(tc)?;
        }
        Ok(())
    }
}
