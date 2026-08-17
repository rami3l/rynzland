# Proposal: Locked Rustup via a Toolchain Pool

## Status Quo

This proposal tries to address several existing issues with the current rustup
implementation (as of rustup v1.29.0), as listed below:

### Conflicts with Concurrent Toolchain Modifications

In rustup's list of open issues, [rust-lang/rustup#988] is a longstanding one.
The problem is very simple: rustup as it currently stands has no defense
mechanisms against concurrent changes to the same toolchain.

When installing/updating/uninstalling a toolchain, a "transaction" is being
used. Quoting the doc string of the `Transaction` type:

> A Transaction tracks changes to the file system, allowing them to be rolled
> back in case of an error. Instead of deleting or overwriting file, the old
> copies are moved to a temporary folder. If the transaction is rolled back,
> they will be moved back into place. If the transaction is committed, these
> files are automatically cleaned up using the temp system.
>
> All operations that create files will automatically create any intermediate
> directories in the path to the file if they do not already exist.
>
> All operations that create files will fail if the destination already exists.

It is clear that this approach assumes rustup's full control of the installation
directory, namely `$RUSTUP_HOME` and its subdirectories as well as the "bindir"
(currently `$CARGO_HOME/bin`), but this assumption is flawed in that it has
completely ignored the possible existence of another rustup process.

