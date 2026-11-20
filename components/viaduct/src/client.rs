/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::{
    backend::get_backend,
    header_names::USER_AGENT,
    settings::{validate_request, GLOBAL_SETTINGS},
    Request, Response, Result, ViaductError,
};
use error_support::{report_error, ExpectedFailure};

pub struct ClientBuilder {
    settings: ClientSettings,
    error_component: Option<String>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        let mut settings = ClientSettings::default();
        settings.update_from_global_settings();
        Self {
            settings,
            error_component: None,
        }
    }
}

impl ClientBuilder {
    pub fn new(mut settings: ClientSettings) -> Self {
        settings.update_from_global_settings();
        Self {
            settings,
            error_component: None,
        }
    }

    /// Add error reporting support to the client
    ///
    /// This reports network errors via the `expected_failures` Glean metric.
    /// `component_name` must match one of the key labels for that metric.
    #[must_use]
    pub fn with_error_reporting(self, component_name: &str) -> Self {
        Self {
            error_component: Some(component_name.into()),
            ..self
        }
    }

    /// Use OHTTP with the specified channel for all requests
    #[cfg(feature = "ohttp")]
    pub fn with_ohttp_channel(mut self, channel: &str) -> Result<Self> {
        if !crate::ohttp::is_ohttp_channel_configured(channel) {
            return Err(crate::ViaductError::OhttpChannelNotConfigured(
                channel.to_string(),
            ));
        }
        self.settings.ohttp_channel = Some(channel.to_string());
        Ok(self)
    }

    pub fn finish(self) -> Client {
        Client {
            settings: self.settings,
            error_component: self.error_component,
        }
    }
}

/// HTTP Client
#[derive(Default)]
pub struct Client {
    settings: ClientSettings,
    error_component: Option<String>,
}

impl Client {
    /// Create a new client
    pub fn new(mut settings: ClientSettings) -> Self {
        settings.update_from_global_settings();
        Self {
            settings,
            error_component: None,
        }
    }

    /// Create a client that uses OHTTP with the specified channel for all requests
    #[cfg(feature = "ohttp")]
    pub fn with_ohttp_channel(
        channel: &str,
        settings: ClientSettings,
    ) -> Result<Self, crate::ViaductError> {
        if !crate::ohttp::is_ohttp_channel_configured(channel) {
            return Err(crate::ViaductError::OhttpChannelNotConfigured(
                channel.to_string(),
            ));
        }
        let mut client_settings = settings;
        client_settings.ohttp_channel = Some(channel.to_string());
        Ok(Self::new(client_settings))
    }

    fn set_user_agent(&self, request: &mut Request) -> Result<()> {
        if let Some(user_agent) = &self.settings.user_agent {
            request.headers.insert_if_missing(USER_AGENT, user_agent)?;
        }
        Ok(())
    }

    pub async fn send(&self, mut request: Request) -> Result<Response> {
        validate_request(&request)?;
        self.set_user_agent(&mut request)?;

        // Check if this client should use OHTTP for all requests
        #[cfg(feature = "ohttp")]
        if let Some(channel) = &self.settings.ohttp_channel {
            crate::debug!(
                "Client configured for OHTTP channel '{}', processing request via OHTTP",
                channel
            );
            return crate::ohttp::process_ohttp_request(request, channel, self.settings.clone())
                .await
                .inspect_err(|e| self.report_error(e));
        }

        // For non-OHTTP requests, use the normal backend
        crate::debug!("Processing request via standard backend");
        get_backend()?
            .send_request(request, self.settings.clone())
            .await
            .inspect_err(|e| self.report_error(e))
    }

    pub fn send_sync(&self, request: Request) -> Result<Response> {
        pollster::block_on(self.send(request))
    }

    fn report_error(&self, e: &ViaductError) {
        let Some(component_name) = &self.error_component else {
            return;
        };

        match e {
            ViaductError::NetworkError(_) => ExpectedFailure::Network.record(component_name),
            ViaductError::NetworkTimeoutError => {
                ExpectedFailure::NetworkTimeout.record(component_name)
            }
            _ => report_error(
                format!("{component_name}-viaduct-unexpected"),
                e.to_string(),
            ),
        }
    }
}

#[derive(Debug, uniffi::Record, Clone)]
#[repr(C)]
pub struct ClientSettings {
    /// Timeout for the entire request in ms (0 indicates no timeout).
    #[uniffi(default = 60_000)]
    pub timeout: u32,
    /// Maximum amount of redirects to follow (0 means redirects are not allowed)
    #[uniffi(default = 10)]
    pub redirect_limit: u32,
    /// OHTTP channel to use for all requests (if any)
    #[cfg(feature = "ohttp")]
    pub ohttp_channel: Option<String>,
    /// Client default user-agent.
    ///
    /// This overrides the global default user-agent and is used when no `User-agent` header is set
    /// directly in the Request.
    #[uniffi(default = None)]
    pub user_agent: Option<String>,
}

impl ClientSettings {
    pub fn update_from_global_settings(&mut self) {
        let settings = GLOBAL_SETTINGS.read();
        if self.user_agent.is_none() {
            self.user_agent = settings.default_user_agent.clone();
        }
    }
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            timeout: 60000,
            redirect_limit: 10,
            user_agent: None,
            #[cfg(feature = "ohttp")]
            ohttp_channel: None,
        }
    }
}

#[cfg(test)]
mod test {
    use url::Url;

    use super::*;
    use crate::settings;

    #[test]
    fn test_user_agent() {
        let mut req = Request::get(Url::parse("http://example.com/").unwrap());
        // No default user agent
        let client = Client::new(ClientSettings::default());
        client.set_user_agent(&mut req).unwrap();
        assert_eq!(req.headers.get(USER_AGENT), None);
        // Global user-agent set
        settings::set_global_default_user_agent("global-user-agent".into());
        let client = Client::new(ClientSettings::default());
        let mut req = Request::get(Url::parse("http://example.com/").unwrap());
        client.set_user_agent(&mut req).unwrap();
        assert_eq!(req.headers.get(USER_AGENT), Some("global-user-agent"));
        // ClientSettings overrides that
        let client = Client::new(ClientSettings {
            user_agent: Some("client-settings-user-agent".into()),
            ..ClientSettings::default()
        });
        let mut req = Request::get(Url::parse("http://example.com/").unwrap());
        client.set_user_agent(&mut req).unwrap();
        assert_eq!(
            req.headers.get(USER_AGENT),
            Some("client-settings-user-agent")
        );
        // Request header overrides that
        let mut req = Request::get(Url::parse("http://example.com/").unwrap());
        req.headers
            .insert(USER_AGENT, "request-user-agent")
            .unwrap();
        client.set_user_agent(&mut req).unwrap();
        assert_eq!(req.headers.get(USER_AGENT), Some("request-user-agent"));
    }
}
