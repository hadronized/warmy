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
