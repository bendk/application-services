/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::ffi::OsString;

pub type Result<T> = std::result::Result<T, LoginsError>;

// ------------
// Imagine these being in our error_support crate
#[macro_use]
pub mod error_crate {

    /// Describes what error reporting action should be taken.
    pub enum ErrorReporting {
        // No logging or error reporting.
        Nothing,
        // We write a log message but don't report it.
        Log {
            level: log::Level,
        },
        // We log a message and report via our error reporter.
        Report {
            level: log::Level,
            report_class: String,
        },
    }

    // Specifies how an "internal" error is converted to an "external" public error and
    // any logging or reporting that should happen.
    pub struct ErrorHandling<E> {
        // The external error that should be returned.
        pub err: E,
        // How the error should be reported.
        pub reporting: ErrorReporting,
    }

    pub trait GetErrorHandling {
        type ExternalError;

        // Return how to handle our internal errors
        fn get_error_handling(&self) -> ErrorHandling<Self::ExternalError>;

        // Some helpers to cut the verbosity down.
        // Just convert the error without any special logging or error reporting.
        fn passthrough(err: Self::ExternalError) -> ErrorHandling<Self::ExternalError> {
            ErrorHandling {
                err,
                reporting: ErrorReporting::Nothing,
            }
        }

        // Just convert and log the error without any special error reporting.
        fn log(err: Self::ExternalError, level: log::Level) -> ErrorHandling<Self::ExternalError> {
            ErrorHandling {
                err,
                reporting: ErrorReporting::Log { level },
            }
        }

        // Convert, report and log the error.
        fn report(
            err: Self::ExternalError,
            level: log::Level,
            report_class: String,
        ) -> ErrorHandling<Self::ExternalError> {
            ErrorHandling {
                err,
                reporting: ErrorReporting::Report {
                    level,
                    report_class,
                },
            }
        }

        // Convert, report and log the error in a way suitable for "unexpected" errors.
        // (With more generics we might be able to abstract away the creation of `err`,
        // but that will have a significant complexity cost for only marginal value)
        fn unexpected(
            err: Self::ExternalError,
            report_class: Option<&str>,
        ) -> ErrorHandling<Self::ExternalError> {
            Self::report(
                err,
                log::Level::Error,
                report_class.unwrap_or("unexpected").to_string(),
            )
        }
    }

    // Handle the specified "internal" error, taking any logging or error
    // reporting actions and converting the error to the public error.
    // Called by our `handle_error` macro.
    pub fn convert_log_report_error<IE, EE>(e: IE) -> EE
    where
        IE: GetErrorHandling<ExternalError = EE> + std::error::Error,
        EE: std::error::Error,
    {
        let handling = e.get_error_handling();
        match handling.reporting {
            ErrorReporting::Nothing => {}
            ErrorReporting::Log { level } => {
                log::log!(level, "{}", e.to_string());
            }
            ErrorReporting::Report {
                report_class,
                level,
            } => {
                log::log!(level, "{}", e.to_string());
                // notify the error reporter.
                error_support::report_error(report_class, format!("{:?}", e));
            }
        }
        handling.err
    }

    /// Function wrapper macro to convert from our internal errors to external errors
    /// and optionally log and report the error.
    macro_rules! handle_error {
        { $($tt:tt)* } => {
            let body = || {
                $($tt)*
            };
            let result: Result<_> = body();
            match result {
                Ok(r) => Ok(r),
                Err(e) => Err(error_crate::convert_log_report_error(e))
            }
        }
    }
}

// --- logins stuff.

// Functions which are part of the public API should use this Result.
pub type ApiResult<T> = std::result::Result<T, LoginsStorageError>;

// Errors we return via the public interface.
//
// Named `LoginsStorageError` for backwards compatibility reasons, although
// this name shouldn't need to be used anywhere other than this file and the .udl
//
// Note that there is no `Into` between public and internal errors, but
// instead the `ErrorHandling` mechanisms are used to explicitly convert
// when necessary.
//
// XXX - not clear that these actually need to use `thiserror`? Certainly
// not necessary to use `#[from]` though.
#[derive(Debug, thiserror::Error)]
pub enum LoginsStorageError {
    #[error("Invalid login: {0}")]
    InvalidRecord(String),

    #[error("No record with guid exists (when one was required): {0:?}")]
    NoSuchRecord(String),

    #[error("Encryption key is in the correct format, but is not the correct key.")]
    IncorrectKey,

    #[error("{0}")]
    Interrupted(String),

