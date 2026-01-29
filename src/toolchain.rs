use std::{
    borrow::ToOwned,
    collections::{BTreeSet, HashSet},
    ffi::{OsStr, OsString},
    fs,
    hash::{Hash, Hasher},
    path::Path,
    sync::LazyLock,
};

use anyhow::{self, Context, Result};
use cmd_lib::run_cmd;
use tracing::info;
use twox_hash::XxHash64;

use crate::{
    LOCAL_RUSTUP, LOCAL_RYNZLAND_HOME, rustup, set_env_local,
    util::{self, HashEncoder, qualify_with_target},
};

static CHANNEL_MANIFEST_SUBPATH: LazyLock<&'static Path> =
    LazyLock::new(|| Path::new("lib/rustlib/multirust-channel-manifest.toml"));

static COMPONENTS_SUBPATH: LazyLock<&'static Path> =
    LazyLock::new(|| Path::new("lib/rustlib/components"));

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentifiableToolchain {
    /// The value of `pkg.rust.version` in the channel manifest.
    pub rust_ver: String,

    /// The items in the `components` file.
    // TODO: Investigate whether host targets need to be normalized,
    // as well as whether `multirust-config.toml` should be used instead.
    pub components: BTreeSet<String>,
}

pub fn resolve_channel(channel: &str, components: &[String]) -> Result<IdentifiableToolchain> {
    let temp_dir = tempfile::Builder::new().prefix("rynzland").tempdir()?;
    let temp_dir = temp_dir.path();
    fs::create_dir_all(temp_dir)?;

    let manifest_url = rustup::manifest_url(channel);
    let manifest_path = temp_dir.join("multirust-channel-manifest.toml");
    info!("downloading manifest from {manifest_url}...");
    util::download_file(&manifest_url, &manifest_path)?;
    let rust_ver = rust_ver_from_manifest(&manifest_path)?;

    let components = match components {
        [] => ["rustc", "cargo", "rust-std"]
            .into_iter()
            .chain(
                util::BUILD_TARGET
                    .ends_with("-pc-windows-gnu")
                    .then_some("rust-mingw"),
            )
            .map(|s| qualify_with_target(s).to_string())
            .collect(),
        cs => cs.iter().map(|s| qualify_with_target(s).into()).collect(),
    };

    Ok(IdentifiableToolchain {
        rust_ver,
        components,
    })
}

impl IdentifiableToolchain {
    pub const SEED: u64 = 0xfeed_c001_1ced_7ea5;

    pub fn new(toolchain: &Path) -> Result<Self> {
        let manifest_path = toolchain.join(*CHANNEL_MANIFEST_SUBPATH);
        let rust_ver = rust_ver_from_manifest(&manifest_path)?;

        let components_path = toolchain.join(*COMPONENTS_SUBPATH);
        let components = fs::read_to_string(components_path)?;
        let components = components.lines().map(ToOwned::to_owned).collect();

        Ok(Self {
            rust_ver,
            components,
        })
    }

    pub fn id(&self) -> String {
        let ver = &self.rust_ver;

        let ver_rlimit = ver
            .bytes()
            .take_while(|&b| b".0123456789".contains(&b))
            .count();
        let short_ver = &ver[..ver_rlimit];

        let mut id = if short_ver.is_empty() {
            "unknown-".to_owned()
        } else {
            short_ver.to_owned() + "-"
        };

        let hash = XxHash64::oneshot(Self::SEED, ver.as_bytes());
        id.push_str(&HashEncoder::encode(hash));

        id.push('-');

        let mut hasher = XxHash64::with_seed(Self::SEED);
        self.components.hash(&mut hasher);
        id.push_str(&HashEncoder::encode(hasher.finish()));

        id
    }
}

/// Garbage collect all toolchain links in [`LOCAL_RYNZLAND_HOME`] that are no
/// longer referencing any of the given candidates.
/// If candidates is `None`, then it defaults to all underlying toolchains.
pub fn gc<S, I>(candidates: impl Into<Option<I>>) -> Result<()>
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = S>,
{
    // TODO: Add an OS-global lock to avoid multiple GCs clashing with each other.
    let candidates: Option<HashSet<_>> = candidates
        .into()
        .map(|cs| cs.into_iter().map(|it| it.as_ref().to_owned()).collect());
    if candidates.as_ref().is_some_and(HashSet::is_empty) {
        return Ok(());
    }

    unsafe { set_env_local() };

    let mut referenced = HashSet::new();
    let walker = LOCAL_RYNZLAND_HOME.join("toolchains").read_dir()?;
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
            tc.display()
        );
        run_cmd! { $LOCAL_RUSTUP uninstall $tc }
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

pub fn rust_ver_from_manifest(manifest_path: &Path) -> Result<String> {
    fn enter_table<'v>(table: &'v toml::Value, key: &str) -> Result<&'v toml::Value> {
        table
            .as_table()
            .context("expecting a table")?
            .get(key)
            .with_context(|| format!("failed to find item with key '{key}'"))
    }

    let manifest = fs::read_to_string(manifest_path)
        .with_context(|| format!("when reading manifest at {}", manifest_path.display()))?;
    let manifest: toml::Value = toml::from_str(&manifest)?;

    Ok(enter_table(&manifest, "pkg")
        .and_then(|it| enter_table(it, "rust"))
        .and_then(|it| enter_table(it, "version"))
        .and_then(|it| it.as_str().context("expected a string"))
        .context("failed to get `pkg.rust.version` from channel manifest")?
        .to_owned())
}
