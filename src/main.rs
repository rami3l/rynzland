use std::env;

use anyhow::Result;
use rynzland::{Rynzland, RynzlandSubcmd};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    unsafe {
        env::remove_var("RUSTUP_TOOLCHAIN");
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
        RynzlandSubcmd::CompAdd(comp_add) => comp_add.run()?,
        RynzlandSubcmd::CompRm(comp_rm) => comp_rm.run()?,
    }

    Ok(())
}
