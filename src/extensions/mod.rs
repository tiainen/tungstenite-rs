//! WebSocket extensions.

pub mod deflate;

use deflate::{
    PerMessageDeflate, PARAM_CLIENT_MAX_WINDOW_BITS, PARAM_CLIENT_NO_CONTEXT_TAKEOVER,
    PARAM_SERVER_MAX_WINDOW_BITS, PARAM_SERVER_NO_CONTEXT_TAKEOVER, PERMESSAGE_DEFLATE_NAME,
};
use http::HeaderValue;

/// WEBSOCKETEXTENSION!!!!
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebSocketExtension {
    name: String,
    params: Vec<(String, Option<String>)>,
}

impl Into<String> for WebSocketExtension {
    fn into(self) -> String {
        let mut proto = String::new();

        proto.push_str(&self.name);

        for (key, val) in self.params {
            proto.push_str("; ");
            proto.push_str(&key);
            if let Some(val) = val {
                proto.push('=');
                proto.push_str(&val);
            }
        }

        proto
    }
}

impl From<&HeaderValue> for WebSocketExtension {
    fn from(header_value: &HeaderValue) -> Self {
        let mut values = header_value.to_str().unwrap().split(';');
        let name = values.next().unwrap_or("");
        let mut params = vec![];
        for value in values.into_iter() {
            if value.contains('=') {
                let mut param_with_value = value.split('=');
                let param_name = param_with_value.next().unwrap().to_owned();
                let param_value = param_with_value.next().map(|v| v.to_owned());
                params.push((param_name, param_value));
            } else {
                params.push((value.to_owned(), None));
            }
        }
        WebSocketExtension { name: name.to_owned(), params }
    }
}

/// Struct for defining WebSocket extensions.
#[derive(Copy, Clone, Debug, Default)]
pub struct Extensions {
    /// List of extensions to apply
    pub deflate: Option<PerMessageDeflate>,
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
    ) -> Option<Vec<WebSocketExtension>> {
        let mut accepted_offers = vec![];

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
                    accepted_offers.push(WebSocketExtension {
                        name: PERMESSAGE_DEFLATE_NAME.to_owned(),
                        params,
                    });
                }
            }
        }

        Some(accepted_offers)
    }
}
