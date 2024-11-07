//! WebSocket extensions.

#[cfg(feature = "deflate")]
pub mod deflate;

#[cfg(feature = "deflate")]
use deflate::{DeflateConfig, DeflateContext, PERMESSAGE_DEFLATE_NAME};
use http::HeaderValue;

use crate::{
    error::{ProtocolError, Result},
    Error,
};

/// `Sec-WebSocket-Extensions` header, defined in [RFC6455][RFC6455_11.3.2]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebSocketExtensions(Vec<WebSocketExtension>);

impl WebSocketExtensions {
    /// An iterator over the `WebsocketExtension`s in `SecWebsocketExtensions` header(s).
    pub fn iter(&self) -> impl Iterator<Item = &WebSocketExtension> {
        self.0.iter()
    }
}

impl From<&HeaderValue> for WebSocketExtensions {
    fn from(header_value: &HeaderValue) -> Self {
        let mut extensions = vec![];
        let extension_values = header_value.to_str().unwrap().split(',');
        for extension_value in extension_values.into_iter() {
            let mut values = extension_value.split(';');
            let name = values.next().unwrap_or("");
            let mut params = vec![];
            for value in values {
                if value.contains('=') {
                    let mut param_with_value = value.split('=');
                    let param_name = param_with_value.next().unwrap().trim().to_owned();
                    let param_value = param_with_value.next().map(|v| v.trim().to_owned());
                    params.push((param_name, param_value));
                } else {
                    params.push((value.trim().to_owned(), None));
                }
            }
            extensions.push(WebSocketExtension { name: name.trim().to_owned(), params });
        }
        WebSocketExtensions(extensions)
    }
}

/// A WebSocket extension containing the name and parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebSocketExtension {
    name: String,
    params: Vec<(String, Option<String>)>,
}

impl WebSocketExtension {
    /// Get the name of the extension.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Create the protocol string for the Sec-WebSocket-Extensions header value
    pub fn proto(&self) -> String {
        let mut proto = String::new();

        proto.push_str(&self.name);

        for (key, val) in &self.params {
            proto.push_str("; ");
            proto.push_str(key);
            if let Some(val) = val {
                proto.push('=');
                proto.push_str(val);
            }
        }

        proto
    }

    /// An iterator over the parameters of this extension.
    pub fn params(&self) -> impl Iterator<Item = (&str, Option<&str>)> {
        self.params.iter().map(|(k, v)| (k.as_str(), v.as_ref().map(|v| v.as_str())))
    }
}

/// Struct for defining WebSocket extensions.
#[derive(Copy, Clone, Debug, Default)]
pub struct Extensions {
    #[cfg(feature = "deflate")]
    /// Configuration for the permessage-deflate extension
    pub deflate: Option<DeflateConfig>,
}

impl Extensions {
    pub(crate) fn create_offers(&self) -> Vec<WebSocketExtension> {
        #[cfg(feature = "deflate")]
        {
            let mut extension_offers = vec![];
            if let Some(mut deflate) = self.deflate {
                extension_offers.push(deflate.create_extension());
            }
            extension_offers
        }

        #[cfg(not(feature = "deflate"))]
        {
            vec![]
        }
    }

    pub(crate) fn negotiate_offers(
        &self,
        _offers: Vec<WebSocketExtension>,
    ) -> Result<Vec<WebSocketExtension>> {
        #[cfg(feature = "deflate")]
        {
            let mut accepted_offers = vec![];

            if let Some(deflate) = self.deflate {
                if let Some(accepted_offer) =
                    _offers.iter().find_map(|offer| deflate.accept_offer(offer))
                {
                    accepted_offers.push(accepted_offer);
                }
            }

            Ok(accepted_offers)
        }

        #[cfg(not(feature = "deflate"))]
        {
            Ok(vec![])
        }
    }

    pub(crate) fn resolve_extensions(
        &self,
        _accepted_offers: Vec<WebSocketExtension>,
    ) -> Result<Option<ResolvedExtensions>> {
        #[cfg(feature = "deflate")]
        if let Some(deflate) = self.deflate {
            let mut resolved_extensions = ResolvedExtensions { deflate: None };
            if let Some(extension) =
                _accepted_offers.iter().find(|e| e.name() == PERMESSAGE_DEFLATE_NAME)
            {
                resolved_extensions.deflate =
                    Some(DeflateContext::new_from_extension_params(deflate, extension.params())?);
                Ok(Some(resolved_extensions))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }

        #[cfg(not(feature = "deflate"))]
        {
            Ok(None)
        }
    }

    pub(crate) fn verify_extensions(
        &self,
        agreed: &WebSocketExtensions,
    ) -> Result<Option<ResolvedExtensions>> {
        #[cfg(feature = "deflate")]
        {
            let mut resolved_extensions = None;

            if let Some(deflate) = self.deflate {
                for extension in agreed.iter() {
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

                    resolved_extensions = Some(ResolvedExtensions {
                        deflate: Some(DeflateContext::new_from_extension_params(
                            deflate,
                            extension.params(),
                        )?),
                    });
                }

                return Ok(resolved_extensions);
            }
        }

        if let Some(extension) = agreed.iter().next() {
            // The client didn't request anything, but got something
            return Err(Error::Protocol(ProtocolError::InvalidExtension(
                extension.name().to_string(),
            )));
        }

        Ok(None)
    }
}

/// Struct for defining resolved WebSocket extensions.
#[derive(Debug, Default)]
#[allow(missing_copy_implementations)]
pub struct ResolvedExtensions {
    #[cfg(feature = "deflate")]
    /// Resolved context for the permessage-deflate extension
    pub deflate: Option<DeflateContext>,
}
