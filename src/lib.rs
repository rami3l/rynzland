use std::{
    borrow::Cow,
    fs, iter,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Result;
use argh::FromArgs;
use tracing::info;

use crate::{
    toolchain::IdentifiableToolchain,
    util::{CommandExt, qualify_with_target},
};

mod rustup;
mod toolchain;
mod util;

#[cfg(test)]
mod test;

#[derive(Debug, Clone)]
pub struct Ctx {
    pub home: PathBuf,
    pub rustup: PathBuf,
    pub rustup_home: PathBuf,
    pub rynzland_home: PathBuf,
    pub cargo_home: PathBuf,
}

impl Ctx {
    #[must_use]
    pub fn new(home: impl AsRef<Path>) -> Self {
        let home = home.as_ref().to_path_buf();
        Self {
            rustup: home.join("rustup"),
            rustup_home: home.join("rustup_home"),
            rynzland_home: home.join("rynzland_home"),
            cargo_home: home.join("cargo_home"),
            home,
        }
    }

    pub fn set_env_local<'a>(&self, cmd: &'a mut Command) -> &'a mut Command {
        cmd.env("RUSTUP_HOME", &self.rustup_home)
            .env("CARGO_HOME", &self.cargo_home)
    }

    pub fn set_env_rynzland<'a>(&self, cmd: &'a mut Command) -> &'a mut Command {
        cmd.env("RUSTUP_HOME", &self.rynzland_home)
            .env("CARGO_HOME", &self.cargo_home)
    }
}

/// Hey choom, mind giving me a hand?
#[derive(FromArgs, PartialEq, Eq, Debug)]
pub struct Rynzland {
    #[argh(subcommand)]
    pub subcmd: RynzlandSubcmd,
}

#[derive(FromArgs, PartialEq, Eq, Debug)]
#[argh(subcommand)]
pub enum RynzlandSubcmd {
    Setup(SetupSubcmd),
    Add(AddSubcmd),
    Rm(RmSubCmd),
    Run(RunSubCmd),
    Nuke(NukeSubcmd),
    Id(IdSubcmd),
    IdChan(IdChanSubcmd),
    CompAdd(CompAddSubcmd),
    CompRm(CompRmSubcmd),
}

impl RynzlandSubcmd {
    pub fn run(self, ctx: &Ctx) -> Result<()> {
        match self {
            Self::Setup(cmd) => cmd.run(ctx),
            Self::Add(cmd) => cmd.run(ctx),
            Self::Rm(cmd) => cmd.run(ctx),
            Self::Run(cmd) => cmd.run(ctx),
            Self::Nuke(cmd) => cmd.run(ctx),
            Self::Id(cmd) => cmd.run(ctx),
            Self::IdChan(cmd) => cmd.run(ctx),
            Self::CompAdd(cmd) => cmd.run(ctx),
            Self::CompRm(cmd) => cmd.run(ctx),
        }
    }
}

/// add components to a toolchain
#[derive(FromArgs, Clone, PartialEq, Eq, Debug)]
#[argh(subcommand, name = "comp-add")]
pub struct CompAddSubcmd {
    /// the toolchain to modify
    #[argh(positional)]
    toolchain: String,

    /// the components to add
    #[argh(positional)]
    components: Vec<String>,
}

/// remove components from a toolchain
#[derive(FromArgs, Clone, PartialEq, Eq, Debug)]
#[argh(subcommand, name = "comp-rm")]
pub struct CompRmSubcmd {
    /// the toolchain to modify
    #[argh(positional)]
    toolchain: String,

    /// the components to remove
    #[argh(positional)]
    components: Vec<String>,
}

/// set up a local rustup installation
#[derive(FromArgs, Clone, Copy, PartialEq, Eq, Debug)]
#[argh(subcommand, name = "setup")]
pub struct SetupSubcmd {}

/// install a toolchain in the local environment
#[derive(FromArgs, Clone, PartialEq, Eq, Debug)]
#[argh(subcommand, name = "add")]
pub struct AddSubcmd {
    /// the underlying source toolchain to install from, defaults to
    /// the target toolchain itself
    #[argh(option, short = 's')]
    source: Option<String>,

    /// the toolchain to install
    #[argh(positional)]
    toolchain: String,
}

/// remove a toolchain in the local environment
#[derive(FromArgs, Clone, PartialEq, Eq, Debug)]
#[argh(subcommand, name = "rm")]
pub struct RmSubCmd {
    /// the toolchain to remove
    #[argh(positional)]
    toolchain: String,
}

