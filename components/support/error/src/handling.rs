/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Helpers for components to "handle" errors.

/// Describes what error reporting action should be taken.
pub enum ErrorReporting {
    /// No logging or error reporting.
    Nothing,
    /// We write a log message but don't report it.
    Log { level: log::Level },
    /// We log a message and report via our error reporter.
    Report {
        level: log::Level,
        report_class: String,
    },
}

/// Specifies how an "internal" error is converted to an "external" public error and
/// any logging or reporting that should happen.
pub struct ErrorHandling<E> {
    /// The external error that should be returned.
    pub err: E,
    /// How the error should be reported.
    pub reporting: ErrorReporting,
}

/// A trait to define how errors are converted and reported.
pub trait GetErrorHandling {
    type ExternalError;

    /// Return how to handle our internal errors
    fn get_error_handling(&self) -> ErrorHandling<Self::ExternalError>;

    // Some helpers to cut the verbosity down.
    /// Just convert the error without any special logging or error reporting.
    fn passthrough(err: Self::ExternalError) -> ErrorHandling<Self::ExternalError> {
        ErrorHandling {
            err,
            reporting: ErrorReporting::Nothing,
        }
    }

    /// Just convert and log the error without any special error reporting.
    fn log(err: Self::ExternalError, level: log::Level) -> ErrorHandling<Self::ExternalError> {
        ErrorHandling {
            err,
            reporting: ErrorReporting::Log { level },
        }
    }

    /// Convert, report and log the error.
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

    /// Convert, report and log the error in a way suitable for "unexpected" errors.
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

/// Handle the specified "internal" error, taking any logging or error
/// reporting actions and converting the error to the public error.
/// Called by our `handle_error` macro so needs to be public.
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
            // avoid unused var warning when the "reporting" feature isn't enabled.
            report_class,
            level,
        } => {
            log::log!(level, "{}", e.to_string());
            // notify the error reporter if the feature is enabled.
            // XXX - should we arrange for the `report_class` to have the
            // original crate calling this as a prefix, or will we still be
            // able to identify that?
            #[cfg(feature = "reporting")]
            crate::report_error(report_class, e.to_string());
            #[cfg(not(feature = "reporting"))]
            let _ = report_class; // avoid clippy warning when feature's not enabled.
        }
    }
    handling.err
}
