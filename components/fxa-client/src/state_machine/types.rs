/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::fmt;

use crate::FxaRustAuthState;

/// Fxa state
///
/// These are the states of [crate::FxaStateMachine] that consumers observe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FxaState {
    /// The state machine needs to be initialized via [Event::Initialize].
    Uninitialized,
    /// User has not connected to FxA or has logged out
    Disconnected,
    /// User is currently performing an OAuth flow
    Authenticating { oauth_url: String },
    /// User is currently connected to FxA
    Connected,
    /// User was connected to FxA, but we observed issues with the auth tokens.
    /// The user needs to reauthenticate before the account can be used.
    AuthIssues,
}

/// Fxa event
///
/// These are the events that consumers send to [crate::FxaStateMachine::process_event]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FxaEvent {
    /// Initialize the state machine.  This must be the first event sent.
    Initialize,
    /// Begin an oauth flow
    ///
    /// If successful, the state machine will transition the [FxaState::Authenticating].  The next
    /// step is to navigate the user to the `oauth_url` and let them sign and authorize the client.
    BeginOAuthFlow {
        scopes: Vec<String>,
        entrypoint: String,
    },
    /// Begin an oauth flow using a URL from a pairing code
    ///
    /// If successful, the state machine will transition the [FxaState::Authenticating].  The next
    /// step is to navigate the user to the `oauth_url` and let them sign and authorize the client.
    BeginPairingFlow {
        pairing_url: String,
        scopes: Vec<String>,
        entrypoint: String,
    },
    /// Complete an OAuth flow.
    ///
    /// Send this event after the user has navigated through the OAuth flow and has reached the
    /// redirect URI.  Extract `code` and `state` from the query parameters.  If successful the
    /// state machine will transition to [FxaState::Connected].
    CompleteOAuthFlow { code: String, state: String },
    /// Cancel an OAuth flow.
    ///
    /// Use this to cancel an in-progress OAuth, returning to [FxaState::Disconnected] so the
    /// process can begin again.
    CancelOAuthFlow,
    /// Check the authorization status for a connected account.
    ///
    /// Send this when issues are detected with the auth tokens for a connected account.  It will
    /// double check for authentication issues with the account.  If it detects them, the state
    /// machine will transition to [FxaState::AuthIssues].  From there you can start an OAuth flow
    /// again to re-connect the user.
    CheckAuthorizationStatus,
    /// Disconnect the user
    ///
    /// Send this when the user is asking to be logged out.  The state machine will transition to
    /// [FxaState::Disconnected].
    Disconnect,
}

/// Internal [crate::FxaStateMachine] states
///
/// These states represent the method calls made when transitioning from one [FxaState] to another.
/// This enum has the same variants as [FxaState], but each variant stores an enum that represents
/// the internal states for that public state.
///
/// This forms a kind of hierarchical state machine.  For example, you can picture
/// FxaState::Disconnected as containing each of the `DisconnectedState` variants as child states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InternalState {
    Uninitialized(UninitializedState),
    Disconnected(DisconnectedState),
    Authenticating(AuthenticatingState),
    Connected(ConnectedState),
    AuthIssues(AuthIssuesState),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UninitializedState {
    GetAuthState,
    EnsureDeviceCapabilities,
    CheckAuthorizationStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisconnectedState {
    BeginOAuthFlow {
        scopes: Vec<String>,
        entrypoint: String,
    },
    BeginPairingFlow {
        pairing_url: String,
        scopes: Vec<String>,
        entrypoint: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthenticatingState {
    CompleteOAuthFlow { code: String, state: String },
    InitializeDevice,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectedState {
    CheckAuthorizationStatus,
    Disconnect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthIssuesState {
    BeginOAuthFlow {
        scopes: Vec<String>,
        entrypoint: String,
    },
}

/// Internal [crate::FxaStateMachine] events
///
/// These represent the results of the method calls for each internal state.
#[derive(Debug)]
pub enum InternalEvent {
    GetAuthStateSuccess { auth_state: FxaRustAuthState },
    BeginOAuthFlowSuccess { oauth_url: String },
    BeginPairingFlowSuccess { oauth_url: String },
    CompleteOAuthFlowSuccess,
    InitializeDeviceSuccess,
    EnsureDeviceCapabilitiesSuccess,
    CheckAuthorizationStatusSuccess { active: bool },
    DisconnectSuccess,
    CallError { kind: CallErrorKind },
}

#[derive(Debug)]
pub enum CallErrorKind {
    Auth,
    Other,
}

// Display impl for FxaState
//
// This only returns the variant name to avoid leaking any PII
impl fmt::Display for FxaState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            Self::Uninitialized => "Uninitialized",
            Self::Disconnected => "Disconnected",
            Self::Authenticating { .. } => "Authenticating",
            Self::Connected => "Connected",
            Self::AuthIssues => "AuthIssues",
        };
        write!(f, "{name}")
    }
}

// ==================== Display impls ====================
//
// These only returns the variant name to avoid leaking any PII

impl fmt::Display for UninitializedState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            UninitializedState::GetAuthState => "Uninitialized(GetAuthState)",
            UninitializedState::EnsureDeviceCapabilities => {
                "Uninitialized(EnsureDeviceCapabilities)"
            }
            UninitializedState::CheckAuthorizationStatus => {
                "Uninitialized(CheckAuthorizationStatu)"
            }
        };
        write!(f, "{name}")
    }
}

