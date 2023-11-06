/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use super::Bookmark;
use std::thread;
use std::time::Duration;

/// Simulate fetching incoming bookmarks from the sync server
pub fn fetch_incoming() -> Vec<Bookmark> {
    println!("sync: Fetching incoming records...");
    thread::sleep(Duration::from_millis(1000));
    (0..100)
        .into_iter()
        .map(|i| Bookmark {
            url: format!("https://example.com/page/{i}"),
            title: format!("Page {i}"),
        })
        .collect()
}

/// Simulate upload outgoing bookmarks to the sync server
pub fn upload_outgoing(_bookmarks: Vec<Bookmark>) {
    println!("sync: Uploading outgoing records...");
    thread::sleep(Duration::from_millis(1000));
}
