mod prelude;

use prelude::*;
use serial_test::serial;

use super::*;
use crate::{toolchain, toolchain::IdentifiableToolchain};

#[test]
#[serial]
fn setup_and_nuke() -> Result<()> {
    let ctx = Ctx::setup()?;
    let home = ctx.home();

    // Rustup should exist now.
    let rustup_path = home.join("rustup");
    assert!(rustup_path.try_exists()?);
    let rustup_link_path = home.join("cargo_home").join("bin").join("rustup");
    assert!(rustup_link_path.try_exists()?);

    // So is `rustup_home/settings.toml` and `rynzland_home/settings.toml`.
    let rustup_settings_path = home.join("rustup_home").join("settings.toml");
    assert!(rustup_settings_path.try_exists()?);
    let rynzland_settings_path = home.join("rynzland_home").join("settings.toml");
    assert!(rynzland_settings_path.try_exists()?);

    NukeSubcmd {}.run()?;

    // The home dir should be empty again.
    let entries: Vec<_> = fs::read_dir(&home)?.collect();
    assert!(entries.is_empty(), "home dir not empty: {entries:#?}");

    drop(ctx);
    Ok(())
}

#[test]
#[serial]
fn toolchain_id() -> Result<()> {
    let ctx = Ctx::setup()?;
    let home = ctx.home();
    let rynzland_home = home.join("rynzland_home");

    // Use a specific version to be deterministic about what `stable` points to.
    let minor = "1.92";
    let patch = "1.92.0";

    // Add a versioned-based toolchain.
    AddSubcmd {
        toolchain: minor.into(),
        source: None,
    }
    .run()?;

    let tc_path = rynzland_home
        .join("toolchains")
        .join(util::qualify_with_target(minor).as_ref());
    let id_from_disk = IdentifiableToolchain::new(&tc_path)?.id();
    let id_from_remote = toolchain::resolve_channel(patch, &[])?.id();
    assert_eq!(id_from_disk, id_from_remote);

    let id_from_remote_nightly = toolchain::resolve_channel("nightly", &[])?.id();
    assert_ne!(id_from_disk, id_from_remote_nightly);

    drop(ctx);
    Ok(())
}

#[test]
#[serial]
fn toolchain_management() -> Result<()> {
    let ctx = Ctx::setup()?;
    let home = ctx.home();
    let rynzland_home = home.join("rynzland_home");

    // Use a specific version to be deterministic about what `stable` points to.
    let ver = "1.81.0";
    let chan = "stable";

    // Add a versioned-based toolchain.
    AddSubcmd {
        toolchain: ver.into(),
        source: None,
    }
    .run()?;

    let tc_link = rynzland_home
        .join("toolchains")
        .join(util::qualify_with_target(ver).as_ref());
    assert!(tc_link.exists(), "toolchain link should exist");

    // Check underlying toolchain in rustup_home.
    // `link_target` should be relative or absolute path to
    // `rustup_home/toolchains/<id>`.
    let link_target = fs::read_link(&tc_link)?;
    let underlying_path = if link_target.is_relative() {
        tc_link.parent().unwrap().join(link_target)
    } else {
        link_target
    };
    assert!(
        underlying_path.exists(),
        "underlying toolchain should exist"
    );

    let id_from_disk = IdentifiableToolchain::new(&underlying_path)?.id();

    // Check identification match (remote vs local)
    let id_from_remote = toolchain::resolve_channel(ver, &[])?.id();
    assert_eq!(
        id_from_disk, id_from_remote,
        "local and remote IDs should match"
    );

    // Add a channel-based toolchain pointing to same underlying toolchain.
    AddSubcmd {
        toolchain: chan.into(),
        source: Some(ver.into()),
    }
    .run()?;

    let chan_link = rynzland_home
        .join("toolchains")
        .join(util::qualify_with_target(chan).as_ref());
    assert!(chan_link.exists());
    let chan_target = fs::read_link(&chan_link)?;
    let chan_underlying = if chan_target.is_relative() {
        chan_link.parent().unwrap().join(chan_target)
    } else {
        chan_target
    };
    assert_eq!(
        fs::canonicalize(&underlying_path)?,
        fs::canonicalize(&chan_underlying)?,
        "underlying toolchain should be reused",
    );

    // Remove channel-based toolchain
    RmSubCmd {
        toolchain: chan.into(),
    }
    .run()?;
    assert!(
        !chan_link.exists(),
        "channel-based toolchain should be gone"
    );
    // Underlying should still exist because the other toolchain still uses it.
    assert!(underlying_path.exists(), "underlying should still exist",);

    // Remove final ref.
    RmSubCmd {
        toolchain: ver.into(),
    }
    .run()?;
    assert!(!tc_link.exists(), "original link should be gone");
    assert!(
        !underlying_path.exists(),
        "underlying toolchain should be removed",
    );

    drop(ctx);
    Ok(())
}

