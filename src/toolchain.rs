use std::{
    borrow::ToOwned,
    collections::BTreeSet,
    fs,
    hash::{Hash, Hasher},
    path::Path,
    sync::LazyLock,
};

use anyhow::{self, Context, Result};
use tracing::info;
use twox_hash::XxHash64;

use crate::{
    rustup,
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
    pub const SEED: u64 = 0xfeed_1ced_0d06_f00d;

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
        let mut id = String::new();

        let hash = XxHash64::oneshot(Self::SEED, self.rust_ver.as_bytes());
        id.push_str(&HashEncoder::encode(hash));

        id.push('-');

        let mut hasher = XxHash64::with_seed(Self::SEED);
        self.components.hash(&mut hasher);
        id.push_str(&HashEncoder::encode(hasher.finish()));

        id
    }
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
        .context("failed to get `pkg.rust.version` from channel manifest")?
        .to_string())
}
