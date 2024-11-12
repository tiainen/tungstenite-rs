//! WebSocket Extensions, as defined in [RFC6455][RFC6455_9]

use crate::error::{Error, ProtocolError, Result};
use crate::handshake::headers::WebSocketExtensions;

/// Configures the websocket extensions to be offered during
/// negotiation.
#[derive(Clone, Copy, Debug, Default)]
#[allow(missing_copy_implementations)]
pub struct ExtensionsConfig {}

impl ExtensionsConfig {
    /// Create extensions to be offered to the server
    pub fn create_offers(&self) -> WebSocketExtensions {
        WebSocketExtensions::new(vec![])
    }

    /// Negotiate the extensions that are offered by the client
    /// and return a list of the extensions that are accepted by
    /// the server as well as an optional WebSocket context
    /// that will be used by the server when processing data
    /// frames.
    pub fn negotiate_offers(
        &self,
        _offered_extensions: &WebSocketExtensions,
    ) -> Result<(WebSocketExtensions, Option<ExtensionsContext>)> {
        Ok((WebSocketExtensions::new(vec![]), None))
    }

    /// Verify the extensions and create the WebSocket context to be used
    /// by the client when processing data frames.
    pub fn verify_extensions(
        &self,
        agreed_extensions: &WebSocketExtensions,
    ) -> Result<Option<ExtensionsContext>> {
        if let Some(extension) = agreed_extensions.iter().next() {
            // The client didn't request anything, but got something
            return Err(Error::Protocol(ProtocolError::InvalidExtension(
                extension.name().to_string(),
            )));
        }

        Ok(None)
    }
}

/// Contains the websocket extensions that were agreed upon
/// by both sides.
#[derive(Debug, Default)]
#[allow(missing_copy_implementations)]
pub struct ExtensionsContext {}
