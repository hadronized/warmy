//! Shareable resources.

use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

/// Shareable resource type.
///
/// Resources are wrapped in this type. You cannot do much with an object of this type, despite
/// borrowing immutable or mutably its content.
#[derive(Debug)]
pub struct Res<T>(Rc<RefCell<T>>);

impl<T> Clone for Res<T> {
  fn clone(&self) -> Self {
    Res(self.0.clone())
  }
}

impl<T> Res<T> {
  /// Wrap a value in a shareable resource.
  pub fn new(t: T) -> Self {
    Res(Rc::new(RefCell::new(t)))
  }

  /// Borrow a resource for as long as the return value lives.
  pub fn borrow(&self) -> Ref<T> {
    self.0.borrow()
  }

  /// Mutably borrow a resource for as long as the return value lives.
  pub fn borrow_mut(&self) -> RefMut<T> {
    self.0.borrow_mut()
  }
}
