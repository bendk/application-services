/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

 use parking_lot::RwLock;
 //use std::sync::atomic::{AtomicU32, Ordering};
 //use std::sync::{Arc, Mutex};


pub trait PersistCallback: Sync + Send {
    fn persist(&self, context_id: String, creation_date: i64);
}

// ApplicationErrorReporter to use if the app doesn't set one
struct DefaultPersistCallback;
impl PersistCallback for DefaultPersistCallback {
    fn persist(&self, _context_id: String, _creation_date: i64) {}
}

lazy_static::lazy_static! {
    // RwLock rather than a Mutex, since we only expect to set this once.
    pub(crate) static ref PERSIST_CALLBACK_HANDLE: RwLock<Box<dyn PersistCallback>> = RwLock::new(Box::new(DefaultPersistCallback));
}

pub fn set_persist_callback(reporter: Box<dyn PersistCallback>) {
    *PERSIST_CALLBACK_HANDLE.write() = reporter;
}

pub fn unset_persist_callback() {
    *PERSIST_CALLBACK_HANDLE.write() = Box::new(DefaultPersistCallback)
}

pub fn persist_with_callback(client_id: String, creation_date: i64) {
    PERSIST_CALLBACK_HANDLE
        .read()
        .persist(client_id, creation_date);
}
