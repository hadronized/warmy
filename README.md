# warmy, hot-reloading loadable and reloadable resources

warmy is a crate that helps you introduce hot-reloading in your software.

A resource is a (possibly) disk-cached object that can be hot-reloaded while you use it.
Resources can be serialized and deserialized as you see fit. The concept of *caching* and
*loading* are split in different code locations so that you can easily compose both – provide
the loading code and ask the resource system to cache it for you.

This flexibility is exposed in the public interface so that the cache can be augmented with
user-provided objects. You might be interested in implementing `Load` and `CacheKey` – from
the [any-cache](https://crates.io/crates/any-cache) crate.

In order to have hot-reloading working, you have to call the `Store::sync` function that will
perform disk syncing. This function will unqueue disk events.

> Note: this is not the queue used by the underlying library (depending on your platform; for
> instance, inotify). This queue cannot, in theory, overflow. It’ll get bigger and bigger if you
> never sync.

# Key wrapping

If you use the resource system, your resources will be cached and accessible by their keys. The
key type is not enforced. Resource’s keys are typed to enable namespacing: if you have two
resources which ID is `34`, because the key types are different, you can safely cache the
resource with the ID `34` without any clashing or undefined behaviors. More in the any-cache
crate.

# Borrowing

Because the resource you access might change at anytime, you have to ensure you are the single
one handling them around. This is done via the `Rc::borrow` and `Rc::borrow_mut` functions.

> Important note: keep in mind `Rc` is not `Send`. This is a limitation that might be fixed in
> the near future if it’s a wanted feature.
