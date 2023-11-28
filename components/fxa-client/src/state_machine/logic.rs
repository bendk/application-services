/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use super::types::*;
use crate::FxaRustAuthState;
use error_support::report_error;
use std::fmt::Display;

// Let's save some typing
use FxaEvent::*;
use FxaState::*;
use InternalEvent::*;
use InternalStateTransition::*;

/// Trait for internal state logic
///
/// This is what we use to implement `process_event`.
pub trait InternalStateLogic: Sized {
    /// Initial state to start handling an public event
    fn start_transition(public_state: &FxaState, event: &FxaEvent)
        -> InternalStateTransition<Self>;

    /// Transition from an internal state based on the result of a method call.
    fn transition(self, event: InternalEvent) -> InternalStateTransition<Self>;
}

/// Internal state transition
///
/// This represents what we should do after calling the `FirefoxAccount` method that corresponds to
/// an internal state.
#[derive(Debug, PartialEq, Eq)]
pub enum InternalStateTransition<T = InternalState> {
    /// Process a new internal state
    Process(T),
    /// Complete the process by transitioning to a new public state
    Complete(FxaState),
    /// Complete the process by transitioning back to the last public state
    Cancel,
}

impl<T: Into<InternalState>> InternalStateTransition<T> {
    // I wish this could be a blanket `From` impl, but that creates a conflicting implementation
    // since `InternalState: From<InternalState>`
    fn convert(self) -> InternalStateTransition<InternalState> {
        match self {
            InternalStateTransition::Process(state) => {
                InternalStateTransition::Process(state.into())
            }
            InternalStateTransition::Complete(fxa_state) => {
                InternalStateTransition::Complete(fxa_state)
            }
            InternalStateTransition::Cancel => InternalStateTransition::Cancel,
        }
    }
}

impl InternalStateLogic for InternalState {
    fn start_transition(
        public_state: &FxaState,
        event: &FxaEvent,
    ) -> InternalStateTransition<Self> {
        match public_state {
            Uninitialized => UninitializedState::start_transition(public_state, event).convert(),
            Disconnected => DisconnectedState::start_transition(public_state, event).convert(),
            Authenticating { .. } => {
                AuthenticatingState::start_transition(public_state, event).convert()
            }
            Connected => ConnectedState::start_transition(public_state, event).convert(),
            AuthIssues => AuthIssuesState::start_transition(public_state, event).convert(),
        }
    }

    fn transition(self, event: InternalEvent) -> InternalStateTransition<Self> {
        match self {
            Self::Uninitialized(inner) => inner.transition(event).convert(),
            Self::Disconnected(inner) => inner.transition(event).convert(),
            Self::Authenticating(inner) => inner.transition(event).convert(),
            Self::Connected(inner) => inner.transition(event).convert(),
            Self::AuthIssues(inner) => inner.transition(event).convert(),
        }
    }
}

impl InternalStateLogic for UninitializedState {
    fn start_transition(
        public_state: &FxaState,
        event: &FxaEvent,
    ) -> InternalStateTransition<Self> {
        match event {
            Initialize => Process(Self::GetAuthState),
            _ => invalid_transition(public_state, &event),
        }
    }

    fn transition(self, event: InternalEvent) -> InternalStateTransition<Self> {
        match (self, event) {
            (Self::GetAuthState, GetAuthStateSuccess { auth_state }) => match auth_state {
                FxaRustAuthState::Disconnected => Complete(Disconnected),
                FxaRustAuthState::AuthIssues => {
                    // FIXME: We should move to `State::AuthIssues` here, but we don't in order to
                    // match the current firefox-android behavior
                    // See https://bugzilla.mozilla.org/show_bug.cgi?id=1794212
                    Complete(Connected)
                }
                FxaRustAuthState::Connected => Process(Self::EnsureDeviceCapabilities),
            },
            (Self::EnsureDeviceCapabilities, EnsureDeviceCapabilitiesSuccess) => {
                Complete(Connected)
            }
            (Self::EnsureDeviceCapabilities, CallError { kind }) => match kind {
                CallErrorKind::Auth => Process(Self::CheckAuthorizationStatus),
                CallErrorKind::Other => Complete(Disconnected),
            },
            // FIXME: we should re-run `ensure_capabilities` in this case, but we don't in order to
            // match the current firefox-android behavior.
            // See https://bugzilla.mozilla.org/show_bug.cgi?id=1868418
            (Self::CheckAuthorizationStatus, CheckAuthorizationStatusSuccess { active: true }) => {
                Complete(Connected)
            }
            (Self::CheckAuthorizationStatus, CheckAuthorizationStatusSuccess { active: false })
            | (Self::CheckAuthorizationStatus, CallError { .. }) => Complete(AuthIssues),
            (state, event) => return invalid_transition(&state, &event),
        }
    }
}

