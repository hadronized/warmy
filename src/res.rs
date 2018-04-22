use std::{
  cell::RefCell,
  ops::{Deref, DerefMut},
  rc::Rc,
};

/// Resources are wrapped in this type.
#[derive(Debug)]
pub struct Res<T>(Rc<RefCell<T>>);

impl<T> Clone for Res<T> {
  fn clone(&self) -> Self {
    Res(self.0.clone())
  }
}

impl<T> Res<T> {
  pub fn new(t: T) -> Self {
    Res(Rc::new(RefCell::new(t)))
  }
}

impl<T> Deref for Res<T> {
  type Target = Rc<RefCell<T>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T> DerefMut for Res<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