/// run a rustup shim in the linked environment
#[derive(FromArgs, Clone, PartialEq, Eq, Debug)]
#[argh(subcommand, name = "run")]
pub struct RunSubCmd {
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
#[derive(FromArgs, Clone, Copy, PartialEq, Eq, Debug)]
#[argh(subcommand, name = "nuke")]
pub struct NukeSubcmd {}

/// print the ID of a toolchain
#[derive(FromArgs, Clone, PartialEq, Eq, Debug)]
#[argh(subcommand, name = "id")]
pub struct IdSubcmd {
    /// the toolchain to identify
    #[argh(positional)]
    toolchain: String,
}
/// identify a channel by downloading its manifest
#[derive(FromArgs, Clone, PartialEq, Eq, Debug)]
#[argh(subcommand, name = "id-chan")]
pub struct IdChanSubcmd {
    /// the toolchain channel to identify
    #[argh(positional)]
    channel: String,

    /// explicit list of components to include
    #[argh(option, short = 'c')]
    components: Vec<String>,
}

impl SetupSubcmd {
    #[allow(clippy::unused_self)]
    pub fn run(self, ctx: &Ctx) -> Result<()> {
        if ctx.rustup.try_exists()? {
            info!("rustup already set up, skipping...");
        } else {
            info!("setting up rustup...");
            rustup::setup(&ctx.rustup)?;
        }
        info!("setting up FS link to local rustup...");
        let local_cargo_bin = ctx.cargo_home.join("bin");

        for dir in [
            &local_cargo_bin,
            &ctx.rustup_home.join("toolchains"),
            &ctx.rynzland_home.join("toolchains"),
        ] {
            fs::create_dir_all(dir)?;
        }

        let local_rustup_link = local_cargo_bin.join("rustup");
        if !local_rustup_link.try_exists()? {
            #[cfg(unix)]
            util::soft_link(&ctx.rustup, &local_rustup_link)?;

            #[cfg(windows)]
            fs::hard_link(&*ctx.rustup, &local_rustup_link)?;
        }

        for home in [&ctx.rustup_home, &ctx.rynzland_home] {
            Command::new(&ctx.rustup)
                .env("RUSTUP_HOME", home)
                .args(["set", "profile", "minimal"])
                .run_checked()?;

            Command::new(&ctx.rustup)
                .env("RUSTUP_HOME", home)
                .args(["set", "auto-install", "disable"])
                .run_checked()?;

            Command::new(&ctx.rustup)
                .env("RUSTUP_HOME", home)
                .args(["set", "auto-self-update", "disable"])
                .run_checked()?;
        }
        Ok(())
    }
}

impl AddSubcmd {
    pub fn run(&self, ctx: &Ctx) -> Result<()> {
        let toolchain = qualify_with_target(&self.toolchain);
        let src = self
            .source
            .as_deref()
            .map_or_else(|| Cow::Borrowed(&toolchain), qualify_with_target);

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
        let src_old = ctx.rustup_home.join("toolchains").join(&*src);
        let src_with_id = ctx.rustup_home.join("toolchains").join(&id);
        let link = ctx.rynzland_home.join("toolchains").join(&*toolchain);

        // NOTE: We create the in-flight link first to declare the beginning of the
        // transaction of the `link` toolchain creation.
        let link_in_flight = util::with_tmp(&link);
        util::soft_link(&src_with_id, &link_in_flight)?;

        // Save the original underlying toolchain for GC later.
        let underlying = util::soft_link_target(&link).ok();
        let underlying = underlying.as_ref().map(|it| it.file_name().unwrap());

        if src_with_id.exists() {
            info!("toolchain with id {id} already installed, skipping...");
        } else {
            ctx.set_env_local(&mut Command::new(&ctx.rustup))
                .args(["install", &src])
                .run_checked()?;
            fs::rename(&src_old, &src_with_id)?;
        }

        // NOTE: Renaming is atomic on most platforms.
        // This also declares the successful end of the transaction.
        fs::rename(&link_in_flight, &link)?;

        if let Some(underlying) = underlying {
            toolchain::gc(ctx, [underlying])?;
        }
        Ok(())
    }
}

impl RmSubCmd {
    pub fn run(&self, ctx: &Ctx) -> Result<()> {
        let toolchain = qualify_with_target(&self.toolchain);
        info!("removing toolchain: {toolchain}");

        let link = ctx.rynzland_home.join("toolchains").join(&*toolchain);
        let link_target = util::soft_link_target(&link)?;
        let underlying = link_target.file_name().unwrap();

        util::soft_unlink(&link)?;
        toolchain::gc(ctx, [underlying])
    }
}

impl RunSubCmd {
    pub fn run(&self, ctx: &Ctx) -> Result<()> {
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
        ctx.set_env_rynzland(&mut Command::new(&ctx.rustup))
            .env("RUSTUP_FORCE_ARG0", &shim)
            .args(&*args)
            .run_checked()
    }
}

impl NukeSubcmd {
    #[allow(clippy::unused_self)]
    pub fn run(self, ctx: &Ctx) -> Result<()> {
        info!("nuking local rustup installation...");

        let walker = ctx.home.read_dir()?;
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
    pub fn run(&self, ctx: &Ctx) -> Result<()> {
        let toolchain = qualify_with_target(&self.toolchain);
        let toolchain_path = ctx.rynzland_home.join("toolchains").join(&*toolchain);
        let id = IdentifiableToolchain::new(&toolchain_path)?.id();
        println!("{id}");
        Ok(())
    }
}

impl IdChanSubcmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self, _ctx: &Ctx) -> Result<()> {
        let id_toolchain = toolchain::resolve_channel(&self.channel, &self.components)?;
        println!("{}", id_toolchain.id());
        Ok(())
    }
}

