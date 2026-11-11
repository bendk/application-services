/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! State machine helpers: error wrapper, target-state extension trait, and
//! `AccountWrapper`.

use crate::{
    internal::FirefoxAccount, DeviceCapability, DeviceType, Error, FxaState, LocalDevice, Result,
};

/// Error returned from `transition()`.
///
/// Causes are boxed because `Error` is ~120 bytes; keeping the variants small
/// avoids inflating every transition's `Result`.
#[derive(Debug)]
pub enum StateMachineErr {
    /// Unhandled error.
    ///
    /// If this propagates up to `process_event`, then the state machine will keep the initial
    /// state.
    Unhandled { cause: Error },
    /// Handled error with an explicit state to transition to
    Handled { cause: Error, new_state: FxaState },
}

pub type StateMachineResult<T> = std::result::Result<T, StateMachineErr>;

/// Used to attach methods to `Result<T, StateMachineErr>`
pub trait ResultExt<T> {
    fn handle_err(self, new_state: FxaState) -> Self;
}

impl<T> ResultExt<T> for StateMachineResult<T> {
    fn handle_err(self, new_state: FxaState) -> Self {
        match self {
            Err(StateMachineErr::Unhandled { cause }) => {
                Err(StateMachineErr::Handled { cause, new_state })
            }
            _ => self,
        }
    }
}

impl From<Error> for StateMachineErr {
    fn from(cause: Error) -> Self {
        Self::Unhandled { cause }
    }
}

/// Retry policy applied by [`AccountWrapper`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Number of times to retry a call after a transient network error.
    pub network_retries: u8,
}

impl RetryPolicy {
    pub const DEFAULT: RetryPolicy = RetryPolicy { network_retries: 3 };
}

/// Wraps [`FirefoxAccount`] and adds retry and authorization checking logic
///
/// Network requests will be retried accounting to the RetryPolicy.
pub struct AccountWrapper<'a> {
    inner: &'a mut FirefoxAccount,
    policy: RetryPolicy,
}

impl<'a> AccountWrapper<'a> {
    pub fn new(inner: &'a mut FirefoxAccount) -> Self {
        Self {
            inner,
            policy: RetryPolicy::DEFAULT,
        }
    }

    pub fn complete_oauth_flow(&mut self, code: &str, state: &str) -> StateMachineResult<()> {
        self.wrap_operation(|a| a.complete_oauth_flow(code, state))
    }

    pub fn handle_web_channel_password_change(
        &mut self,
        json_payload: &str,
    ) -> StateMachineResult<()> {
        self.wrap_operation(|a| a.handle_web_channel_password_change(json_payload))
    }

    /// Cancels any existing OAuth flow before starting a new one.
    pub fn begin_oauth_flow(
        &mut self,
        service: &str,
        scopes: &[&str],
        entrypoint: &str,
    ) -> StateMachineResult<String> {
        self.inner.cancel_existing_oauth_flows();
        self.wrap_operation(|a| a.begin_oauth_flow(service, scopes, entrypoint))
    }

    /// Cancels any existing OAuth flow before starting a new one.
    pub fn begin_pairing_flow(
        &mut self,
        pairing_url: &str,
        service: &str,
        scopes: &[&str],
        entrypoint: &str,
    ) -> StateMachineResult<String> {
        self.inner.cancel_existing_oauth_flows();
        self.wrap_operation(|a| a.begin_pairing_flow(pairing_url, service, scopes, entrypoint))
    }

    /// Auth errors land the FSM at `Disconnected`; no operation-level recovery.
    pub fn initialize_device(
        &mut self,
        name: &str,
        device_type: DeviceType,
        capabilities: &[DeviceCapability],
    ) -> StateMachineResult<LocalDevice> {
        self.wrap_operation(|a| a.initialize_device(name, device_type, capabilities))
    }

    /// Finish initializing a connected account
    pub fn finish_initialize(
        &mut self,
        capabilities: &[DeviceCapability],
    ) -> StateMachineResult<()> {
        self.wrap_operation(|a| {
            a.ensure_capabilities(capabilities)?;
            Ok(())
        })
    }

    /// Check if the refresh token for this account is still valid
    pub fn check_authorization_status(&mut self) -> AuthStatus {
        self._check_authorization_status(&mut NetworkErrorRetrier::new(self.policy))
    }

    fn _check_authorization_status(&mut self, retrier: &mut NetworkErrorRetrier) -> AuthStatus {
        self.inner.set_auth_recheck_timer_to_now();
        match retrier.with_retry(|| self.inner.check_authorization_status()) {
            Ok(v) => {
                if v.active {
                    AuthStatus::Valid
                } else {
                    AuthStatus::Invalid
                }
            }
            // Missing a refresh/session token means this check failed
            // https://bugzilla.mozilla.org/show_bug.cgi?id=2084260
            Err(Error::NoRefreshToken) => AuthStatus::Invalid,
            // Too many auth checks in a short time, return RefreshTokenInvalid to prevent infinite loops
            // https://bugzilla.mozilla.org/show_bug.cgi?id=2084267
            Err(Error::AuthCircuitBreakerError) => AuthStatus::Invalid,
            Err(_) => AuthStatus::Unknown,
        }
    }

    pub fn get_profile(&mut self) -> StateMachineResult<()> {
        self.wrap_operation(|a| {
            a.get_profile(true)?;
            Ok(())
        })
    }

    pub fn get_initial_state(&mut self) -> FxaState {
        self.inner.get_initial_state()
    }

    pub fn disconnect(&mut self) {
        self.inner.disconnect()
    }

