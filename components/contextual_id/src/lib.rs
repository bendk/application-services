/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

mod error;

pub use error::{ApiResult, Result, ApiError, Error};

use error_support::handle_error;
use chrono::{DateTime, Duration, Utc};
use parking_lot::Mutex;
use uuid::Uuid;

uniffi::setup_scaffolding!("context_id");

mod callback;
pub use callback::{
    persist_with_callback, set_persist_callback, unset_persist_callback,
    PersistCallback,
};

pub fn context_id_test_function() {
    println!("Holy cow here we go");
}

/// Top-level API for the context_id component
#[derive(uniffi::Object)]
pub struct ContextIDComponent {
    inner: Mutex<ContextIDComponentInner>,
}

#[uniffi::export]
impl ContextIDComponent {
    /// Construct a new [ContextIDComponent].
    ///
    /// If no creation timestamp is provided, the current time will be used.
    #[uniffi::constructor]
    #[handle_error(Error)]
    pub fn new(init_context_id: &str, creation_timestamp_s: i64) -> ApiResult<Self> {
        Ok(Self {
            inner: Mutex::new(
                ContextIDComponentInner::new(
                    init_context_id,
                    creation_timestamp_s,
                    Utc::now()
                )?
            ),
        })
    }

    /// Return the current context ID string.
    #[handle_error(Error)]
    pub fn request(&self, rotation_days_in_s: u8) -> ApiResult<String> {
        let mut inner = self.inner.lock();
        inner.request(rotation_days_in_s, Utc::now())
    }

    /// Return the current context ID string.
    #[handle_error(Error)]
    pub fn force_rotation(&self) -> ApiResult<()> {
        let mut inner = self.inner.lock();
        inner.force_rotation(Utc::now());
        Ok(())
    }
}

struct ContextIDComponentInner {
    context_id: String,
    creation_timestamp: DateTime<Utc>,
}

impl ContextIDComponentInner {
    pub fn new(init_context_id: &str, creation_timestamp_s: i64, now: DateTime<Utc>) -> Result<Self> {
        let (context_id, generated_context_id) = match init_context_id.trim() {
            "" => (Uuid::new_v4().to_string(), true),
            // If the passed in string isn't empty, but still not a valid UUID,
            // just go ahead and generate a new UUID.
            s => match Uuid::parse_str(s) {
                Ok(_) => (s.to_string(), false),
                Err(_) => (Uuid::new_v4().to_string(), true),
            },
        };

        let (creation_timestamp, generated_creation_timestamp) = if generated_context_id {
            // If we had to generate a UUID, also force a new timestamp.
            (now, true)
        } else {
            match creation_timestamp_s {
                secs if secs > 0 => (
                    DateTime::<Utc>::from_timestamp(secs, 0)
                        .ok_or(Error::InvalidTimestamp { timestamp: secs })?,
                    false,
                ),
                _ => (now, true),
            }
        };

        let instance = Self {
            context_id,
            creation_timestamp,
        };

        // We only need to persist these if we just generated one.
        if generated_context_id || generated_creation_timestamp {
            instance.persist();
        }

        Ok(instance)
    }

    pub fn request(&mut self, rotation_days: u8, now: DateTime::<Utc>) -> Result<String> {
        if rotation_days == 0 {
            return Ok(self.context_id.clone());
        }

        let age = now - self.creation_timestamp;
        if age >= Duration::days(rotation_days.into()) {
            self.rotate_context_id(now);
        }

        Ok(self.context_id.clone())
    }

    pub fn rotate_context_id(&mut self, now: DateTime::<Utc>) {
        self.context_id = Uuid::new_v4().to_string();
        self.creation_timestamp = now;
    }

    pub fn force_rotation(&mut self, now: DateTime::<Utc>) {
        self.rotate_context_id(now);
    }

