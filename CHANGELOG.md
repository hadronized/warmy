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
