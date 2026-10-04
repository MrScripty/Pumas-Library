//! Independent worker outcomes remain owned until the caller consumes them.

use std::sync::{Arc, Mutex};

#[derive(Default)]
enum Outcome {
    #[default]
    Pending,
    Completed(Option<String>),
    Observed,
}

#[derive(Clone, Default)]
pub(super) struct OperationReceipt(Arc<Mutex<Outcome>>);

impl OperationReceipt {
    pub(super) fn complete<T>(&self, result: &pumas_library::Result<T>) {
        *self.0.lock().unwrap() =
            Outcome::Completed(result.as_ref().err().map(ToString::to_string));
    }

    pub(super) fn observe(&self) {
        *self.0.lock().unwrap() = Outcome::Observed;
    }

    pub(super) fn retain(&self) -> bool {
        matches!(
            *self.0.lock().unwrap(),
            Outcome::Pending | Outcome::Completed(Some(_))
        )
    }

    pub(super) fn unobserved_error(&self) -> Option<String> {
        match &*self.0.lock().unwrap() {
            Outcome::Completed(error) => error.clone(),
            Outcome::Pending | Outcome::Observed => None,
        }
    }
}