impl CompAddSubcmd {
    pub fn run(&self, ctx: &Ctx) -> Result<()> {
        ctx.modify_components(&self.toolchain, &self.components, true)
    }
}

impl CompRmSubcmd {
    pub fn run(&self, ctx: &Ctx) -> Result<()> {
        ctx.modify_components(&self.toolchain, &self.components, false)
    }
}

impl Ctx {
    fn modify_components(&self, toolchain: &str, comps: &[String], add: bool) -> Result<()> {
        if comps.is_empty() {
            info!("no components specified, skipping...");
            return Ok(());
        }

        let toolchain = qualify_with_target(toolchain);
        let link = self.rynzland_home.join("toolchains").join(&*toolchain);

        let underlying_path = link.canonicalize()?;
        let mut underlying = IdentifiableToolchain::new(&underlying_path)?;

        for comp in comps {
            let comp = util::qualify_with_target(comp);
            if add {
                underlying.components.insert(comp.into_owned());
            } else {
                underlying.components.remove(&*comp);
            }
        }

        let old_id = underlying_path.file_name().unwrap();
        let new_id = underlying.id();

        let new_toolchain_dir = self.rustup_home.join("toolchains").join(&new_id);

        // NOTE: We create the in-flight link first to declare the beginning of the
        // transaction of the `link` toolchain creation.
        let link_in_flight = util::with_tmp(&link);
        util::soft_link(&new_toolchain_dir, &link_in_flight)?;

        if new_toolchain_dir.exists() {
            info!("toolchain with id {new_id} already exists, switching...");
        } else {
            info!("creating toolchain {new_id}...");
            let tmp_dir = util::with_tmp(&new_toolchain_dir);

            info!(
                "cloning {} into {}...",
                underlying_path.display(),
                tmp_dir.display()
            );

            // NOTE: This will likely error out if the underlying toolchain exists, because
            // the first `fs::create_dir()` will fail in the first place.
            util::copy_dir_all(&underlying_path, &tmp_dir)?;

            let op = if add { "add" } else { "remove" };

            // HACK: We will have to make rustup think that `toolchain_name` is an official
            // toolchain, so it has to use an official name. This logic shouldn't exist in
            // the final version. Anyway, following the current naming scheme, a
            // toolchain in the pool can never have the name `"stable-<host>"`, so it's
            // fine.
            let toolchain_name = util::qualify_with_target("stable");
            let hack_link = tmp_dir.with_file_name(toolchain_name.as_ref());
            util::soft_link(&tmp_dir, &hack_link)?;
            self.set_env_local(&mut Command::new(&self.rustup))
                .env("RUSTUP_TOOLCHAIN", &*toolchain_name)
                .arg("component")
                .arg(op)
                .args(comps)
                .run_checked()?;
            util::soft_unlink(&hack_link)?;

            fs::rename(&tmp_dir, &new_toolchain_dir)?;
        }

        // NOTE: Renaming is atomic on most platforms.
        // This also declares the successful end of the transaction.
        fs::rename(&link_in_flight, &link)?;
        toolchain::gc(self, [old_id])
    }
}
