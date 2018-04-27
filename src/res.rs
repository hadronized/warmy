use std::{cell::{Ref, RefCell, RefMut},
          rc::Rc};

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

  pub fn borrow(&self) -> Ref<T> {
    self.0.borrow()
  }

  pub fn borrow_mut(&self) -> RefMut<T> {
    self.0.borrow_mut()
  }
}