impl InternalStateLogic for DisconnectedState {
    fn start_transition(
        public_state: &FxaState,
        event: &FxaEvent,
    ) -> InternalStateTransition<Self> {
        match event {
            BeginOAuthFlow { scopes, entrypoint } => Process(Self::BeginOAuthFlow {
                scopes: scopes.clone(),
                entrypoint: entrypoint.clone(),
            }),
            BeginPairingFlow {
                pairing_url,
                scopes,
                entrypoint,
            } => Process(Self::BeginPairingFlow {
                pairing_url: pairing_url.clone(),
                scopes: scopes.clone(),
                entrypoint: entrypoint.clone(),
            }),
            event => invalid_transition(public_state, &event),
        }
    }

    fn transition(self, event: InternalEvent) -> InternalStateTransition<Self> {
        match (self, event) {
            (Self::BeginOAuthFlow { .. }, BeginOAuthFlowSuccess { oauth_url }) => {
                Complete(Authenticating { oauth_url })
            }
            (Self::BeginPairingFlow { .. }, BeginPairingFlowSuccess { oauth_url }) => {
                Complete(Authenticating { oauth_url })
            }
            (Self::BeginOAuthFlow { .. }, CallError { .. }) => Cancel,
            (Self::BeginPairingFlow { .. }, CallError { .. }) => Cancel,
            (state, event) => return invalid_transition(&state, &event),
        }
    }
}

impl InternalStateLogic for AuthenticatingState {
    fn start_transition(
        public_state: &FxaState,
        event: &FxaEvent,
    ) -> InternalStateTransition<Self> {
        match event {
            CompleteOAuthFlow { code, state } => Process(Self::CompleteOAuthFlow {
                code: code.clone(),
                state: state.clone(),
            }),
            CancelOAuthFlow => Complete(Disconnected),
            event => invalid_transition(public_state, &event),
        }
    }

    fn transition(self, event: InternalEvent) -> InternalStateTransition<Self> {
        match (self, event) {
            (Self::CompleteOAuthFlow { .. }, CompleteOAuthFlowSuccess) => {
                Process(Self::InitializeDevice)
            }
            (Self::CompleteOAuthFlow { .. }, CallError { .. }) => Cancel,
            (Self::InitializeDevice, InitializeDeviceSuccess) => Complete(Connected),
            (Self::InitializeDevice, CallError { .. }) => Cancel,
            (state, event) => invalid_transition(&state, &event),
        }
    }
}

impl InternalStateLogic for ConnectedState {
    fn start_transition(
        public_state: &FxaState,
        event: &FxaEvent,
    ) -> InternalStateTransition<Self> {
        match event {
            Disconnect => Process(Self::Disconnect),
            CheckAuthorizationStatus => Process(Self::CheckAuthorizationStatus),
            event => invalid_transition(public_state, &event),
        }
    }

    fn transition(self, event: InternalEvent) -> InternalStateTransition<Self> {
        match (self, event) {
            (Self::Disconnect, DisconnectSuccess) => Complete(Disconnected),
            (Self::Disconnect, CallError { .. }) => {
                // disconnect() is currently infallible, but let's handle errors anyway in case we
                // refactor it in the future.
                report_error!("fxa-state-machine-error", "saw CallError after Disconnect");
                Complete(Disconnected)
            }
            (Self::CheckAuthorizationStatus, CheckAuthorizationStatusSuccess { active }) => {
                if active {
                    Complete(Connected)
                } else {
                    Complete(Disconnected)
                }
            }
            (Self::CheckAuthorizationStatus, CallError { .. }) => Complete(Disconnected),
            (state, event) => invalid_transition(&state, &event),
        }
    }
}

impl InternalStateLogic for AuthIssuesState {
    fn start_transition(
        public_state: &FxaState,
        event: &FxaEvent,
    ) -> InternalStateTransition<Self> {
        match event {
            BeginOAuthFlow { scopes, entrypoint } => Process(Self::BeginOAuthFlow {
                scopes: scopes.clone(),
                entrypoint: entrypoint.clone(),
            }),
            // Note: Pairing flow is intended for connecting new devices only and isn't valid for
            // re-authentication from the `auth-issues` state.
            event => invalid_transition(public_state, &event),
        }
    }

