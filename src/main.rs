use std::{
    borrow::Cow,
    env, fs, iter,
    os::unix::fs as ofs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::Result;
use argh::FromArgs;
use cmd_lib::run_cmd;
use pathdiff::diff_paths;
use tracing::info;

use crate::{toolchain::IdentifiableToolchain, util::qualify_with_target};

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

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    unsafe {
        env::remove_var("RUSTUP_TOOLCHAIN");
        cmd_lib::set_debug(true);
    }

    let app: Rynzland = argh::from_env();
    match app.subcmd {
        // TODO: Figure out how to check for and how to handle toolchain updates
        RynzlandSubcmd::Setup(setup) => setup.run()?,
        RynzlandSubcmd::Add(add) => add.run()?,
        RynzlandSubcmd::Rm(rm) => rm.run()?,
        RynzlandSubcmd::Run(run) => run.run()?,
        RynzlandSubcmd::Nuke(nuke) => nuke.run()?,
        RynzlandSubcmd::Id(id) => id.run()?,
        RynzlandSubcmd::IdChan(id_chan) => id_chan.run()?,
    }

    Ok(())
}

impl SetupSubcmd {
    #[allow(clippy::unused_self)]
    fn run(self) -> Result<()> {
        unsafe { set_env_local() };

        if LOCAL_RUSTUP.try_exists()? {
            info!("rustup already set up, skipping...");
        } else {
            info!("setting up rustup...");
            rustup::setup(&LOCAL_RUSTUP)?;
        }
        // TODO: Use hardlink as a fallback on Windows
        info!("setting up FS link to local rustup...");
        let local_cargo_bin = LOCAL_CARGO_HOME.join("bin");

        for dir in [
            &local_cargo_bin,
            &LOCAL_RUSTUP_HOME.join("toolchains"),
            &LOCAL_RYNZLAND_HOME.join("toolchains"),
        ] {
            fs::create_dir_all(dir)?;
        }

        let local_rustup_link = local_cargo_bin.join("rustup");
        if !local_rustup_link.try_exists()? {
            ofs::symlink(&*LOCAL_RUSTUP, &local_rustup_link)?;
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
    fn run(&self) -> Result<()> {
        unsafe { set_env_local() };

        let toolchain = qualify_with_target(&self.toolchain);
        let src = self
            .source
            .as_deref()
            .map_or_else(|| Cow::Borrowed(&*toolchain), qualify_with_target);

        let chan = src
            .strip_suffix(&format!("-{}", util::BUILD_TARGET))
            .unwrap();
        let id = toolchain::resolve_channel(chan, &[])?.id();

        if toolchain == src {
            info!("adding toolchain: {toolchain} (id: {id})");
        } else {
            info!("adding toolchain: {toolchain} from source {src} (id: {id})");
        }

        // TODO: Use juntion on Windows
        let src_old = LOCAL_RUSTUP_HOME.join("toolchains").join(&*src);
        let src_with_id = LOCAL_RUSTUP_HOME.join("toolchains").join(&id);
        let link = LOCAL_RYNZLAND_HOME.join("toolchains").join(&*toolchain);
        let relative_target = diff_paths(&src_with_id, link.parent().unwrap())
            .map_or_else(|| Cow::Borrowed(&src_with_id), Cow::Owned);

        // NOTE: We create the in-flight link first to declare the beginning of the
        // transaction of the `link` toolchain creation.
        let mut link_in_flight = link.clone();
        link_in_flight.set_extension("tmp");
        ofs::symlink(&*relative_target, &link_in_flight)?;

        if src_with_id.exists() {
            info!("toolchain with id {id} already installed, skipping...");
        } else {
            run_cmd! { $LOCAL_RUSTUP install $src }?;
            fs::rename(&src_old, &src_with_id)?;
        }

        // NOTE: Renaming is atomic on most platforms.
        // This also declares the successful end of the transaction.
        fs::rename(&link_in_flight, &link)?;
        Ok(())
    }
}

impl RmSubCmd {
    fn run(&self) -> Result<()> {
        unsafe { set_env_local() };

        let toolchain = qualify_with_target(&self.toolchain);
        info!("removing toolchain: {toolchain}");

        // TODO: Use juntion on Windows
        let link = LOCAL_RYNZLAND_HOME.join("toolchains").join(&*toolchain);
        let link_target = fs::read_link(&link)?;
        let underlying = link_target.file_name().unwrap().to_string_lossy();

        fs::remove_file(&link)?;

        // TODO: Extract the GC logic elsewhere; the GC should be guarded by a global
        // lock.
        let mut referenced = false;
        let walker = fs::read_dir(LOCAL_RYNZLAND_HOME.join("toolchains"))?;
        for entry in walker {
            let entry = entry?;
            let Ok(target) = fs::read_link(entry.path()) else {
                continue;
            };
            if target == link_target {
                referenced = true;
                break;
            }
        }
        if !referenced {
            info!("underlying toolchain {underlying} is no longer referenced, removing...");
            run_cmd! { $LOCAL_RUSTUP uninstall $underlying }?;
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
    #[allow(clippy::unused_self)]
    fn run(self) -> Result<()> {
        info!("nuking local rustup installation...");

        let walker = fs::read_dir(&*LOCAL_HOME)?;
        for entry in walker {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() || file_type.is_file() {
                if entry.file_name() == ".gitkeep" {
                    continue;
                }
                fs::remove_file(entry.path())?;
            } else if file_type.is_dir() {
                fs::remove_dir_all(entry.path())?;
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
    fn run(&self) -> Result<()> {
        let id_toolchain = toolchain::resolve_channel(&self.channel, &self.components)?;
        println!("{}", id_toolchain.id());
        Ok(())
    }
}
