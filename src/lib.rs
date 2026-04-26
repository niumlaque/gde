mod auto;
mod copy;
pub mod git;

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

pub use auto::AutoCopy;
pub use copy::FilesCopy;

#[cfg(test)]
pub(crate) mod testutil {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    pub(crate) fn git_test_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        match LOCK.get_or_init(|| Mutex::new(())).lock() {
            Ok(lock) => lock,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}
