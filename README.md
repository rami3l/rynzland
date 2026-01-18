# Rynzland

This is another playground for me to tinker with ideas of addressing the
(in)famous [rustup#988].

Different from the previous take in [`rami3l/noife`] which is focused on
designing a locking mechanism, this project concentrates on redesigning the
transaction system itself to work with FS links referencing actual toolchains in
a pool, which might be more atomic and less error-prone than adding a layer
above the current transaction mechanisms.

## Rationale

The current transaction system in rustup relies on backing up the current
toolchain and logging all changes made during a toolchain modification in a
journal. If the transaction fails, the journal is replayed in reverse to restore
the previous state. However, this doesn't play very well with multiple rustup
instances working on the same set of toolchains, as they might very likely clash
with each other during the transaction process.

To address this, this project proposes a different approach where all modifiable
toolchains are stored in a pool and referenced via per-channel FS links. When a
toolchain modification is needed, the toolchain is first installed into the
pool, then a new (per-channel) FS link is (often atomically) created pointing to
the new toolchain, and finally the link is (often atomically) overwriting-moved
to the final position. This way, multiple rustup instances working on the same
pool can agree on which toolchains are being modified.

At the same time, the introduction of a pool of toolchains also opens up the
possibility of sharing toolchains between multiple rustup channels, which will
hopefully address some long-standing issues regarding rustup's disk space usage.

## Checklist

This new system is still a work in progress, and many details need to be sorted
out. Currently, it is designed as simulating `rustup` wrapper that runs a single
version-pinned `rustup` binary on two different instances of `$RUSTUP_HOME`, one
for the pool and another for the proxies.

Some basic features that should be implemented include:

- [x] Initial setup for both instances.
- [x] Installing a new toolchain.
- [x] Removing an existing toolchain.
- [ ] Garbage-collecting unreferenced toolchains in the pool.
- [ ] Uniquely identifying toolchains in the pool based on version and
      components.
- [ ] Identifying channels that can be updated.
- [ ] Updating a given channel.
  - [ ] ... to the same menifest, i.e. component addition or removal.
  - [ ] ... to a different manifest, i.e. minor or patch version update.

[rustup#988]: https://github.com/rust-lang/rustup/issues/988
[`rami3l/noife`]: https://github.com/rami3l/noife
