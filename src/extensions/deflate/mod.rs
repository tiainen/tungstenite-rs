//! WebSocket Extension permessage-deflate

use super::WebSocketExtension;

use flate2::{Compress, Compression, Decompress, Status};
use thiserror::Error;

pub(crate) const PERMESSAGE_DEFLATE_NAME: &str = "permessage-deflate";
pub(crate) const PARAM_CLIENT_MAX_WINDOW_BITS: &str = "client_max_window_bits";
pub(crate) const PARAM_CLIENT_NO_CONTEXT_TAKEOVER: &str = "client_no_context_takeover";
pub(crate) const PARAM_SERVER_MAX_WINDOW_BITS: &str = "server_max_window_bits";
pub(crate) const PARAM_SERVER_NO_CONTEXT_TAKEOVER: &str = "server_no_context_takeover";

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
    #[error("Extension negotiation failed")]
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

/// Configuration for the deflate extension
#[derive(Clone, Copy, Debug)]
pub struct DeflateConfig {
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

impl Default for DeflateConfig {
    fn default() -> DeflateConfig {
        DeflateConfig {
            max_window_bits: 15,
            request_no_context_takeover: false,
            accept_no_context_takeover: true,
        }
    }
}

impl DeflateConfig {
    pub(crate) fn name(&self) -> &str {
        PERMESSAGE_DEFLATE_NAME
    }

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

/// Context for the resolved deflate extension
#[derive(Debug)]
pub struct DeflateContext {
    config: DeflateConfig,
    compressor: Compress,
    decompressor: Decompress,
}

impl DeflateContext {
    /// Create a new context from the given extension
    pub fn new_from_extension(_extension: &WebSocketExtension) -> Result<Self, DeflateError> {
        let config = DeflateConfig::default();

        Ok(DeflateContext::new(config))
    }

    /// Create a new context from a configuration
    pub fn new(config: DeflateConfig) -> Self {
        DeflateContext {
            config,
            compressor: Compress::new(Compression::fast(), false),
            decompressor: Decompress::new(false),
        }
    }

    /// Compress the provided data using the configured compressor
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

    /// Decompress the provided data using the configured decompressor
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

        if is_final && self.config.accept_no_context_takeover {
            self.decompressor.reset(false);
        }

        Ok(output)
    }
}