impl fmt::Display for DisconnectedState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            DisconnectedState::BeginOAuthFlow { .. } => "Disconnected(BeginOAuthFlow)",
            DisconnectedState::BeginPairingFlow { .. } => "Disconnected(BeginPairingFlow)",
        };
        write!(f, "{name}")
    }
}

impl fmt::Display for AuthenticatingState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            AuthenticatingState::CompleteOAuthFlow { .. } => "Authenticating(CompleteOAuthFlow)",
            AuthenticatingState::InitializeDevice => "Authenticating(InitializeDevice)",
        };
        write!(f, "{name}")
    }
}

impl fmt::Display for ConnectedState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            ConnectedState::CheckAuthorizationStatus => "Connected(CheckAuthorizationStatus)",
            ConnectedState::Disconnect => "Connected(Disconnected)",
        };
        write!(f, "{name}")
    }
}

impl fmt::Display for AuthIssuesState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            AuthIssuesState::BeginOAuthFlow { .. } => "AuthIssues::BeginOAuthFlow",
        };
        write!(f, "{name}")
    }
}

impl fmt::Display for InternalEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            Self::GetAuthStateSuccess { .. } => "Event::GetAuthStateSuccess",
            Self::BeginOAuthFlowSuccess { .. } => "Event::BeginOAuthFlowSuccess",
            Self::BeginPairingFlowSuccess { .. } => "Event::BeginPairingFlowSuccess",
            Self::CompleteOAuthFlowSuccess { .. } => "Event::CompleteOAuthFlowSuccess",
            Self::InitializeDeviceSuccess { .. } => "Event::InitializeDeviceSuccess",
            Self::EnsureDeviceCapabilitiesSuccess { .. } => {
                "Event::EnsureDeviceCapabilitiesSuccess"
            }
            Self::CheckAuthorizationStatusSuccess { .. } => {
                "Event::CheckAuthorizationStatusSuccess"
            }
            Self::DisconnectSuccess { .. } => "Event::DisconnectSuccess",
            Self::CallError { .. } => "Event::CallError",
        };
        write!(f, "{name}")
    }
}

impl fmt::Display for FxaEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            Self::Initialize => "FxaEvent::Initialize",
            Self::BeginOAuthFlow { .. } => "FxaEvent::BeginOAuthFlow",
            Self::BeginPairingFlow { .. } => "FxaEvent::BeginPairingFlow",
            Self::CompleteOAuthFlow { .. } => "FxaEvent::CompleteOAuthFlow",
            Self::CancelOAuthFlow => "FxaEvent::CancelOAuthFlow",
            Self::CheckAuthorizationStatus => "FxaEvent::CheckAuthorizationStatus",
            Self::Disconnect => "FxaEvent::Disconnect",
        };
        write!(f, "{name}")
    }
}

// ==================== Conversions ====================

impl From<UninitializedState> for InternalState {
    fn from(state: UninitializedState) -> Self {
        Self::Uninitialized(state)
    }
}

impl From<DisconnectedState> for InternalState {
    fn from(state: DisconnectedState) -> Self {
        Self::Disconnected(state)
    }
}

impl From<AuthenticatingState> for InternalState {
    fn from(state: AuthenticatingState) -> Self {
        Self::Authenticating(state)
    }
}

impl From<ConnectedState> for InternalState {
    fn from(state: ConnectedState) -> Self {
        Self::Connected(state)
    }
}

impl From<AuthIssuesState> for InternalState {
    fn from(state: AuthIssuesState) -> Self {
        Self::AuthIssues(state)
    }
}
