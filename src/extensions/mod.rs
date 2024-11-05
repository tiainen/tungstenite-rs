//! WebSocket extensions.

pub mod deflate;

use deflate::{
    DeflateConfig, DeflateContext, PARAM_CLIENT_MAX_WINDOW_BITS, PARAM_CLIENT_NO_CONTEXT_TAKEOVER,
    PARAM_SERVER_MAX_WINDOW_BITS, PARAM_SERVER_NO_CONTEXT_TAKEOVER, PERMESSAGE_DEFLATE_NAME,
};
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
            for value in values.into_iter() {
                if value.contains('=') {
                    let mut param_with_value = value.split('=');
                    let param_name = param_with_value.next().unwrap().trim().to_owned();
                    let param_value = param_with_value.next().map(|v| v.trim().to_owned());
                    params.push((param_name, param_value));
                } else {
                    params.push((value.trim().to_owned(), None));
                }
            }
            extensions.push(WebSocketExtension { name: name.to_owned(), params });
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
            proto.push_str(&key);
            if let Some(val) = val {
                proto.push('=');
                proto.push_str(&val);
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
    /// Configuration for the permessage-deflate extension
    pub deflate: Option<DeflateConfig>,
}

impl Extensions {
    pub(crate) fn create_offers(&self) -> Vec<WebSocketExtension> {
        let mut extension_offers = vec![];
        if let Some(mut deflate) = self.deflate {
            extension_offers.push(deflate.create_extension());
        }

        extension_offers
    }

    pub(crate) fn negotiate_offers(
        &self,
        offers: Vec<WebSocketExtension>,
    ) -> Result<(Vec<WebSocketExtension>, ResolvedExtensions)> {
        let mut accepted_offers = vec![];
        let mut resolved_extensions = ResolvedExtensions::default();

        if let Some(deflate) = self.deflate {
            if let Some(offer) = offers.iter().find(|offer| offer.name == PERMESSAGE_DEFLATE_NAME) {
                let mut rejected = false;
                let mut params = vec![];
                for (key, val) in offer.params.iter() {
                    match key.as_str() {
                        PARAM_CLIENT_MAX_WINDOW_BITS => {
                            if let Some(val) = val {
                                let max_windows_bits = val.parse::<u8>().ok();
                                if let Some(max_window_bits) = max_windows_bits {
                                    if max_window_bits < 8
                                        || max_window_bits > 15
                                        || max_window_bits >= deflate.max_window_bits
                                    {
                                        rejected = true;
                                        break;
                                    }
                                }
                            }
                        }
                        PARAM_CLIENT_NO_CONTEXT_TAKEOVER => {
                            params.push((PARAM_CLIENT_NO_CONTEXT_TAKEOVER.to_owned(), None));
                        }
                        PARAM_SERVER_MAX_WINDOW_BITS => {}
                        PARAM_SERVER_NO_CONTEXT_TAKEOVER => {
                            params.push((PARAM_SERVER_NO_CONTEXT_TAKEOVER.to_owned(), None));
                        }
                        _ => {
                            break;
                        }
                    }
                }

                if !rejected {
                    let deflate_extension = WebSocketExtension {
                        name: PERMESSAGE_DEFLATE_NAME.to_owned(),
                        params: params.clone(),
                    };
                    accepted_offers.push(deflate_extension);
                    let p = params.iter().map(|(k, v)| (k.as_str(), v.as_ref().map(|v| v.as_str())));
                    resolved_extensions.deflate = Some(DeflateContext::new_from_extension_params(deflate, p)?);
                }
            }
        }

        Ok((accepted_offers, resolved_extensions))
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
                        deflate: Some(DeflateContext::new_from_extension_params(deflate, extension.params())?),
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
pub struct ResolvedExtensions {
    /// Resolved context for the permessage-deflate extension
    pub deflate: Option<DeflateContext>,
}

/*
impl TryFrom<Vec<WebSocketExtension>> for ResolvedExtensions {
    type Error = Error;

    fn try_from(extensions: Vec<WebSocketExtension>) -> std::result::Result<Self, Self::Error> {
        let mut resolved_extensions = ResolvedExtensions::default();

        #[cfg(feature = "deflate")]
        {
            if let Some(extension) =
                extensions.iter().filter(|e| e.name() == PERMESSAGE_DEFLATE_NAME).next()
            {
                resolved_extensions.deflate = Some(DeflateContext::new_from_extension_params(extension.params())?);
            }
        }

        Ok(resolved_extensions)
    }
}
*/