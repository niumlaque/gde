mod copy;
pub mod git;

pub use copy::FilesCopy;

pub struct Defer<F: FnOnce()> {
    f: Option<F>,
}

impl<F: FnOnce()> Defer<F> {
    pub fn new(f: F) -> Self {
        Self { f: Some(f) }
    }

    pub fn cancel(&mut self) {
        self.f = None;
    }
}

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(f) = self.f.take() {
            f();
        }
    }
}
