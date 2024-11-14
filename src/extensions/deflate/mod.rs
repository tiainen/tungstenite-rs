//! WebSocket Per-Message Compression extension, as defined in [RFC7692]

use flate2::{Compress, Compression, Decompress, Status};
use thiserror::Error;

#[cfg(feature = "handshake")]
use crate::handshake::headers::WebSocketExtension;

#[cfg(feature = "handshake")]
const PERMESSAGE_DEFLATE_NAME: &str = "permessage-deflate";
#[cfg(feature = "handshake")]
const PARAM_CLIENT_MAX_WINDOW_BITS: &str = "client_max_window_bits";
#[cfg(feature = "handshake")]
const PARAM_CLIENT_NO_CONTEXT_TAKEOVER: &str = "client_no_context_takeover";
#[cfg(feature = "handshake")]
const PARAM_SERVER_MAX_WINDOW_BITS: &str = "server_max_window_bits";
#[cfg(feature = "handshake")]
const PARAM_SERVER_NO_CONTEXT_TAKEOVER: &str = "server_no_context_takeover";

const TRAILER: [u8; 4] = [0x00, 0x00, 0xff, 0xff];

/// Errors from `permessage-deflate` extension.
#[derive(Debug, Error)]
pub enum DeflateError {
    /// Compress failed
    #[error("Failed to compress")]
    Compress(#[source] std::io::Error),
    /// Decompress failed
    #[error("Failed to decompress")]
    Decompress(#[source] std::io::Error),

    /// Extension negotiation failed.
    #[error("Extension negotiation failed: {0:?}")]
    Negotiation(#[source] NegotiationError),
}

/// Errors from `permessage-deflate` extension negotiation.
#[derive(Debug, Error)]
pub enum NegotiationError {
    /// Unknown parameter in a negotiation response.
    #[error("Unknown parameter in a negotiation response: {0}")]
    UnknownParameter(String),
    /// Duplicate parameter in a negotiation response.
    #[error("Duplicate parameter in a negotiation response: {0}")]
    DuplicateParameter(String),
    /// Received `client_max_window_bits` in a negotiation response for an offer without it.
    #[error("Received client_max_window_bits in a negotiation response for an offer without it")]
    UnexpectedClientMaxWindowBits,
    /// Received unsupported `server_max_window_bits` in a negotiation response.
    #[error("Received unsupported server_max_window_bits in a negotiation response")]
    ServerMaxWindowBitsNotSupported,
    /// Invalid `client_max_window_bits` value in a negotiation response.
    #[error("Invalid client_max_window_bits value in a negotiation response: {0}")]
    InvalidClientMaxWindowBitsValue(String),
    /// Invalid `server_max_window_bits` value in a negotiation response.
    #[error("Invalid server_max_window_bits value in a negotiation response: {0}")]
    InvalidServerMaxWindowBitsValue(String),
    /// Missing `server_max_window_bits` value in a negotiation response.
    #[error("Missing server_max_window_bits value in a negotiation response")]
    MissingServerMaxWindowBitsValue,
}

/// Configuration for the per-message deflate extension.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeflateConfig {
    /// The compression level to use for the deflate algorithm.
    pub compression: Compression,
    /// Indicates whether to ask the other endpoint to reset the sliding window for each message.
    pub request_no_context_takeover: bool,
    /// Indicates whether this endpoint will agree to reset the sliding window for each message it
    /// compresses. If this endpoint won't agree to reset the sliding window, then the handshake
    /// will fail if this endpoint is a client and the server requests no context takeover.
    pub accept_no_context_takeover: bool,
}

/// Context for the resolved per-message deflate extension.
#[derive(Debug)]
pub struct DeflateContext {
    config: DeflateConfig,
    compressor: Compress,
    decompressor: Decompress,
}

#[cfg(feature = "handshake")]
impl DeflateConfig {
    pub(crate) fn name(&self) -> &str {
        PERMESSAGE_DEFLATE_NAME
    }

    pub(crate) fn accept_offer(&self, offer: &WebSocketExtension) -> Option<WebSocketExtension> {
        if offer.name == PERMESSAGE_DEFLATE_NAME {
            let mut params = Vec::new();

            let mut config = DeflateConfig::default();
            let mut seen_server_no_context_takeover = false;
            let mut seen_client_no_context_takeover = false;
            let mut seen_client_max_window_bits = false;

            for (key, val) in offer.params() {
                match key {
                    PARAM_SERVER_NO_CONTEXT_TAKEOVER => {
                        // Invalid offer with multiple params with same name is declined.
                        if seen_server_no_context_takeover {
                            return None;
                        }
                        seen_server_no_context_takeover = true;
                        config.request_no_context_takeover = true;
                        params.push((PARAM_SERVER_NO_CONTEXT_TAKEOVER.to_owned(), None));
                    }

                    PARAM_CLIENT_NO_CONTEXT_TAKEOVER => {
                        // Invalid offer with multiple params with same name is declined.
                        if seen_client_no_context_takeover {
                            return None;
                        }
                        seen_client_no_context_takeover = true;
                        config.accept_no_context_takeover = true;
                        params.push((PARAM_CLIENT_NO_CONTEXT_TAKEOVER.to_owned(), None));
                    }

                    // Max window bits are not supported at the moment.
                    PARAM_SERVER_MAX_WINDOW_BITS => {
                        // Decline offer with invalid parameter value.
                        // `server_max_window_bits` requires a value in range [8, 15].
                        if let Some(bits) = val {
                            if !is_valid_max_window_bits(bits) {
                                return None;
                            }
                        } else {
                            return None;
                        }

                        // A server declines an extension negotiation offer with this parameter
                        // if the server doesn't support it.
                        return None;
                    }

                    // Not supported, but server may ignore and accept the offer.
                    PARAM_CLIENT_MAX_WINDOW_BITS => {
                        // Decline offer with invalid parameter value.
                        // `client_max_window_bits` requires a value in range [8, 15] or no value.
                        if let Some(bits) = val {
                            if !is_valid_max_window_bits(bits) {
                                return None;
                            }
                        }

                        // Invalid offer with multiple params with same name is declined.
                        if seen_client_max_window_bits {
                            return None;
                        }
                        seen_client_max_window_bits = true;
                    }

                    // Offer with unknown parameter MUST be declined.
                    _ => {
                        return None;
                    }
                }
            }

            Some(WebSocketExtension { name: PERMESSAGE_DEFLATE_NAME.to_owned(), params })
        } else {
            None
        }
    }
}

#[cfg(feature = "handshake")]
impl From<DeflateConfig> for WebSocketExtension {
    fn from(val: DeflateConfig) -> Self {
        let mut params = vec![];

        if val.request_no_context_takeover {
            params.push((PARAM_SERVER_NO_CONTEXT_TAKEOVER.to_owned(), None));
        }

        if val.accept_no_context_takeover {
            params.push((PARAM_CLIENT_NO_CONTEXT_TAKEOVER.to_owned(), None));
        }

        Self { name: PERMESSAGE_DEFLATE_NAME.to_owned(), params }
    }
}

impl DeflateContext {
    /// Create a new context from the given extension parameters
    #[cfg(feature = "handshake")]
    pub fn new<'a, I>(config: DeflateConfig, params: I) -> Result<Self, DeflateError>
    where
        I: IntoIterator<Item = (&'a str, Option<&'a str>)>,
    {
        let mut config = DeflateConfig {
            accept_no_context_takeover: config.accept_no_context_takeover,
            ..DeflateConfig::default()
        };

        let mut seen_server_no_context_takeover = false;
        let mut seen_client_no_context_takeover = false;

        // A client MUST _Fail the WebSocket Connection_ if the peer server
        // accepted an extension negotiation offer for this extension with an
        // extension negotiation response meeting any of the following
        // conditions:
        // 1. The negotiation response contains an extension parameter not defined for use in a response.
        // 2. The negotiation response contains an extension parameter with an invalid value.
        // 3. The negotiation response contains multiple extension parameters with the same name.
        // 4. The client does not support the configuration that the response represents.
        for (key, val) in params.into_iter() {
            match key {
                PARAM_SERVER_NO_CONTEXT_TAKEOVER => {
                    // Fail the connection when the response contains multiple parameters with the same name.
                    if seen_server_no_context_takeover {
                        return Err(DeflateError::Negotiation(
                            NegotiationError::DuplicateParameter(key.to_owned()),
                        ));
                    }
                    seen_server_no_context_takeover = true;
                    // A server MAY include the "server_no_context_takeover" extension
                    // parameter in an extension negotiation response even if the extension
                    // negotiation offer being accepted by the extension negotiation
                    // response didn't include the "server_no_context_takeover" extension
                    // parameter.
                    config.request_no_context_takeover = true;
                }
                PARAM_CLIENT_NO_CONTEXT_TAKEOVER => {
                    // Fail the connection when the response contains multiple parameters with the same name.
                    if seen_client_no_context_takeover {
                        return Err(DeflateError::Negotiation(
                            NegotiationError::DuplicateParameter(key.to_owned()),
                        ));
                    }
                    seen_client_no_context_takeover = true;
                    // The server may include this parameter in the response and the client MUST support it.
                    config.accept_no_context_takeover = true;
                }
                PARAM_SERVER_MAX_WINDOW_BITS => {
                    // Fail the connection when the response contains a parameter with invalid value.
                    if let Some(bits) = val {
                        if !is_valid_max_window_bits(bits) {
                            return Err(DeflateError::Negotiation(
                                NegotiationError::InvalidServerMaxWindowBitsValue(bits.to_owned()),
                            ));
                        }
                    } else {
                        return Err(DeflateError::Negotiation(
                            NegotiationError::MissingServerMaxWindowBitsValue,
                        ));
                    }

                    // A server may include the "server_max_window_bits" extension parameter
                    // in an extension negotiation response even if the extension
                    // negotiation offer being accepted by the response didn't include the
                    // "server_max_window_bits" extension parameter.
                    //
                    // However, but we need to fail the connection because we don't support it (condition 4).
                    return Err(DeflateError::Negotiation(
                        NegotiationError::ServerMaxWindowBitsNotSupported,
                    ));
                }
                PARAM_CLIENT_MAX_WINDOW_BITS => {
                    // Fail the connection when the response contains a parameter with invalid value.
                    if let Some(bits) = val {
                        if !is_valid_max_window_bits(bits) {
                            return Err(DeflateError::Negotiation(
                                NegotiationError::InvalidClientMaxWindowBitsValue(bits.to_owned()),
                            ));
                        }
                    }

                    // Fail the connection because the parameter is invalid when the client didn't offer.
                    //
                    // If a received extension negotiation offer doesn't have the
                    // "client_max_window_bits" extension parameter, the corresponding
                    // extension negotiation response to the offer MUST NOT include the
                    // "client_max_window_bits" extension parameter.
                    return Err(DeflateError::Negotiation(
                        NegotiationError::UnexpectedClientMaxWindowBits,
                    ));
                }
                // Response with unknown parameter MUST fail the WebSocket connection.
                _ => {
                    return Err(DeflateError::Negotiation(NegotiationError::UnknownParameter(
                        key.to_owned(),
                    )));
                }
            }
        }

        Ok(config.into())
    }

    /// Compress the provided data using the configured compressor.
    pub fn compress(&mut self, data: &[u8]) -> Result<Vec<u8>, DeflateError> {
        let mut output = Vec::with_capacity(data.len());

        let before_in = self.compressor.total_in() as usize;
        while (self.compressor.total_in() as usize) - before_in < data.len() {
            let offset = (self.compressor.total_in() as usize) - before_in;
            match self
                .compressor
                .compress_vec(&data[offset..], &mut output, flate2::FlushCompress::None)
                .map_err(|e| DeflateError::Compress(e.into()))?
            {
                Status::Ok => continue,
                Status::BufError => output.reserve(4096),
                Status::StreamEnd => break,
            }
        }

        while !output.ends_with(&TRAILER) {
            output.reserve(5);
            match self
                .compressor
                .compress_vec(&[], &mut output, flate2::FlushCompress::Sync)
                .map_err(|e| DeflateError::Compress(e.into()))?
            {
                Status::Ok | Status::BufError => continue,
                Status::StreamEnd => break,
            }
        }

        output.truncate(output.len() - 4);

        if self.config.accept_no_context_takeover {
            self.compressor.reset();
        }

        Ok(output)
    }

    /// Decompress the provided data using the configured decompressor.
    pub fn decompress(
        &mut self,
        mut data: Vec<u8>,
        is_final: bool,
    ) -> Result<Vec<u8>, DeflateError> {
        if is_final {
            data.extend_from_slice(&TRAILER);
        }

        let before_in = self.decompressor.total_in() as usize;
        let mut output = Vec::with_capacity(2 * data.len());
        loop {
            let offset = (self.decompressor.total_in() as usize) - before_in;
            match self
                .decompressor
                .decompress_vec(&data[offset..], &mut output, flate2::FlushDecompress::None)
                .map_err(|e| DeflateError::Decompress(e.into()))?
            {
                Status::Ok => output.reserve(2 * output.len()),
                Status::BufError | Status::StreamEnd => break,
            }
        }

        if is_final && self.config.request_no_context_takeover {
            self.decompressor.reset(false);
        }

        Ok(output)
    }
}

impl From<DeflateConfig> for DeflateContext {
    fn from(val: DeflateConfig) -> Self {
        Self {
            config: val,
            compressor: Compress::new(val.compression, false),
            decompressor: Decompress::new(false),
        }
    }
}

// A valid `client_max_window_bits` is no value or an integer in range `[8, 15]` without leading zeros.
// A valid `server_max_window_bits` is an integer in range `[8, 15]` without leading zeros.
#[cfg(feature = "handshake")]
fn is_valid_max_window_bits(bits: &str) -> bool {
    // Note that values from `headers::SecWebSocketExtensions` is unquoted.
    matches!(bits, "8" | "9" | "10" | "11" | "12" | "13" | "14" | "15")
}

#[cfg(all(test, feature = "handshake"))]
mod tests {
    use super::{
        is_valid_max_window_bits, DeflateConfig, DeflateContext, DeflateError, NegotiationError,
        PARAM_CLIENT_MAX_WINDOW_BITS, PARAM_CLIENT_NO_CONTEXT_TAKEOVER,
        PARAM_SERVER_MAX_WINDOW_BITS, PARAM_SERVER_NO_CONTEXT_TAKEOVER, PERMESSAGE_DEFLATE_NAME,
    };
    use crate::handshake::headers::WebSocketExtension;

    #[test]
    fn accept_correct_extension() {
        let config = DeflateConfig::default();

        let accepted_extension = config
            .accept_offer(&WebSocketExtension {
                name: PERMESSAGE_DEFLATE_NAME.to_owned(),
                params: vec![],
            })
            .expect("offer should have been accepted");
        assert_eq!(accepted_extension.name, PERMESSAGE_DEFLATE_NAME);
        assert!(accepted_extension.params.is_empty());
    }

    #[test]
    fn accept_server_no_context_takeover() {
        let config = DeflateConfig::default();

        let accepted_extension = config
            .accept_offer(&WebSocketExtension {
                name: PERMESSAGE_DEFLATE_NAME.to_owned(),
                params: vec![(PARAM_SERVER_NO_CONTEXT_TAKEOVER.to_owned(), None)],
            })
            .expect("offer should have been accepted");
        assert_eq!(accepted_extension.name, PERMESSAGE_DEFLATE_NAME);
        assert_eq!(accepted_extension.params.len(), 1);
        assert_eq!(
            accepted_extension.params().next(),
            Some((PARAM_SERVER_NO_CONTEXT_TAKEOVER, None))
        );
    }

    #[test]
    fn accept_client_no_context_takeover() {
        let config = DeflateConfig::default();

        let accepted_extension = config
            .accept_offer(&WebSocketExtension {
                name: PERMESSAGE_DEFLATE_NAME.to_owned(),
                params: vec![(PARAM_CLIENT_NO_CONTEXT_TAKEOVER.to_owned(), None)],
            })
            .expect("offer should have been accepted");
        assert_eq!(accepted_extension.name, PERMESSAGE_DEFLATE_NAME);
        assert_eq!(accepted_extension.params.len(), 1);
        assert_eq!(
            accepted_extension.params().next(),
            Some((PARAM_CLIENT_NO_CONTEXT_TAKEOVER, None))
        );
    }

    #[test]
    fn reject_server_max_window_bits() {
        let config = DeflateConfig::default();

        let accepted_extension = config.accept_offer(&WebSocketExtension {
            name: PERMESSAGE_DEFLATE_NAME.to_owned(),
            params: vec![(PARAM_SERVER_MAX_WINDOW_BITS.to_owned(), None)],
        });
        assert!(accepted_extension.is_none());
    }

    #[test]
    fn ignore_client_max_window_bits() {
        let config = DeflateConfig::default();

        let accepted_extension = config
            .accept_offer(&WebSocketExtension {
                name: PERMESSAGE_DEFLATE_NAME.to_owned(),
                params: vec![(PARAM_CLIENT_MAX_WINDOW_BITS.to_owned(), None)],
            })
            .expect("offer should have been accepted");
        assert_eq!(accepted_extension.name, PERMESSAGE_DEFLATE_NAME);
        assert!(accepted_extension.params.is_empty());
    }

    #[test]
    fn reject_other_extension() {
        let config = DeflateConfig::default();

        let accepted_extension = config.accept_offer(&WebSocketExtension {
            name: "custom-extension".to_owned(),
            params: vec![],
        });
        assert!(accepted_extension.is_none());
    }

    #[test]
    fn reject_unknown_parameter() {
        let config = DeflateConfig::default();

        let accepted_extension = config.accept_offer(&WebSocketExtension {
            name: "custom-extension".to_owned(),
            params: vec![("unknown_parameter".to_owned(), None)],
        });
        assert!(accepted_extension.is_none());
    }

    #[test]
    fn reject_multiple_server_no_context_takeover() {
        let config = DeflateConfig::default();

        let accepted_extension = config.accept_offer(&WebSocketExtension {
            name: "custom-extension".to_owned(),
            params: vec![
                (PARAM_SERVER_NO_CONTEXT_TAKEOVER.to_owned(), None),
                (PARAM_SERVER_NO_CONTEXT_TAKEOVER.to_owned(), None),
            ],
        });
        assert!(accepted_extension.is_none());
    }

    #[test]
    fn reject_multiple_client_no_context_takeover() {
        let config = DeflateConfig::default();

        let accepted_extension = config.accept_offer(&WebSocketExtension {
            name: "custom-extension".to_owned(),
            params: vec![
                (PARAM_CLIENT_NO_CONTEXT_TAKEOVER.to_owned(), None),
                (PARAM_CLIENT_NO_CONTEXT_TAKEOVER.to_owned(), None),
            ],
        });
        assert!(accepted_extension.is_none());
    }

    #[test]
    fn reject_multiple_client_max_window_bits() {
        let config = DeflateConfig::default();

        let accepted_extension = config.accept_offer(&WebSocketExtension {
            name: "custom-extension".to_owned(),
            params: vec![
                (PARAM_CLIENT_MAX_WINDOW_BITS.to_owned(), None),
                (PARAM_CLIENT_MAX_WINDOW_BITS.to_owned(), None),
            ],
        });
        assert!(accepted_extension.is_none());
    }

    #[test]
    fn new_context_from_extension() {
        let config = DeflateConfig::default();

        let context = DeflateContext::new(config, vec![]);
        assert!(context.is_ok());
    }

    #[test]
    fn new_context_with_server_no_context_takeover() {
        let config = DeflateConfig::default();

        let context = DeflateContext::new(config, vec![(PARAM_SERVER_NO_CONTEXT_TAKEOVER, None)]);
        assert!(context.is_ok());
    }

    #[test]
    fn new_context_with_client_no_context_takeover() {
        let config = DeflateConfig::default();

        let context = DeflateContext::new(config, vec![(PARAM_CLIENT_NO_CONTEXT_TAKEOVER, None)]);
        assert!(context.is_ok());
    }

    #[test]
    fn new_context_with_unknown_parameter_fails() {
        let config = DeflateConfig::default();

        let context = DeflateContext::new(config, vec![("unknown-parameter", None)]);
        assert!(context.is_err());
        assert!(matches!(
            context,
            Err(DeflateError::Negotiation(NegotiationError::UnknownParameter(_)))
        ));
    }

    #[test]
    fn new_context_with_multiple_server_no_context_takeover_fails() {
        let config = DeflateConfig::default();

        let context = DeflateContext::new(
            config,
            vec![
                (PARAM_SERVER_NO_CONTEXT_TAKEOVER, None),
                (PARAM_SERVER_NO_CONTEXT_TAKEOVER, None),
            ],
        );
        assert!(context.is_err());
        assert!(matches!(
            context,
            Err(DeflateError::Negotiation(NegotiationError::DuplicateParameter(_)))
        ));
    }

    #[test]
    fn new_context_with_multiple_client_no_context_takeover_fails() {
        let config = DeflateConfig::default();

        let context = DeflateContext::new(
            config,
            vec![
                (PARAM_CLIENT_NO_CONTEXT_TAKEOVER, None),
                (PARAM_CLIENT_NO_CONTEXT_TAKEOVER, None),
            ],
        );
        assert!(context.is_err());
        assert!(matches!(
            context,
            Err(DeflateError::Negotiation(NegotiationError::DuplicateParameter(_)))
        ));
    }

    #[test]
    fn valid_max_window_bits() {
        for bits in 8..=15 {
            assert!(is_valid_max_window_bits(&bits.to_string()));
        }
    }

    #[test]
    fn invalid_max_window_bits() {
        assert!(!is_valid_max_window_bits(""));
        assert!(!is_valid_max_window_bits("0"));
        assert!(!is_valid_max_window_bits("08"));
        assert!(!is_valid_max_window_bits("+8"));
        assert!(!is_valid_max_window_bits("-8"));
    }
}
