//! HTTP client utilities for centralized client initialization.

use reqwest::{Client, ClientBuilder};
use std::time::Duration;

use crate::Result;

/// Build an HTTP client with the specified timeout.
///
/// This utility centralizes HTTP client initialization across the codebase,
/// ensuring consistent timeout configuration and error handling.
///
/// # Arguments
///
/// * `timeout` - The request timeout duration
///
/// # Returns
///
/// A configured `reqwest::Client` or a `KlyntbotError` on failure
///
/// # Examples
///
/// ```ignore
/// use common::build_http_client;
/// use std::time::Duration;
///
/// let client = build_http_client(Duration::from_secs(30))?;
/// ```
pub fn build_http_client(timeout: Duration) -> Result<Client> {
    build_http_client_with_builder(|builder| builder.timeout(timeout))
}

/// Build an HTTP client with custom builder configuration.
///
/// This utility allows for custom configuration while maintaining
/// centralized error handling.
///
/// # Arguments
///
/// * `configure` - A closure that takes a `ClientBuilder` and returns a configured `ClientBuilder`
///
/// # Returns
///
/// A configured `reqwest::Client` or a `KlyntbotError` on failure
///
/// # Examples
///
/// ```ignore
/// use common::build_http_client_with_builder;
/// use std::time::Duration;
///
/// let client = build_http_client_with_builder(|builder| {
///     builder
///         .timeout(Duration::from_secs(30))
///         .proxy(proxy)
/// })?;
/// ```
pub fn build_http_client_with_builder<F>(configure: F) -> Result<Client>
where
    F: FnOnce(ClientBuilder) -> ClientBuilder,
{
    let builder = Client::builder();
    let configured = configure(builder);
    configured.build().map_err(|e| {
        crate::KlyntbotError::Channel(crate::ChannelError::ConnectionFailed(format!(
            "Failed to build HTTP client: {}",
            e
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_http_client_success() {
        let result = build_http_client(Duration::from_secs(30));
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_http_client_various_timeouts() {
        let timeouts = vec![
            Duration::from_secs(5),
            Duration::from_secs(30),
            Duration::from_secs(120),
        ];

        for timeout in timeouts {
            let result = build_http_client(timeout);
            assert!(result.is_ok(), "Failed for timeout: {:?}", timeout);
        }
    }

    #[test]
    fn test_build_http_client_with_builder() {
        let result =
            build_http_client_with_builder(|builder| builder.timeout(Duration::from_secs(30)));
        assert!(result.is_ok());
    }
}
