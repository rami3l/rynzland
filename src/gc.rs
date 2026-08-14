use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    io,
    process::Command,
};

use anyhow::Result;
use itertools::Either;
use tracing::info;

use crate::{
    Ctx,
    util::{self, CommandExt},
};

impl Ctx {
    /// Garbage collects all objects in the collection `candidates`.
    ///
    /// Each candidate is identified with the object ID in `self.rustup_home`.
    /// When a candidate is found to be unreachable via any of the existing
    /// references, it will be cleaned up. If `candidates` is `None`, then it
    /// defaults to all existing objects.
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

        // NOTE: Entering the critical section.
        let mut locks = match candidates {
            Some(candidates) => Either::Left(candidates.into_iter()),
            None => Either::Right(
                self.rustup_home
                    .join("toolchains")
                    .read_dir()?
                    .filter_map(|it| Some(it.ok()?.file_name())),
            ),
        }
        .filter_map(|c| match self.lock_obj(&c.to_string_lossy()) {
            Ok(lock) => Some(Ok((c, lock))),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => None,
            Err(e) => Some(Err(e.into())),
        })
        .collect::<Result<HashMap<_, _>>>()?;

        let mut reachable = HashSet::new();
        let walker = self.rynzland_home.join("toolchains").read_dir()?;
        for entry in walker {
            if let Ok(target) = util::soft_link_target(entry?.path())
                && let Some(name) = target.file_name()
            {
                reachable.insert(name.to_owned());
            }
        }

        for tc in &reachable {
            locks.remove(tc);
        }
        for (tc, lock) in locks {
            info!(
                "underlying toolchain {} is unreachable, removing...",
                tc.display(),
            );
            // HACK: In reality, we should be able to just move the target directory to
            // `tmp/`. Here, we just call `rustup uninstall` to remove the toolchain.
            self.set_env_local(&mut Command::new(&self.rustup))
                .arg("uninstall")
                .arg(tc)
                .run_checked()?;
            drop(lock);
        }
        Ok(())
    }
}