    fn transition(self, event: InternalEvent) -> InternalStateTransition<Self> {
        match (self, event) {
            (Self::BeginOAuthFlow { .. }, BeginOAuthFlowSuccess { oauth_url }) => {
                Complete(Authenticating { oauth_url })
            }
            (Self::BeginOAuthFlow { .. }, CallError { .. }) => Cancel,
            (state, event) => invalid_transition(&state, &event),
        }
    }
}

fn invalid_transition<T>(state: &impl Display, event: &impl Display) -> InternalStateTransition<T> {
    report_error!(
        "fxa-state-machine-error",
        "Invalid transition: {state} -> {event}"
    );
    InternalStateTransition::Cancel
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Clone)]
    struct StateMachineTester<T> {
        state: T,
    }

    impl<T: InternalStateLogic + std::fmt::Debug + Clone + Eq> StateMachineTester<T> {
        fn new(public_state: FxaState, event: FxaEvent) -> Self {
            match T::start_transition(&public_state, &event) {
                Process(state) => Self { state },
                transition => panic!("start_transition returned {transition:?}"),
            }
        }

        /// Transition to a new state based on an event
        ///
        /// Only use this when the event should result in a transition to another internal event.
        /// Any other transition will panic.
        fn transition(&mut self, event: InternalEvent) {
            match T::transition(self.state.clone(), event) {
                Process(state) => self.state = state,
                transition => panic!("transition returned {transition:?}"),
            }
        }

        /// Get what transition would be returned for an event.
        ///
        /// Good for testing the final transition to a public state
        fn get_transition(&self, event: InternalEvent) -> InternalStateTransition<T> {
            T::transition(self.state.clone(), event)
        }

        /// Get what transition would be returned for error events
        ///
        /// Like get_transition, but it passes `transition` all possible ErrorKind values.
        ///
        /// Panics if all ErrorKind values don't result in the same transition.
        fn get_transition_for_error(&self) -> InternalStateTransition<T> {
            let error_kinds = [CallErrorKind::Other, CallErrorKind::Auth];
            let mut transitions: Vec<_> = error_kinds
                .into_iter()
                .map(|kind| self.get_transition(InternalEvent::CallError { kind }))
                .collect();

            let transition = transitions.pop().unwrap();
            for other in transitions {
                assert_eq!(transition, other, "Not all error kinds result in the same transition ({transition:?} vs {other:?})");
            }
            transition
        }
    }

    #[test]
    fn test_initialize() {
        let mut tester = StateMachineTester::<UninitializedState>::new(Uninitialized, Initialize);
        assert_eq!(tester.state, UninitializedState::GetAuthState);
        assert_eq!(
            tester.get_transition(GetAuthStateSuccess {
                auth_state: FxaRustAuthState::Disconnected
            }),
            Complete(Disconnected)
        );
        assert_eq!(
            tester.get_transition(GetAuthStateSuccess {
                auth_state: FxaRustAuthState::AuthIssues
            }),
            Complete(Connected)
        );

        tester.transition(GetAuthStateSuccess {
            auth_state: FxaRustAuthState::Connected,
        });
        assert_eq!(tester.state, UninitializedState::EnsureDeviceCapabilities);
        assert_eq!(
            tester.get_transition(CallError {
                kind: CallErrorKind::Other
            }),
            Complete(Disconnected),
        );
        assert_eq!(
            tester.get_transition(EnsureDeviceCapabilitiesSuccess),
            Complete(Connected),
        );

        tester.transition(CallError {
            kind: CallErrorKind::Auth,
        });
        assert_eq!(tester.state, UninitializedState::CheckAuthorizationStatus);
        assert_eq!(tester.get_transition_for_error(), Complete(AuthIssues));
        assert_eq!(
            tester.get_transition(CheckAuthorizationStatusSuccess { active: false }),
            Complete(AuthIssues)
        );
        assert_eq!(
            tester.get_transition(CheckAuthorizationStatusSuccess { active: true }),
            Complete(Connected)
        );
    }

    #[test]
    fn test_oauth_flow() {
        let tester = StateMachineTester::<DisconnectedState>::new(
            Disconnected,
            BeginOAuthFlow {
                scopes: vec!["profile".to_owned()],
                entrypoint: "test-entrypoint".to_owned(),
            },
        );
        assert_eq!(
            tester.state,
            DisconnectedState::BeginOAuthFlow {
                scopes: vec!["profile".to_owned()],
                entrypoint: "test-entrypoint".to_owned(),
            }
        );
        assert_eq!(tester.get_transition_for_error(), Cancel);
        assert_eq!(
            tester.get_transition(BeginOAuthFlowSuccess {
                oauth_url: "http://example.com/oauth-start".to_owned()
            }),
            Complete(Authenticating {
                oauth_url: "http://example.com/oauth-start".to_owned(),
            })
        );
    }

    #[test]
    fn test_pairing_flow() {
        let tester = StateMachineTester::<DisconnectedState>::new(
            Disconnected,
            BeginPairingFlow {
                pairing_url: "https://example.com/pairing-url".to_owned(),
                scopes: vec!["profile".to_owned()],
                entrypoint: "test-entrypoint".to_owned(),
            },
        );
        assert_eq!(
            tester.state,
            DisconnectedState::BeginPairingFlow {
                pairing_url: "https://example.com/pairing-url".to_owned(),
                scopes: vec!["profile".to_owned()],
                entrypoint: "test-entrypoint".to_owned(),
            }
        );
        assert_eq!(tester.get_transition_for_error(), Cancel);
        assert_eq!(
            tester.get_transition(BeginPairingFlowSuccess {
                oauth_url: "http://example.com/oauth-start".to_owned()
            }),
            Complete(Authenticating {
                oauth_url: "http://example.com/oauth-start".to_owned(),
            })
        );
    }

    #[test]
    fn test_complete_oauth_flow() {
        let mut tester = StateMachineTester::<AuthenticatingState>::new(
            Authenticating {
                oauth_url: "http://example.com/oauth-start".to_owned(),
            },
            CompleteOAuthFlow {
                code: "test-code".to_owned(),
                state: "test-state".to_owned(),
            },
        );
        assert_eq!(
            tester.state,
            AuthenticatingState::CompleteOAuthFlow {
                code: "test-code".to_owned(),
                state: "test-state".to_owned(),
            }
        );
        assert_eq!(tester.get_transition_for_error(), Cancel);

        tester.transition(CompleteOAuthFlowSuccess);
        assert_eq!(tester.state, AuthenticatingState::InitializeDevice);
        assert_eq!(tester.get_transition_for_error(), Cancel);
        assert_eq!(
            tester.get_transition(InitializeDeviceSuccess),
            Complete(Connected)
        );
    }

    #[test]
    fn test_cancel_oauth_flow() {
        assert_eq!(
            InternalState::start_transition(
                &Authenticating {
                    oauth_url: "http://example.com/oauth-start".to_owned(),
                },
                &CancelOAuthFlow,
            ),
            Complete(Disconnected)
        );
    }

    #[test]
    fn test_disconnect() {
        let tester = StateMachineTester::<ConnectedState>::new(Connected, Disconnect);
        assert_eq!(tester.state, ConnectedState::Disconnect);

        assert_eq!(tester.get_transition_for_error(), Complete(Disconnected));
        assert_eq!(
            tester.get_transition(DisconnectSuccess),
            Complete(Disconnected)
        );
    }

    #[test]
    fn test_check_authorization() {
        let tester = StateMachineTester::<ConnectedState>::new(Connected, CheckAuthorizationStatus);
        assert_eq!(tester.state, ConnectedState::CheckAuthorizationStatus);
        assert_eq!(tester.get_transition_for_error(), Complete(Disconnected));
        assert_eq!(
            tester.get_transition(CheckAuthorizationStatusSuccess { active: true }),
            Complete(Connected),
        );
        assert_eq!(
            tester.get_transition(CheckAuthorizationStatusSuccess { active: false }),
            Complete(Disconnected)
        );
    }

    #[test]
    fn test_reauthenticate() {
        let tester = StateMachineTester::<AuthIssuesState>::new(
            AuthIssues,
            BeginOAuthFlow {
                scopes: vec!["profile".to_owned()],
                entrypoint: "test-entrypoint".to_owned(),
            },
        );

        assert_eq!(
            tester.state,
            AuthIssuesState::BeginOAuthFlow {
                scopes: vec!["profile".to_owned()],
                entrypoint: "test-entrypoint".to_owned()
            }
        );
        assert_eq!(tester.get_transition_for_error(), Cancel);
        assert_eq!(
            tester.get_transition(BeginOAuthFlowSuccess {
                oauth_url: "http://example.com/oauth-start".to_owned()
            }),
            Complete(Authenticating {
                oauth_url: "http://example.com/oauth-start".to_owned(),
            })
        );
    }
}
