/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::{future, sync::Arc};

use parking_lot::Mutex;

/// Worker queue scheduler trait
///
/// This is implemented by foreign code to schedule blocking Rust tasks:
///   - On Swift, it's implemented with a `DispatchQueue`
///   - On Kotlin, it's implemented with a `CoroutineContext`
///   - On Gecko-JS, it's implemented in Rust using the `moz_task` crate.
#[uniffi::export(with_foreign)]
pub trait WorkerQueue: Send + Sync {
    fn add_task(&self, task: Arc<dyn RustTask>);
}

#[uniffi::export]
pub trait RustTask: Send + Sync {
    fn run(&self);
}

/// Schedule a closure to run in the global worker queue.  Returns the result of the closure
/// asynchronously
pub async fn run_in_background<T: Send + 'static>(
    worker_queue: Arc<dyn WorkerQueue>,
    task: impl FnOnce() -> T + Send + Sync + 'static,
) -> T {
    let (tx, rx) = oneshot::channel();

    worker_queue.add_task(RustTaskContainer::new_arc(move || {
        if let Err(e) = tx.send(task()) {
            error_support::report_error!("suggest-oneshot-send", "{e}");
        }
    }));
    match rx.await {
        Ok(v) => v,
        Err(e) => {
            error_support::report_error!("suggest-oneshot-recv", "{e}");
            // Not much we can do here other than await forever
            future::pending().await
        }
    }
}

/// Implements RustTask for any closure
struct RustTaskContainer<T: FnOnce() + Send> {
    /// The one tricky part is that the task can only be run once, but the foreign language gets a
    /// shared reference to it, so put it behind a Mutex + Option
    task: Mutex<Option<T>>,
}

impl<T: FnOnce() + Send> RustTaskContainer<T> {
    fn new_arc(task: T) -> Arc<Self> {
        Arc::new(Self {
            task: Mutex::new(Some(task)),
        })
    }
}

impl<T: FnOnce() + Send> RustTask for RustTaskContainer<T> {
    fn run(&self) {
        if let Some(f) = self.task.lock().take() {
            f()
        }
    }
}