    pub fn persist(&self) {
        persist_with_callback(self.context_id.clone(), self.creation_timestamp.timestamp());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    // 1745859061 ~= the timestamp for when this test was written (Apr 28, 2025)
    const FAKE_NOW_TS: i64 = 1745859061;
    const FAKE_NOW: DateTime::<Utc> = DateTime::<Utc>::from_timestamp(FAKE_NOW_TS, 0).unwrap();
    const TEST_CONTEXT_ID: &str = "decafbad-0cd1-0cd2-0cd3-decafbad1000";
    // 1706763600 ~= Jan 1st, 2024, which is long ago compared to FAKE_NOW.
    const FAKE_LONG_AGO_TS: i64 = 1706763600;
    const FAKE_LONG_AGO: DateTime::<Utc> = DateTime::<Utc>::from_timestamp(FAKE_LONG_AGO_TS, 0).unwrap();

    #[test]
    fn test_creation_timestamp_with_some_value() {
        let creation_timestamp = FAKE_NOW_TS;
        let component = ContextIDComponentInner::new(TEST_CONTEXT_ID, creation_timestamp, FAKE_NOW).unwrap();

        // We should have left the context_id and creation_timestamp
        // untouched if a creation_timestamp was passed.
        assert_eq!(component.context_id, TEST_CONTEXT_ID);
        assert_eq!(component.creation_timestamp.timestamp(), creation_timestamp);
    }

    #[test]
    fn test_creation_timestamp_with_zero_value() {
        let component = ContextIDComponentInner::new(TEST_CONTEXT_ID, 0, FAKE_NOW).unwrap();

        // If 0 was passed as the creation_timestamp, we'll interpret that
        // as there having been no stored creation_timestamp. In that case,
        // we'll use "now" as the creation_timestamp.
        assert_eq!(component.context_id, TEST_CONTEXT_ID);
        assert_eq!(component.creation_timestamp, FAKE_NOW);
    }

    #[test]
    fn test_empty_initial_context_id() {
        let component = ContextIDComponentInner::new("", 0, FAKE_NOW).unwrap();

        // We expect a new UUID to have been generated for context_id.
        assert!(Uuid::parse_str(&component.context_id).is_ok());
        assert_eq!(component.creation_timestamp, FAKE_NOW);
    }

    #[test]
    fn test_empty_initial_context_id_with_creation_date() {
        // It's possible that the caller passes in
        let component = ContextIDComponentInner::new("", 0, FAKE_NOW).unwrap();

        // We expect a new UUID to have been generated for context_id.
        assert!(Uuid::parse_str(&component.context_id).is_ok());
        assert_eq!(component.creation_timestamp, FAKE_NOW);
    }

    #[test]
    fn test_invalid_context_id_with_no_creation_date() {
        let component = ContextIDComponentInner::new("something-invalid", 0, FAKE_NOW).unwrap();

        // We expect a new UUID to have been generated for context_id.
        assert!(Uuid::parse_str(&component.context_id).is_ok());
        assert_eq!(component.creation_timestamp, FAKE_NOW);
    }

    #[test]
    fn test_invalid_context_id_with_creation_date() {
        let component = ContextIDComponentInner::new("something-invalid", FAKE_LONG_AGO_TS, FAKE_NOW).unwrap();

        // We expect a new UUID to have been generated for context_id.
        assert!(Uuid::parse_str(&component.context_id).is_ok());
        assert_eq!(component.creation_timestamp, FAKE_NOW);
    }

    #[test]
    fn test_request_no_rotation() {
        // Let's create a context_id with a creation date far in the past.
        let mut component = ContextIDComponentInner::new(TEST_CONTEXT_ID, FAKE_LONG_AGO_TS, FAKE_NOW).unwrap();

        // We expect neither the UUID nor creation_timestamp to have been changed.
        assert_eq!(component.context_id, TEST_CONTEXT_ID);
        assert_eq!(component.creation_timestamp, FAKE_LONG_AGO);

        // Now request the context_id, passing 0 for the rotation_days. We
        // interpret this to mean "do not rotate".
        assert_eq!(component.request(0, FAKE_NOW).unwrap(), component.context_id);
        assert_eq!(component.creation_timestamp, FAKE_LONG_AGO);
    }

    #[test]
    fn test_request_with_rotation() {
        // Let's create a context_id with a creation date far in the past.
        let mut component = ContextIDComponentInner::new(TEST_CONTEXT_ID, FAKE_LONG_AGO_TS, FAKE_NOW).unwrap();

        // We expect neither the UUID nor creation_timestamp to have been changed.
        assert_eq!(component.context_id, TEST_CONTEXT_ID);
        assert_eq!(component.creation_timestamp, FAKE_LONG_AGO);

        // Now request the context_id, passing 2 for the rotation_days. Since
        // the number of days since FAKE_LONG_AGO is greater than 2 days, we
        // expect a new context_id to be generated, and the creation_timestamp
        // to update to now.
        assert!(Uuid::parse_str(&component.request(2, FAKE_NOW).unwrap()).is_ok());
        assert_ne!(component.context_id, TEST_CONTEXT_ID);
        assert_eq!(component.creation_timestamp, FAKE_NOW);
    }

    #[test]
    fn test_force_rotation() {
        let mut component = ContextIDComponentInner::new(TEST_CONTEXT_ID, FAKE_LONG_AGO_TS, FAKE_NOW).unwrap();

        component.force_rotation(FAKE_NOW);
        assert!(Uuid::parse_str(&component.request(2, FAKE_NOW).unwrap()).is_ok());
        assert_ne!(component.context_id, TEST_CONTEXT_ID);
        assert_eq!(component.creation_timestamp, FAKE_NOW);
    }
}
