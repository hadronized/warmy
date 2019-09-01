# 0.13

> Sun Sep 1st 2019

## Minor changes

  - Add the universal implementation for the [ron](https://crates.io/crates/ron) crate. It is
    accessible via the `Ron` type.

# 0.12

> Sat Jun 8th 2019

  - Switch from [tempdir](https://crates.io/crates/tempdir) to
    [tempfile](https://crates.io/crates/tempdir) in `dev-dependencies`.
  - Add the universal implementation for the [toml](https://crates.io/crates/toml) crate. It is
    accessible via the `Toml` type.

## 0.11.3

> Thu Apr 25th 2019

  - Update the README with [cargo-sync-readme](https://crates.io/crates/cargo-sync-readme).

## 0.11.2

> Wed Apr 24th 2019

  - Add the `"arc"` feature gate, allowing to replace the internal representation of resources by
    `Arc`-ed objects in order to help sending resources accross threads. This is a temporary patch
    until a better solution shows up.

## 0.11.1

> Thu Jan 24th 2019

  - Add universal JSON support (via **serde**).

# 0.11

> Saturday, 27th of October 2018

  - Change the key system. Keys are now used as type variables in the `Load` trait in order to allow
    for custom keys to be more easily used across an entire `Store` use through the code base. This
    is also the first premise of a bigger change that will arrive in `0.12` or `0.13`: resource
    sources and event collectors customization.
  - Remove `FSKey`, `LogicalKey`, `DepKey`.
  - Add the convenient `SimpleKey` type.
  - Enhance the documentation.

# 0.10

> Sunday, 30th of September 2018

  - Replace and remove the `std::error::Error` constraint by `Display` in the `Error` associated
    type of the `Load` trait.
  - Enhance the implementation of the `Display` trait for several types.
  - Implement `Display` for the `DepKey` type.

# 0.9

> Tuesday, 25th of September 2018

  - Add the resource discovery mechanism.
  - Change some internal code about debounced events. That has the effect to change the interface of
    the debounce duration’s type, going from *milliseconds* as `u64` to a more common and pleasant
    type to work with: `std::time::Duration`.

# 0.8

> Monday, 13th of August 2018

  - Fix a typo in the `RootDoesNotExist` error type.

## 0.7.3

> Tuesday, 24th of July 2018

  - Overall documentation enhancement.
  - Add the `Inspect` trait. This trait is there to help people make libraries using **warmy** more
    composable.

## 0.7.2

> Monday, April, 30th 2018

  - Fix a typo in the README.md. (am I drunk or what?)

## 0.7.1

> Monday, April, 30th 2018

  - Fix a typo in the README.md.

# 0.7

> Monday, April, 30th 2018

  - Refactor and reworked all the key system. The new system implements a
    [VFS](https://en.wikipedia.org/wiki/Virtual_file_system) and is easier to use – among important
    changes: the functional dependency between a key and the resource it points to was removed and is
    now injected by the implementation.
  - Add a `rustfmt.toml` file to the project. This is an experiment only to see whether it makes it
    easier to collaborate.
  - Add context passing. That enables situations where a mutable reference can be passed to a
    loading or even reloading resource, allowing for several interesting situations (loading /
    reloading statistics, tuning, etc.).
  - Add loading and reloading methods. Those are tag-only type variables that can be used to
    implement `Load` several times for a same type `T`, giving it the possibility to load or reload
    via several algorithms (JSON, YAML, custom, etc.).
  - Complete rewrite of the documentation. The documentation index (at the crate level) now contains
    a pretty detailed and as exhaustive as possible about **warmy** and everything that can be done
    with it. (hint: if you’re a developer and state that something is missing, please open an issue or
    even better, please open a merge request if you have spare time!).

> A gigantic **thank you** to [@icefoxen](https://github.com/icefoxen) for all their contributions
> to the crate, especially the context passing feature (it was their idea!) and all the testing they
> have done – `warmy` was tested with success with [ggez](https://crates.io/crates/ggez); how cool
> is that!

## 0.6.1

> Saturday, April, 7th 2018

  - Add functions to build both `PathKey` and `LogicalKey`.

# 0.6.0

  - The `update_await_time_ms` `StoreOpt` value is now defaulted to **50ms**. You must think of it as:
    “If a resource gets written, if nothing happens for the next `update_await_time_ms`, reload it.”
    You are free to change that value and experiment with it. However, keep in mind that a too much
    high value would result in latency, and that a too much low value could miss give you an incorrect
    behavior. To understand that, think of a copy of a large resource (a texture for instance). It’s
    very likely that the resource will be stream-copied to the file system, generating several write
    file system event that `warmy` will see. If the time between each write is higher than the value
    of `update_await_time_ms`, the reload code will be ran while the resource is still being
    stream-copied! Thus, **50ms** seems pretty fair (it’s actually pretty high, but you never know).
  - Interface change: you now handle a `Store` around, but the `Load` code handles a
    `Storage` instead of a `Store`. This is needed to enable partial borrowing
    optimizations.
  - Fix a bug for long-lasting reloading resources and OS bytes chunks streaming.
  - Complete rewrite of internals via partial borrowing and thus, way less allocations.

## 0.5.2

  - Fix premature dependency drop when reloading a resource.

## 0.5.1

  - In `Load::reload`, change the `_: &Self` into `&self`. Sorry for that. :D

# 0.5.0

  - Fix upper-bounds for notify dependency.
  - Introduce *logical resources*. Those are resources that don’t *directly* map to a path in the file
    system, yet require hot-reloading and caching.
  - Because of *logical resources*, the `Load` trait also get reviewed: the `from_fs` function now
    becomes `load` and doesn’t take a `Path`-ref-like value anymore, but depends on the kind of key
    your type selects via the associated `Key` type.
  - Various fixes for dependencies.
  - Documentation enhancement and update.

# 0.4.0

  - Disable people from performing *path sharing*. It is now forbidden to have two separate
    (different types) resources pointing to the same path. You’ll get errors when trying to get the
    second resource.

# 0.3.0

  - Fix paths handled to the `from_fs` method. The paths are now correctly prefixed by the
    canonicalized root.

# 0.2.0

  - Overall enhancement of the documentation.
  - New error system based on `::std::error::Error` and custom error.
  - Various `notify` fixes.

# 0.1.0

  - Initial revision.
