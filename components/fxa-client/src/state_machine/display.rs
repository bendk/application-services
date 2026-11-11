/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Display impls for state machine types
//!
//! These are sent to Sentry, so they must not leak PII.
//! In general this means they don't output values for inner fields.

use super::{FxaEvent, FxaState};
use std::fmt;

impl fmt::Display for FxaState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Uninitialized => write!(f, "Uninitialized"),
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Authenticating { .. } => write!(f, "Authenticating"),
            Self::Connected => write!(f, "Connected"),
            Self::AuthIssues { account_deleted } => f
                .debug_struct("AuthIssues")
                .field("account_deleted", account_deleted)
                .finish(),
        }
    }
}

impl fmt::Display for FxaEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            Self::Initialize { .. } => "Initialize",
            Self::BeginOAuthFlow { .. } => "BeginOAuthFlow",
            Self::BeginPairingFlow { .. } => "BeginPairingFlow",
            Self::CompleteOAuthFlow { .. } => "CompleteOAuthFlow",
            Self::CancelOAuthFlow => "CancelOAuthFlow",
            Self::CheckAuthorizationStatus => "CheckAuthorizationStatus",
            Self::WebChannelPasswordChange { .. } => "WebChannelPwdChange",
            Self::Disconnect => "Disconnect",
            Self::CallGetProfile => "CallGetProfile",
        };
        write!(f, "{name}")
    }
}
