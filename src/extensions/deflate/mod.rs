//! WebSocket Extension permessage-deflate

use super::WebSocketExtension;

pub(crate) const PERMESSAGE_DEFLATE_NAME: &str = "permessage-deflate";
pub(crate) const PARAM_CLIENT_MAX_WINDOW_BITS: &str = "client_max_window_bits";
pub(crate) const PARAM_CLIENT_NO_CONTEXT_TAKEOVER: &str = "client_no_context_takeover";
pub(crate) const PARAM_SERVER_MAX_WINDOW_BITS: &str = "server_max_window_bits";
pub(crate) const PARAM_SERVER_NO_CONTEXT_TAKEOVER: &str = "server_no_context_takeover";

/// Configuration for the deflate extension
#[derive(Clone, Copy, Debug)]
pub struct PerMessageDeflate {
    /// The max size of the sliding window. If the other endpoint selects a smaller size, that size
    /// will be used instead. This must be an integer between 8 and 15 inclusive.
    /// Default: 15
    pub max_window_bits: u8,
    /// Indicates whether to ask the other endpoint to reset the sliding window for each message.
    /// Default: false
    pub request_no_context_takeover: bool,
    /// Indicates whether this endpoint will agree to reset the sliding window for each message it
    /// compresses. If this endpoint won't agree to reset the sliding window, then the handshake
    /// will fail if this endpoint is a client and the server requests no context takeover.
    /// Default: true
    pub accept_no_context_takeover: bool,
}

impl Default for PerMessageDeflate {
    fn default() -> PerMessageDeflate {
        PerMessageDeflate {
            max_window_bits: 15,
            request_no_context_takeover: false,
            accept_no_context_takeover: true,
        }
    }
}

impl PerMessageDeflate {
    /// deflate protocol
    pub(crate) fn create_extension(&mut self) -> WebSocketExtension {
        let mut params = vec![];
        if self.max_window_bits < 15 {
            params.push((
                PARAM_CLIENT_MAX_WINDOW_BITS.to_owned(),
                Some(self.max_window_bits.to_string()),
            ));
            params.push((
                PARAM_SERVER_MAX_WINDOW_BITS.to_owned(),
                Some(self.max_window_bits.to_string()),
            ));
        } else {
            params.push((PARAM_CLIENT_MAX_WINDOW_BITS.to_owned(), None));
        }

        if self.request_no_context_takeover {
            params.push((PARAM_SERVER_NO_CONTEXT_TAKEOVER.to_owned(), None));
        }

        WebSocketExtension { name: PERMESSAGE_DEFLATE_NAME.to_owned(), params }
    }
}
