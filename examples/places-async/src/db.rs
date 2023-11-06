/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use super::Bookmark;
use crate::network;
use parking_lot::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

/// Simulate an SQLite transaction, which only allows one writer at a time
static DB_MUTEX: Mutex<()> = Mutex::new(());
struct Transaction<'a>(MutexGuard<'a, ()>);

impl<'a> Transaction<'a> {
    /// Start a new transaction
    fn new() -> Self {
        Self(DB_MUTEX.lock())
    }

    /// Commit a transaction
    ///
    /// Writers call this after they have completed their operation.
    ///
    /// This is also called periodically during a sync to give other writers a chance.
    fn commit(self) {
        drop(self.0)
    }
}

pub struct Connection {
    staged: Vec<Bookmark>,
}

impl Connection {
    pub fn new() -> Self {
        Self { staged: vec![] }
    }

    /// Simulate inserting a new bookmark
    ///
    /// This is an example of a small/fast write
    pub fn insert_bookmark(&mut self, _bookmark: Bookmark) {
        let tx = Transaction::new();
        println!("write: inserting bookmark");
        thread::sleep(Duration::from_millis(100));
        tx.commit();
    }

    /// Simulate syncing bookmarks
    ///
    /// This is an example of a long-running operation that uses multiple transactions, committing
    /// at set periods of time to give writers a chance to run.
    pub fn sync(&mut self) {
        println!("sync: started");
        let incoming = network::fetch_incoming();
        for chunk in incoming.chunks(25) {
            self.stage_incoming(chunk);
        }
        let outgoing = self.merge();
        network::upload_outgoing(outgoing);
        println!("sync: finished");
    }

    /// Simulate staging incoming records in the local database for later processing
    fn stage_incoming(&mut self, incoming: &[Bookmark]) {
        let tx = Transaction::new();
        println!("sync: staging incoming chunk");
        self.staged.extend(incoming.into_iter().cloned());
        thread::sleep(Duration::from_millis(500));
        tx.commit();
    }

    /// Simulate merging the staged incoming records with existing records
    ///
    /// Returns outgoing bookmarks to upload to the sync server
    fn merge(&mut self) -> Vec<Bookmark> {
        let tx = Transaction::new();
        println!("sync: Merging bookmarks");
        thread::sleep(Duration::from_millis(1000));
        let outgoing = self
            .staged
            .drain(..)
            .enumerate()
            .filter_map(|(i, bookmark)| (i % 2 == 0).then_some(bookmark))
            .collect();
        tx.commit();
        outgoing
    }
}
