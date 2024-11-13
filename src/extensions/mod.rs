//! WebSocket Extensions, as defined in [RFC6455][RFC6455_9]

#[cfg(feature = "deflate")]
pub mod deflate;

#[cfg(feature = "deflate")]
use deflate::{DeflateConfig, DeflateContext};

use crate::error::{Error, ProtocolError, Result};
use crate::handshake::headers::WebSocketExtensions;

/// Configures the websocket extensions to be offered during
/// negotiation.
#[derive(Clone, Copy, Debug, Default)]
#[allow(missing_copy_implementations)]
pub struct ExtensionsConfig {
    #[cfg(feature = "deflate")]
    /// Configuration for the permessage-deflate extension
    pub deflate: Option<DeflateConfig>,
}

impl ExtensionsConfig {
    /// Create extensions to be offered to the server
    pub(crate) fn create_offers(&self) -> WebSocketExtensions {
        #[cfg(feature = "deflate")]
        {
            if let Some(deflate) = self.deflate {
                return WebSocketExtensions::new(vec![deflate.into()]);
            }
        }

        WebSocketExtensions::new(vec![])
    }

    /// Negotiate the extensions that are offered by the client
    /// and return a list of the extensions that are accepted by
    /// the server as well as a WebSocket context that will be
    /// used by the server when processing data frames.
    pub(crate) fn negotiate_offers(
        &self,
        _offered_extensions: &WebSocketExtensions,
    ) -> Result<(WebSocketExtensions, Option<ExtensionsContext>)> {
        #[cfg(feature = "deflate")]
        {
            if let Some(deflate) = self.deflate {
                if let Some(accepted_offer) =
                    _offered_extensions.iter().find_map(|offer| deflate.accept_offer(offer))
                {
                    let extensions_context = ExtensionsContext {
                        deflate: Some(DeflateContext::new(deflate, accepted_offer.params())?),
                    };
                    return Ok((
                        WebSocketExtensions::new(vec![accepted_offer]),
                        Some(extensions_context),
                    ));
                }
            }
        }

        Ok((WebSocketExtensions::new(vec![]), None))
    }

    /// Verify the extensions and create the WebSocket context to be used
    /// by the client when processing data frames.
    pub(crate) fn verify_extensions(
        &self,
        agreed_extensions: &WebSocketExtensions,
    ) -> Result<Option<ExtensionsContext>> {
        #[cfg(feature = "deflate")]
        {
            let mut resolved_extensions = None;

            if let Some(deflate) = self.deflate {
                for extension in agreed_extensions.iter() {
                    if extension.name != deflate.name() {
                        return Err(Error::Protocol(ProtocolError::InvalidExtension(
                            extension.name().to_string(),
                        )));
                    }

                    if resolved_extensions.is_some() {
                        return Err(Error::Protocol(ProtocolError::ExtensionConflict(
                            extension.name().to_string(),
                        )));
                    }

                    resolved_extensions = Some(ExtensionsContext {
                        deflate: Some(DeflateContext::new(deflate, extension.params())?),
                    });
                }

                return Ok(resolved_extensions);
            }
        }

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
pub struct ExtensionsContext {
    #[cfg(feature = "deflate")]
    /// Resolved context for the permessage-deflate extension
    pub deflate: Option<DeflateContext>,
}
