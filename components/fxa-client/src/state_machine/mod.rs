/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! FxA state machine
//!
//! This presents a high-level API for logging in, logging out, dealing with authentication token issues, etc.

use std::sync::Arc;

use error_support::{breadcrumb, convert_log_report_error, handle_error};
use parking_lot::Mutex;

use crate::{internal, ApiResult, DeviceConfig, Error, FirefoxAccount, FxaError, Result};
mod logic;
mod types;

use logic::{InternalStateLogic, InternalStateTransition};
pub use types::*;

/// Number of internal state transitions to perform before giving up and assuming the internal code
/// is stuck in an infinite loop
const MAX_INTERNAL_TRANSITIONS: usize = 20;

/// FxA state machine
///
/// This provides a high-level interface for using a [FirefoxAccount] -- login, logout, checking
/// auth status, etc.
pub struct FxaStateMachine {
    account: Arc<FirefoxAccount>,
    state: Mutex<FxaState>,
    device_config: DeviceConfig,
}

impl FxaStateMachine {
    /// Create an FxaStateMachine
    ///
    /// Note: When restoring a connected account, only `device_config.capabilities` will be used.
    /// We will use the device type and device name info that's stored on the server.
    pub fn new(account: Arc<FirefoxAccount>, device_config: DeviceConfig) -> Self {
        Self {
            account,
            state: Mutex::new(FxaState::Uninitialized),
            device_config,
        }
    }

    /// Get the current state
    pub fn state(&self) -> FxaState {
        self.state.lock().clone()
    }

    /// Process an event (login, logout, etc).
    ///
    /// On success, returns the new state.
    /// On error, the state will remain the same.
    #[handle_error(Error)]
    pub fn process_event(&self, event: FxaEvent) -> ApiResult<FxaState> {
        breadcrumb!("FxaStateMachine.process_event starting");
        let mut account = self.account.internal.lock();
        let mut current_state = self.state.lock();
        let mut count = 0;
        let mut transition = InternalState::start_transition(&current_state, &event);

        loop {
            let internal_state = match transition {
                InternalStateTransition::Process(internal_state) => internal_state,
                InternalStateTransition::Complete(public_state) => {
                    breadcrumb!("FxaStateMachine.process_event finished");
                    *current_state = public_state.clone();
                    return Ok(public_state);
                }
                InternalStateTransition::Cancel => {
                    breadcrumb!("FxaStateMachine.process_event finished");
                    if count == 0 {
                        // If `start_transition` returned `Cancel` then the application sent us an
                        // invalid event.
                        return Err(Error::InvalidStateTransition(format!(
                            "{current_state} -> {event}"
                        )));
                    } else {
                        return Ok(current_state.clone());
                    }
                }
            };
            count += 1;
            if count > MAX_INTERNAL_TRANSITIONS {
                breadcrumb!("FxaStateMachine.process_event finished");
                return Err(Error::StateMachineLogicError(
                    "infinite loop detected".to_owned(),
                ));
            }
            let internal_event = match self.process_internal_state(&mut account, &internal_state) {
                Ok(internal_event) => internal_event,
                // For errors, log/report them if the logic in error.rs says so.
                // Then process `InternalEvent::CallError`.
                Err(e) => match convert_log_report_error(e) {
                    FxaError::Authentication => InternalEvent::CallError {
                        kind: CallErrorKind::Auth,
                    },
                    _ => InternalEvent::CallError {
                        kind: CallErrorKind::Other,
                    },
                },
            };
            transition = internal_state.transition(internal_event);
        }
    }

    /// Process an internal state
    ///
    /// This means invoking the [FirefoxAccount] call that corresponds to the state and returning
    /// the [InternalEvent] that corresponds to the result.
    ///
    /// Returns the Event that's the result of the call
    pub fn process_internal_state(
        &self,
        account: &mut internal::FirefoxAccount,
        state: &InternalState,
    ) -> Result<InternalEvent> {
        // Save some typing
        use InternalEvent::*;
        use InternalState::*;

        Ok(match state {
            Uninitialized(UninitializedState::GetAuthState) => GetAuthStateSuccess {
                auth_state: account.get_auth_state(),
            },
            Uninitialized(UninitializedState::EnsureDeviceCapabilities) => {
                account.ensure_capabilities(&self.device_config.capabilities)?;
                EnsureDeviceCapabilitiesSuccess
            }
            Disconnected(DisconnectedState::BeginOAuthFlow { scopes, entrypoint })
            | AuthIssues(AuthIssuesState::BeginOAuthFlow { scopes, entrypoint }) => {
                let scopes: Vec<&str> = scopes.iter().map(String::as_str).collect();
                let oauth_url = account.begin_oauth_flow(&scopes, entrypoint)?;
                BeginOAuthFlowSuccess { oauth_url }
            }
            Disconnected(DisconnectedState::BeginPairingFlow {
                pairing_url,
                scopes,
                entrypoint,
            }) => {
                let scopes: Vec<&str> = scopes.iter().map(String::as_str).collect();
                let oauth_url = account.begin_pairing_flow(pairing_url, &scopes, entrypoint)?;
                BeginOAuthFlowSuccess { oauth_url }
            }
            Authenticating(AuthenticatingState::CompleteOAuthFlow { code, state }) => {
                account.complete_oauth_flow(code, state)?;
                CompleteOAuthFlowSuccess
            }
            Authenticating(AuthenticatingState::InitializeDevice) => {
                account.initialize_device(
                    &self.device_config.name,
                    self.device_config.device_type,
                    &self.device_config.capabilities,
                )?;
                InitializeDeviceSuccess
            }
            Connected(ConnectedState::CheckAuthorizationStatus)
            | Uninitialized(UninitializedState::CheckAuthorizationStatus) => {
                let active = account.check_authorization_status()?.active;
                CheckAuthorizationStatusSuccess { active }
            }
            Connected(ConnectedState::Disconnect) => {
                account.disconnect();
                DisconnectSuccess
            }
        })
    }
}
