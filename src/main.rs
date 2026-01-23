use std::{
    borrow::Cow,
    env, iter,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::Result;
use argh::FromArgs;
use cmd_lib::run_cmd;
use pathdiff::diff_paths;
use tokio::fs;
use tracing::info;

use crate::{
    toolchain::{IdentifiableToolchain, rust_ver_from_manifest},
    util::qualify_with_target,
};

mod rustup;
mod toolchain;
mod util;

/// Hey choom, mind giving me a hand?
#[derive(FromArgs, PartialEq, Debug)]
struct Rynzland {
    #[argh(subcommand)]
    subcmd: RynzlandSubcmd,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum RynzlandSubcmd {
    Setup(SetupSubcmd),
    Add(AddSubcmd),
    Rm(RmSubCmd),
    Run(RunSubCmd),
    Nuke(NukeSubcmd),
    Id(IdSubcmd),
    IdChan(IdChanSubcmd),
}

/// set up a local rustup installation
#[derive(FromArgs, Clone, Copy, PartialEq, Debug)]
#[argh(subcommand, name = "setup")]
struct SetupSubcmd {}

/// install a toolchain in the local environment
#[derive(FromArgs, Clone, PartialEq, Debug)]
#[argh(subcommand, name = "add")]
struct AddSubcmd {
    /// the underlying source toolchain to install from, defaults to
    /// the target toolchain itself
    #[argh(option, short = 's')]
    source: Option<String>,

    /// the toolchain to install
    #[argh(positional)]
    toolchain: String,
}

/// remove a toolchain in the local environment
#[derive(FromArgs, Clone, PartialEq, Debug)]
#[argh(subcommand, name = "rm")]
struct RmSubCmd {
    /// the toolchain to remove
    #[argh(positional)]
    toolchain: String,
}

/// run a rustup shim in the linked environment
#[derive(FromArgs, Clone, PartialEq, Debug)]
#[argh(subcommand, name = "run")]
struct RunSubCmd {
    /// the shim to run
    #[argh(positional)]
    shim: String,

    /// the toolchain to use
    #[argh(option, short = 't')]
    toolchain: Option<String>,

    /// the args to pass to the shim
    #[argh(positional)]
    args: Vec<String>,
}

/// nuke the local rustup installation
#[derive(FromArgs, Clone, Copy, PartialEq, Debug)]
#[argh(subcommand, name = "nuke")]
struct NukeSubcmd {}

/// print the ID of a toolchain
#[derive(FromArgs, Clone, PartialEq, Debug)]
#[argh(subcommand, name = "id")]
struct IdSubcmd {
    /// the toolchain to identify
    #[argh(positional)]
    toolchain: String,
}
/// identify a channel by downloading its manifest
#[derive(FromArgs, Clone, PartialEq, Debug)]
#[argh(subcommand, name = "id-chan")]
struct IdChanSubcmd {
    /// the toolchain channel to identify
    #[argh(positional)]
    channel: String,

    /// explicit list of components to include
    #[argh(option, short = 'c')]
    components: Vec<String>,
}

static LOCAL_HOME: LazyLock<PathBuf> = LazyLock::new(|| Path::new("home").canonicalize().unwrap());
static LOCAL_RUSTUP: LazyLock<PathBuf> = LazyLock::new(|| LOCAL_HOME.join("rustup"));
static LOCAL_RUSTUP_HOME: LazyLock<PathBuf> = LazyLock::new(|| LOCAL_HOME.join("rustup_home"));
static LOCAL_RYNZLAND_HOME: LazyLock<PathBuf> = LazyLock::new(|| LOCAL_HOME.join("rynzland_home"));
static LOCAL_CARGO_HOME: LazyLock<PathBuf> = LazyLock::new(|| LOCAL_HOME.join("cargo_home"));

unsafe fn set_env_local() {
    unsafe {
        env::set_var("RUSTUP_HOME", &*LOCAL_RUSTUP_HOME);
        env::set_var("CARGO_HOME", &*LOCAL_CARGO_HOME);
    }
}

unsafe fn set_env_rynzland() {
    unsafe {
        env::set_var("RUSTUP_HOME", &*LOCAL_RYNZLAND_HOME);
        env::set_var("CARGO_HOME", &*LOCAL_CARGO_HOME);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    unsafe {
        env::remove_var("RUSTUP_TOOLCHAIN");
        cmd_lib::set_debug(true);
    }

    let app: Rynzland = argh::from_env();
    match app.subcmd {
        // TODO: Figure out how to check for and how to handle toolchain updates
        RynzlandSubcmd::Setup(setup) => setup.run().await?,
        RynzlandSubcmd::Add(add) => add.run().await?,
        RynzlandSubcmd::Rm(rm) => rm.run().await?,
        RynzlandSubcmd::Run(run) => run.run()?,
        RynzlandSubcmd::Nuke(nuke) => nuke.run().await?,
        RynzlandSubcmd::Id(id) => id.run()?,
        RynzlandSubcmd::IdChan(id_chan) => id_chan.run().await?,
    }

    Ok(())
}

impl SetupSubcmd {
    async fn run(&self) -> Result<()> {
        unsafe { set_env_local() };

        if fs::try_exists(&*LOCAL_RUSTUP).await? {
            info!("rustup already set up, skipping...");
        } else {
            info!("setting up rustup...");
            rustup::setup(&LOCAL_RUSTUP).await?;
        }
        // TODO: Use hardlink as a fallback on Windows
        info!("setting up FS link to local rustup...");
        let local_cargo_bin = LOCAL_CARGO_HOME.join("bin");

        for dir in [
            &local_cargo_bin,
            &LOCAL_RUSTUP_HOME.join("toolchains"),
            &LOCAL_RYNZLAND_HOME.join("toolchains"),
        ] {
            fs::create_dir_all(dir).await?;
        }

        let local_rustup_link = local_cargo_bin.join("rustup");
        if !fs::try_exists(&local_rustup_link).await? {
            fs::symlink(&*LOCAL_RUSTUP, &local_rustup_link).await?;
        }

        run_cmd! { $LOCAL_RUSTUP --version }?;
        for home in [&*LOCAL_RUSTUP_HOME, &*LOCAL_RYNZLAND_HOME] {
            run_cmd! { RUSTUP_HOME=$home $LOCAL_RUSTUP set profile minimal }?;
            run_cmd! { RUSTUP_HOME=$home $LOCAL_RUSTUP set auto-self-update disable }?;
        }
        Ok(())
    }
}

impl AddSubcmd {
    async fn run(&self) -> Result<()> {
        unsafe { set_env_local() };

        let toolchain = qualify_with_target(&self.toolchain);
        let source = self
            .source
            .as_deref()
            .map_or_else(|| Cow::Borrowed(&*toolchain), qualify_with_target);

        if toolchain == source {
            info!("adding toolchain: {toolchain}");
        } else {
            info!("adding toolchain: {toolchain} from source {source}");
        }

        // TODO: Use juntion on Windows
        let actual = LOCAL_RUSTUP_HOME.join("toolchains").join(&*source);
        let link = LOCAL_RYNZLAND_HOME.join("toolchains").join(&*toolchain);
        let actual = diff_paths(&actual, link.parent().unwrap()).unwrap_or(actual);

        // NOTE: We create the in-flight link first to declare the beginning of the transaction of
        // the `link` toolchain creation.
        let mut link_in_flight = link.clone();
        link_in_flight.set_extension("tmp");
        fs::symlink(&actual, &link_in_flight).await?;

        run_cmd! { $LOCAL_RUSTUP install $source }?;

        // NOTE: Renaming is atomic on most platforms.
        // This also declares the successful end of the transaction.
        fs::rename(&link_in_flight, &link).await?;
        Ok(())
    }
}

impl RmSubCmd {
    async fn run(&self) -> Result<()> {
        unsafe { set_env_local() };

        let toolchain = qualify_with_target(&self.toolchain);
        info!("removing toolchain: {toolchain}");

        // TODO: Use juntion on Windows
        let link = LOCAL_RYNZLAND_HOME.join("toolchains").join(&*toolchain);
        let link_target = fs::read_link(&link).await?;
        let underlying_toolchain = link_target.file_name().unwrap().to_string_lossy();

        fs::remove_file(&link).await?;

        // TODO: Extract the GC logic elsewhere; the GC should be guarded by a global lock.
        let mut referenced = false;
        let mut walker = fs::read_dir(LOCAL_RYNZLAND_HOME.join("toolchains")).await?;
        while let Some(entry) = walker.next_entry().await? {
            let Ok(target) = fs::read_link(entry.path()).await else {
                continue;
            };
            if target == link_target {
                referenced = true;
                break;
            }
        }
        if !referenced {
            info!(
                "underlying toolchain {underlying_toolchain} is no longer referenced, removing..."
            );
            run_cmd! { $LOCAL_RUSTUP uninstall $underlying_toolchain }?;
        }

        Ok(())
    }
}

impl RunSubCmd {
    fn run(&self) -> Result<()> {
        unsafe { set_env_rynzland() };

        let Self {
            shim,
            args,
            toolchain,
        } = self;
        let mut args = Cow::Borrowed(args);
        if let Some(toolchain) = toolchain {
            args = Cow::Owned(
                iter::once(format!("+{toolchain}"))
                    .chain(args.iter().cloned())
                    .collect(),
            );
        }
        let args = args.as_slice();

        run_cmd! { RUSTUP_FORCE_ARG0=$shim $LOCAL_RUSTUP $[args] }?;
        Ok(())
    }
}

impl NukeSubcmd {
    async fn run(&self) -> Result<()> {
        info!("nuking local rustup installation...");

        let mut walker = fs::read_dir(&*LOCAL_HOME).await?;
        while let Some(entry) = walker.next_entry().await? {
            let file_type = entry.file_type().await?;
            if file_type.is_symlink() || file_type.is_file() {
                if entry.file_name() == ".gitkeep" {
                    continue;
                }
                fs::remove_file(entry.path()).await?;
            } else if file_type.is_dir() {
                fs::remove_dir_all(entry.path()).await?;
            }
        }

        Ok(())
    }
}

impl IdSubcmd {
    fn run(&self) -> Result<()> {
        unsafe { set_env_rynzland() };

        let toolchain = qualify_with_target(&self.toolchain);
        let toolchain_path = LOCAL_RYNZLAND_HOME.join("toolchains").join(&*toolchain);
        let id = IdentifiableToolchain::new(&toolchain_path)?.id();
        println!("{id}");
        Ok(())
    }
}

impl IdChanSubcmd {
    async fn run(&self) -> Result<()> {
        let temp_dir = tempfile::Builder::new().prefix("rynzland").tempdir()?;
        let temp_dir = temp_dir.path();
        fs::create_dir_all(&temp_dir).await?;

        let manifest_url = rustup::manifest_url(&self.channel);
        let manifest_path = temp_dir.join("multirust-channel-manifest.toml");
        info!("downloading manifest from {manifest_url}...");
        util::download_file(&manifest_url, &manifest_path).await?;
        let rust_ver = rust_ver_from_manifest(&manifest_path)?;

        let components = match &self.components[..] {
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

        let id = IdentifiableToolchain {
            rust_ver,
            components,
        }
        .id();

        println!("{id}");
        Ok(())
    }
}