When two concurrently-running rustup processes try to install/update/uninstall
the same target toolchain, clashes will inevitably happen and certain failures
will occur. The possible symptoms are quite diverse, but the most typical error
message is along these lines ([rust-lang/rustup#4465]):

> ```console
> [..]
> info: installing component 'cargo'
> info: rolling back changes
> error: failed to install component: 'cargo-x86_64-unknown-linux-gnu', detected conflict: 'bin/cargo'
> ```

... but it can also take a different form such as one of the following:

- `error: "C:\Users\runneradmin\.cargo\bin\rustup.exe" is not a valid subcommand[..]`
  ([rust-lang/rustup#3709])
- `error: failed to install component: 'rust-src', detected conflict: 'lib/rustlib/src/rust/Cargo.lock'`
  ([rust-lang/rustup#3716])
- `error: the 'cargo' binary, normally provided by the 'cargo' component, is not applicable to the '1.78.0-x86_64-unknown-linux-gnu' toolchain`
  ([rust-lang/rust-clippy#12763])

... and this often leaves the user confused, especially because there is no
clear indications of what might have gone wrong.

It should be emphasized that this is not an arcane edge case, but a quite common
source of frustration for regular users.

Notably, at the moment of writing, rustup's [implicit installation] of the
active toolchain is still enabled by default for proxy binaries like `cargo`.
This means e.g. running `cargo build` twice on the same Rust project directory
with no active toolchain installed beforehand may result in two concurrent
rustup processes trying to install the same toolchain, and this is before
`cargo` is actually invoked. As a result, the user might end up with broken
toolchain installations.

To make things worse, one of those `cargo` invocations is commonly performed by
`rust-analyzer` from the IDE, and the user might not even be aware of the
situation when it happens.

[implicit installation]:
  https://blog.rust-lang.org/inside-rust/2026/07/03/rustup-update-1.30/#refining-the-implicit-installation-behavior

Given the above background, a new transaction mechanism is needed to ensure that
only one rustup process can modify one nominally-unique toolchain at a time.

### Problem Recovering from Interrupted Transactions

Another downside of the current transactions being non-atomic is that your Rust
installation is not guaranteed to be valid at all times. When you interrupt an
ongoing transaction, rustup might get
[confused](https://github.com/rust-lang/rustup/issues/4400) and don't know how
to recover.

### Duplicate Toolchain Installations

Rustup has the notion of "moving target" channels (e.g. `stable`, `1.92`) and
"fixed target" channels (e.g. `1.92.0`), and sometimes they may happen to have
the same contents. In such cases, we would want to prevent rustup from
installing the same toolchain twice and simply
[use one single copy](https://github.com/rust-lang/rustup/issues/4663) of the
toolchain in question instead, at least until the referees of the channels
diverge again.

## Proposed Solution

The solution proposed below is heavily inspired by programming language runtime
concepts, so we will start by introducing those concepts and how they map to the
entities in rustup.

The overall idea is to separate this problem into two different sets:

- That of objects. In rustup, an object is an actual toolchain with all the
  components we want it to contain, corresponding to an FS directory located at
  `$RUSTUP_HOME/heap/<object-identifier>`.
- That of references. In rustup, a reference is a toolchain name, which
  corresponds to an FS link[^fs-link] located at
  `$RUSTUP_HOME/toolchains/<toolchain-name>`. In any normal state, a reference
  should point to an existing object, but in transient states, it can also be
  dangling as long as it points to a valid object at the end of a transaction,
  as we will see later in this introduction.

[^fs-link]:
    Here we only consider symlinks or NTFS junctions to a directory, since
    toolchains are all directories. Today's rustup already supports those
    natively, see `rustup toolchain link --help`.

At any point in time, the members of those two sets are of a finite amount and
can be enumerated by walking the respective directory.

Continuing with our analogy, the directory of objects will be conveniently
called the heap from now on. It is thus natural to think about interning the
heap to reduce disk space waste as long as they are immutable if we can find an
identifying mechanism. In this proposal, an object is identified by the
following factors:

- The Rust version string[^manifest-hash].
- The ordered collection of fully qualified component names before renaming.

[^manifest-hash]:
    We cannot use the checksum of the manifest file here, unfortunately, because
    `1.92.0` and `1.92` can have the same manifest with different checksums, for
    example. That non-determinism is a reality in ~~our release server~~ certain
    release servers and there is nothing we can do to change what has been
    published in the past. Fortunately, the official release server seems to
    provide deterministic manifest checksum today, which might constitute an
    interesting fast path in certain scenarios.

Then, when we update a toolchain, we would take the
["functional update"](https://doc.rust-lang.org/stable/reference/expressions/struct-expr.html#r-expr.struct.update)
approach so the objects would stay immutable all the time. What we actually do
could be to, in chronological order:

- Declare the beginning of the transaction.
- Construct a new object in the heap that satisfies our needs (with a
  same/different Rust version and/or a same/different set of components).
- Alter the reference so that it points to the newly constructed object.
- Declare the end of the transaction.

Declaring the beginning and the end of the transaction with particular actions
is necessary to ensure that the new transaction scheme will not be vulnerable to
the concurrent execution of another rustup instance. As we will see later, this
is mostly about acquiring and releasing the reader part of a reader/writer lock.

For example, given a toolchain name `stable`, two versions `v1` and `v2`, and
`stable` initially pointing to `v1`, we can express this situation with
Rust-like pseudo-code like so (note that this pseudo-language has no name
shadowing):

```rust=
let v1: Toolchain = make_v1();
let v2: Toolchain;
let mut stable: &Toolchain = &v1;
```

If we want to upgrade it to `v2`, so that in the end:

```rust=
stable == &v2
```

Below is the list the actions to be performed in chronological order (modulo the
locking operations):

```rust=
let stable_tmp = &v2;
let v2_tmp: Toolchain = make_v2();
v2 = v2_tmp; // FS move/rename `v2_tmp` to `v2`.
stable = stable_tmp;
```

In other words, in a transaction from `v1` to `v2`, we:

- Create a new reference `stable_tmp` that points to the _updated_ object.
  - Please note that we do know where `v2` should end up with (`&v2`) because we
    can calculate its identifier ahead of time:
    - Its Rust version string can be fetched locally if modifying an existing
      toolchain, or be pulled from the release manifest when installing/updating
      the toolchain.
    - Its component list is already known, since rustup must be aware of such
      information to proceed with the transaction.
  - This way, if another rustup instance tries to modify `stable`, the creation
    of `stable_tmp` will fail immediately, since the creation of references (FS
    links) should often be atomic.[^atomic]
- Start functionally updating `v1` to `v2` by creating in the heap an
  uninitialized object `v2_tmp` and initializing it in place.
  - Similarly, this prevents another rustup instance from updating _to_ the same
    object, since the creation of objects (FS directories) should often be
    atomic.[^atomic]
- When the construction is finished, move `v2_tmp` to `v2`. This is atomic on
  most environments.[^atomic]
- Finally, move `stable_tmp` to `stable`. This should also be atomic on most
  environments.[^atomic]

[^atomic]:
    See
    [here](https://rcrowley.org/2010/01/06/things-unix-can-do-atomically.html)
    for a short list of atomic FS operations on Unix. I have mostly used
    operations that are more or less atomic above, but if anything turns out to
    be otherwise, we can always use a heap-global FS lock of some sort.

The above has already covered the actions of installing a new toolchain as well
as modifying an existing toolchain.

As for uninstalling a toolchain (e.g. `stable`), it is also very simple:

`rust= forget(stable) // Unlink/remove the FS link "stable". `

... of course that is only theoretically correct since it might be leaking its
referee if the latter is unreachable. To actually reclaim disk space, we need a
GC mechanism for the heap.

A trivial full GC means removing all the unreachable objects in the heap, and to
do so, a scan of all references as well as another scan of the whole heap are
required upfront in order to compare the set of referees with what we have in
the heap.

<!-- A partial GC means removing all `Toolchain`s in a candidate set that turn
out to be unreachable. It might be useful when the user has a lot of
toolchains. -->

The GC is of course not atomic in anyway, so it should be triggered behind a
heap-global GC lock.

Furthermore, to avoid potential
[ToC/ToU](https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use)
inconsistencies regarding the two scans in a GC cycle (one that scans over all
references, and the other over the heap), we need to introduce a reader/writer
lock which is globally-applied to the combination of two sets; a read lock is
required whenever any set in the combination is written to, with the only
exception during a GC where a write lock is required.

The GC can be most eagerly triggered after each write to the set, namely after
each toolchain update or uninstallation. (There doesn't seem to be a need for GC
after installing a new toolchain, though.)

With every crucial action guarded behind atomic FS operations and/or named
locks, it should be safe to say that if any conflict (other than the few
intentionally produced that we can easily special-case) does occur, it should
not come from rustup itself.

## Discussion

### File Leaking

This type of transaction proposed here might leak files/folders when being
interrupted in the middle, but it should be okay to leave it to the user's
discretion: this is also what Git and APT do with their "lockfiles".

During a full GC, however, the remaining in-flight objects from the previous
interrupted transactions in the heap can be easily cleaned up since the GC
cannot happen with a transaction in place (thanks to the lock). The same should
also apply to in-flight references (since they are supposed to have a special
name format).

### Implementation of the Lock

On Unix systems, a named reader/writer lock can be easily implemented via the
`fcntl()` locking APIs on a dummy file. The potential "close one, lose all" flaw
with POSIX `fcntl()`[^fcntl-flaw] will not cause problems for us since we are
not actually interested in `close()`ing the file (we will conveniently place it
in a tempdir).

[^fcntl-flaw]:
    As per the
    [man page](https://man7.org/linux/man-pages/man2/fcntl_locking.2.html), if a
    process closes any file descriptor referring to a file, then all of the
    process's locks on that file are released, regardless of the file
    descriptor(s) on which the locks were obtained.

On Windows, there is no `fcntl()` equivalent. Instead, we will use a hand-rolled
reader/writer lock implemented
[with a named mutex and a named semaphore](https://stackoverflow.com/a/640368).
The resulting lock is not very performant, but it should be okay for rustup
since we don't expect a lot of contentions.

### Why Attacking Multiple Issues in a Single Proposal?

Our most important design requirement is that a Rust installation should be
valid 100% of the time to ensure recovery from interrupted transaction. The best
recovery strategy is none at all: we can design the transaction in such a way
that it can put the Rust installation in a transient state, but that state must
be easily identifiable, and more importantly, it must not alter the existing
toolchains in any non-atomic way.

The introduction of a staging area would be a natural consequence of that
requirement. In this proposal, our main staging area would be the heap. In the
transient state, there will be in-flight objects in the heap, but they remain
easily distinguishable since they are not reachable until the transaction
finishes. At the same time, the introduction of the indirection layer of the
references has made it easier to achieve atomicity on most platforms.

Next up, to prevent clashes when concurrently modifying the same toolchain, we
would like to add some sort of per-toolchain locking mechanism, so that only one
process will succeed in initiating the transaction. In our design, this mutual
exclusion is achieved via the creation of the in-flight reference, which is
performed on a per toolchain name-basis.

Finally, now that we have already introduced the references, we only need to
find a way for the objects to be content-addressable to deduplicate different
copies of the same object.
