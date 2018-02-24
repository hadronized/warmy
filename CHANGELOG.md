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