#[test]
#[serial]
fn update_toolchain_gc() -> Result<()> {
    let ctx = Ctx::setup()?;
    let home = ctx.home();
    let rynzland_home = home.join("rynzland_home");

    let stable = "stable";
    let v1 = "1.91.0";
    let v2 = "1.92.0";

    // Add stable from 1.91.0.
    AddSubcmd {
        toolchain: stable.into(),
        source: Some(v1.into()),
    }
    .run()?;

    let stable_link = rynzland_home
        .join("toolchains")
        .join(util::qualify_with_target(stable).as_ref());

    let link_target_v1 = fs::read_link(&stable_link)?;
    let underlying_v1 = if link_target_v1.is_relative() {
        stable_link.parent().unwrap().join(&link_target_v1)
    } else {
        link_target_v1
    };
    assert!(underlying_v1.exists(), "v1 toolchain should exist");

    // Update stable to 1.92.0.
    AddSubcmd {
        toolchain: stable.into(),
        source: Some(v2.into()),
    }
    .run()?;

    let link_target_v2 = fs::read_link(&stable_link)?;
    let underlying_v2 = if link_target_v2.is_relative() {
        stable_link.parent().unwrap().join(&link_target_v2)
    } else {
        link_target_v2
    };

    assert!(underlying_v2.exists(), "v2 toolchain should exist");
    assert_ne!(
        underlying_v1, underlying_v2,
        "underlying toolchains should be different"
    );

    assert!(
        !underlying_v1.exists(),
        "v1 toolchain should have been GC'd"
    );

    drop(ctx);
    Ok(())
}

#[test]
#[serial]
fn comp_add_rm() -> Result<()> {
    let ctx = Ctx::setup()?;
    let home = ctx.home();
    let rynzland_home = home.join("rynzland_home");

    let toolchain_name = "1.78";

    // Add stable toolchain
    AddSubcmd {
        toolchain: toolchain_name.into(),
        source: None,
    }
    .run()?;

    let link_path = rynzland_home
        .join("toolchains")
        .join(util::qualify_with_target(toolchain_name).as_ref());

    let resolve_underlying = |path: &std::path::Path| -> Result<std::path::PathBuf> {
        let link_target = fs::read_link(path)?;
        if link_target.is_relative() {
            Ok(path.parent().unwrap().join(link_target))
        } else {
            Ok(link_target)
        }
    };

    let underlying_1 = resolve_underlying(&link_path)?;
    assert!(underlying_1.exists(), "Underlying toolchain 1 should exist");

    // Cargo bin path in underlying toolchain
    let cargo_bin_1 = underlying_1.join("bin").join("cargo");
    assert!(cargo_bin_1.exists(), "Cargo should exist initially");

    // Remove cargo
    CompRmSubcmd {
        toolchain: toolchain_name.into(),
        components: vec!["cargo".into()],
    }
    .run()?;

    let underlying_2 = resolve_underlying(&link_path)?;
    assert_ne!(
        underlying_1, underlying_2,
        "Should point to new underlying toolchain"
    );

    assert!(!underlying_1.exists(), "Old toolchain should be GC'd");
    assert!(underlying_2.exists(), "New toolchain should exist");

    let cargo_bin_2 = underlying_2.join("bin").join("cargo");
    assert!(
        !cargo_bin_2.exists(),
        "Cargo should be gone in new toolchain"
    );

    // Add cargo back
    CompAddSubcmd {
        toolchain: toolchain_name.into(),
        components: vec!["cargo".into()],
    }
    .run()?;

    let underlying_3 = resolve_underlying(&link_path)?;
    // underlying_2 should be gone
    assert!(!underlying_2.exists(), "Second toolchain should be GC'd");
    assert!(underlying_3.exists(), "Third toolchain should exist");

    let cargo_bin_3 = underlying_3.join("bin").join("cargo");
    assert!(cargo_bin_3.exists(), "Cargo should be back");

    assert_eq!(
        underlying_1, underlying_3,
        "Should return to original toolchain ID/path"
    );

    drop(ctx);
    Ok(())
}
