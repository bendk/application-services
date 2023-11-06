/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use parking_lot::Mutex;

mod db;
mod network;
use db::Connection;

pub struct PlacesApi {
    write_connection: Mutex<Connection>,
    sync_connection: Mutex<Connection>,
}

impl PlacesApi {
    pub fn new() -> Self {
        println!("creating PlacesApi");
        Self {
            write_connection: Mutex::new(Connection::new()),
            sync_connection: Mutex::new(Connection::new()),
        }
    }

    pub fn insert_bookmark(&self, bookmark: Bookmark) {
        let mut conn = self.write_connection.lock();
        conn.insert_bookmark(bookmark);
    }

    pub fn sync(&self) {
        let mut conn = self.sync_connection.lock();
        conn.sync();
    }
}

#[derive(Clone, Debug)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
}

uniffi::include_scaffolding!("places");
