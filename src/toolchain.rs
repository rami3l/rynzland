use std::{
    borrow::ToOwned,
    collections::BTreeSet,
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    sync::LazyLock,
};

use anyhow::{self, Context, Result};

use crate::util::HashEncoder;

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

impl IdentifiableToolchain {
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

        let mut hasher = DefaultHasher::new();
        self.rust_ver.hash(&mut hasher);
        id.push_str(&HashEncoder::encode(hasher.finish()));

        id.push('-');

        hasher = DefaultHasher::new();
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
        .and_then(|pkg| enter_table(pkg, "rust"))
        .and_then(|rust| enter_table(rust, "version"))
        .context("failed to get pkg.rust from channel manifest")?
        .to_string())
}
