use anyhow::Result;
use rynzland::{Ctx, Rynzland};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    unsafe {
        std::env::remove_var("RUSTUP_TOOLCHAIN");
    }

    let app: Rynzland = argh::from_env();
    let ctx = Ctx::new("home");
    app.subcmd.run(&ctx)
}