    #[error("Unexpected Error: {0}")]
    UnexpectedLoginsStorageError(String),
}

/// Logins error type
/// These are "internal" errors used by the implementation. This error type
/// is never returned to the consumer.
#[derive(Debug, thiserror::Error)]
pub enum LoginsError {
    #[error("Invalid login: {0}")]
    InvalidLogin(#[from] InvalidLogin),

    #[error("The `sync_status` column in DB has an illegal value: {0}")]
    BadSyncStatus(u8),

    #[error("No record with guid exists (when one was required): {0:?}")]
    NoSuchRecord(String),

    // Fennec import only works on empty logins tables.
    #[error("The logins tables are not empty")]
    NonEmptyTable,

    #[error("local encryption key not set")]
    EncryptionKeyMissing,

    #[error("Error synchronizing: {0}")]
    SyncAdapterError(#[from] sync15::Error),

    #[error("Error parsing JSON data: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Error executing SQL: {0}")]
    SqlError(#[from] rusqlite::Error),

    #[error("Error parsing URL: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Invalid path: {0:?}")]
    InvalidPath(OsString),

    #[error("Invalid database file: {0}")]
    InvalidDatabaseFile(String),

    #[error("Invalid encryption key")]
    InvalidKey,

    #[error("Crypto Error: {0}")]
    CryptoError(#[from] jwcrypto::JwCryptoError),

    #[error("{0}")]
    Interrupted(#[from] interrupt_support::Interrupted),

    #[error("IOError: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Migration Error: {0}")]
    MigrationError(String),
}

/// Error::InvalidLogin subtypes
#[derive(Debug, thiserror::Error)]
pub enum InvalidLogin {
    // EmptyOrigin error occurs when the login's origin field is empty.
    #[error("Origin is empty")]
    EmptyOrigin,
    #[error("Password is empty")]
    EmptyPassword,
    #[error("Login already exists")]
    DuplicateLogin,
    #[error("Both `formActionOrigin` and `httpRealm` are present")]
    BothTargets,
    #[error("Neither `formActionOrigin` or `httpRealm` are present")]
    NoTarget,
    #[error("Login has illegal field: {field_info}")]
    IllegalFieldValue { field_info: String },
}

// Define how our internal errors are handled and converted to external errors.
use error_crate::{ErrorHandling, GetErrorHandling};

impl GetErrorHandling for LoginsError {
    type ExternalError = LoginsStorageError;

    // Return how to handle our internal errors
    fn get_error_handling(&self) -> ErrorHandling<Self::ExternalError> {
        // WARNING: The details inside the `LoginsStorageError` we return should not
        // contain any personally identifying information.
        // However, because many of the string details come from the underlying
        // internal error, we operate on a best-effort basis, since we can't be
        // completely sure that our dependencies don't leak PII in their error
        // strings.  For example, `rusqlite::Error` could include data from a
        // user's database in their errors, which would then cause it to appear
        // in our `LoginsStorageError::Unexpected` structs, log messages, etc.
        // But because we've never seen that in practice we are comfortable
        // forwarding that error message into ours without attempting to sanitize.
        match self {
            Self::InvalidLogin(why) => {
                Self::passthrough(LoginsStorageError::InvalidRecord(why.to_string()))
            }
            // Our internal "no such record" error is converted to our public "no such record" error, with no logging and no error reporting.
            Self::NoSuchRecord(guid) => {
                Self::passthrough(LoginsStorageError::NoSuchRecord(guid.to_string()))
            }
            // NonEmptyTable error is just a sanity check to ensure we aren't asked to migrate into an
            // existing DB - consumers should never actually do this, and will never expect to handle this as a specific
            // error - so it gets reported to the error reporter and converted to an "internal" error.
            Self::NonEmptyTable => Self::unexpected(
                LoginsStorageError::UnexpectedLoginsStorageError(
                    "must be an empty DB to migrate".to_string(),
                ),
                Some("migration"),
            ),
            Self::CryptoError(_) => Self::log(LoginsStorageError::IncorrectKey, log::Level::Warn),
            Self::Interrupted(_) => {
                Self::passthrough(LoginsStorageError::Interrupted(self.to_string()))
            }
            // This list is partial - not clear if a best-practice should be to ask that every
            // internal error is listed here (and remove this default branch) to ensure every error
            // is considered, or whether this default is fine for obscure errors?
            // But it's fine for now because errors were always converted with a default
            // branch to "unexpected"
            _ => Self::unexpected(
                LoginsStorageError::UnexpectedLoginsStorageError(self.to_string()),
                None,
            ),
        }
    }
}