    /// Panics if called before the driver has processed `Initialize`.
    pub fn device_config(&self) -> &crate::DeviceConfig {
        self.inner
            .device_config
            .as_ref()
            .expect("device_config must be set before transition runs (Initialize event seeds it)")
    }

    /// Wrap an operation on the underlying `FirefoxAccount` with retry and auth-checking logic.
    fn wrap_operation<T>(
        &mut self,
        mut op: impl FnMut(&mut FirefoxAccount) -> Result<T>,
    ) -> StateMachineResult<T> {
        let mut retrier = NetworkErrorRetrier::new(self.policy);
        retrier
            .with_retry(|| op(&mut self.inner))
            .or_else(|e| match e {
                Error::RemoteError { code: 401, .. } => {
                    // 401 error: clear the access token cache and check the authorization status.
                    self.inner.clear_access_token_cache();
                    match self._check_authorization_status(&mut retrier) {
                        AuthStatus::Valid => {
                            // Auth check succeeded, try the operation again.  Maybe we just needed
                            // to fetch a new access token.
                            Ok(retrier.with_retry(|| op(&mut self.inner))?)
                        }
                        AuthStatus::Invalid | AuthStatus::Unknown => {
                            Err(StateMachineErr::Unhandled { cause: e })
                        }
                    }
                }
                e => Err(StateMachineErr::Unhandled { cause: e }),
            })
    }

    /// Checks if enough time has passed since the last auth attempt that we should try checking the
    /// auth again.
    pub fn should_recheck_auth(&self) -> bool {
        self.inner.should_recheck_auth()
    }
}

pub enum AuthStatus {
    Valid,
    Invalid,
    /// The request failed, we don't know if the refresh token is good or bad
    Unknown,
}

struct NetworkErrorRetrier {
    network_retries: u8,
    policy: RetryPolicy,
}

impl NetworkErrorRetrier {
    fn new(policy: RetryPolicy) -> Self {
        Self {
            network_retries: 0,
            policy,
        }
    }

    fn with_retry<T>(&mut self, mut op: impl FnMut() -> Result<T>) -> Result<T> {
        loop {
            match op() {
                Ok(v) => return Ok(v),
                Err(Error::RequestError(_))
                    if self.network_retries < self.policy.network_retries =>
                {
                    self.network_retries += 1;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viaduct::ViaductError;

    fn network_error() -> Error {
        Error::RequestError(ViaductError::NetworkError("test".into()))
    }

    fn auth_error() -> Error {
        Error::NoRefreshToken
    }

    // The retry tests construct a real `FirefoxAccount` because `AccountWrapper`
    // requires one structurally; the test closures don't actually touch it.

    fn mock_account() -> FirefoxAccount {
        use crate::internal::config::Config;
        FirefoxAccount::with_config(Config::new_with_mock_well_known_fxa_client_configuration(
            "https://mock-fxa.example.com",
            "12345678",
            "https://foo.bar",
        ))
    }

    #[test]
    fn with_retry_succeeds_first_try() {
        let mut retrier = NetworkErrorRetrier::new(RetryPolicy { network_retries: 3 });
        let mut calls = 0;
        let result = retrier.with_retry(|| {
            calls += 1;
            Ok::<_, Error>(42)
        });
        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls, 1);
    }

    #[test]
    fn with_retry_retries_network_errors_then_succeeds() {
        let mut retrier = NetworkErrorRetrier::new(RetryPolicy { network_retries: 3 });
        let mut calls = 0;
        let result = retrier.with_retry(|| {
            calls += 1;
            if calls <= 2 {
                Err(network_error())
            } else {
                Ok(7)
            }
        });
        assert_eq!(result.unwrap(), 7);
        assert_eq!(calls, 3);
    }

    #[test]
    fn with_retry_gives_up_after_network_retry_limit() {
        let mut retrier = NetworkErrorRetrier::new(RetryPolicy { network_retries: 3 });
        let mut calls = 0;
        let result: Result<i32> = retrier.with_retry(|| {
            calls += 1;
            Err(network_error())
        });
        assert!(result.is_err());
        assert_eq!(calls, 4); // 1 attempt + 3 retries
    }

    #[test]
    fn with_retry_does_not_retry_auth_errors() {
        let mut retrier = NetworkErrorRetrier::new(RetryPolicy { network_retries: 3 });
        let mut calls = 0;
        let result: Result<i32> = retrier.with_retry(|| {
            calls += 1;
            Err(auth_error())
        });
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }

    // The auth-recovery path itself can't be exercised here without a real
    // backend (it calls `check_authorization_status` against the mock); these
    // tests cover the paths that don't trigger recovery.

    #[test]
    fn with_auth_recovery_succeeds_first_try() {
        nss_as::ensure_initialized();
        let mut account = mock_account();
        let mut wrapper = AccountWrapper::new(&mut account);
        let mut calls = 0;
        let result = wrapper.wrap_operation(|_| {
            calls += 1;
            Ok::<_, Error>("ok")
        });
        assert_eq!(result.unwrap(), "ok");
        assert_eq!(calls, 1);
    }

    #[test]
    fn with_auth_recovery_retries_network_errors() {
        nss_as::ensure_initialized();
        let mut account = mock_account();
        let mut wrapper = AccountWrapper::new(&mut account);
        let mut calls = 0;
        let result = wrapper.wrap_operation(|_| {
            calls += 1;
            if calls <= 1 {
                Err(network_error())
            } else {
                Ok("ok")
            }
        });
        assert_eq!(result.unwrap(), "ok");
        assert_eq!(calls, 2);
    }
}
