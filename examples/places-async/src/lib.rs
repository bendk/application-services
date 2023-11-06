/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use async_mutex::Mutex;

mod db;
mod network;
use db::Connection;

use uniffi::BlockingTaskQueue;

pub struct PlacesApi {
    queue: BlockingTaskQueue,
    write_connection: Mutex<Connection>,
    sync_connection: Mutex<Connection>,
}

impl PlacesApi {
    pub fn new(queue: BlockingTaskQueue) -> Self {
        println!("creating PlacesApi");
        Self {
            queue,
            write_connection: Mutex::new(Connection::new()),
            sync_connection: Mutex::new(Connection::new()),
        }
    }

    pub async fn insert_bookmark(&self, bookmark: Bookmark) {
        let mut conn = self.write_connection.lock().await;
        self.queue
            .run_blocking(|| conn.insert_bookmark(bookmark))
            .await;
    }

    pub async fn sync(&self) {
        let mut conn = self.sync_connection.lock().await;
        self.queue.run_blocking(|| conn.sync()).await;
    }
}

#[derive(Clone, Debug)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
}

uniffi::include_scaffolding!("places");
