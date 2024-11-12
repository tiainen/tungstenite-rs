use http::{header::SEC_WEBSOCKET_EXTENSIONS, HeaderMap, HeaderValue};

/// `Sec-WebSocket-Extensions` header, defined in [RFC6455][RFC6455_11.3.2]
///
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebSocketExtensions(Vec<WebSocketExtension>);

impl WebSocketExtensions {
    /// Create a `WebSocketExtensions` from a `Vec<WebSocketExtension>`.
    pub fn new(extensions: Vec<WebSocketExtension>) -> Self {
        Self(extensions)
    }

    /// Create a `WebSocketExtensions` from a map of http headers.
    pub fn from_headers(headers: &HeaderMap) -> Self {
        Self(
            headers
                .iter()
                .filter(|(key, _)| key.as_str() == SEC_WEBSOCKET_EXTENSIONS.as_str())
                .flat_map(|(_, value)| {
                    WebSocketExtensions::from(value)
                        .iter()
                        .map(|w| w.to_owned())
                        .collect::<Vec<_>>()
                })
                .collect(),
        )
    }

    /// Write the header value
    pub fn write_headers(&self, headers: &mut HeaderMap) {
        for accepted_offer in self.iter() {
            let proto = accepted_offer.proto();
            headers.append(SEC_WEBSOCKET_EXTENSIONS, HeaderValue::from_str(&proto).unwrap());
        }
    }

    /// An iterator over the `WebSocketExtension`s in `SecWebsocketExtensions` header(s).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_header_value_no_params() {
        let header_value = HeaderValue::from_static("extension-name");
        let websocket_extensions = WebSocketExtensions::from(&header_value);

        assert_eq!(websocket_extensions.0.len(), 1);
        assert_eq!(
            websocket_extensions.0.get(0).map(WebSocketExtension::name),
            Some("extension-name")
        );
        assert_eq!(
            websocket_extensions.0.get(0).map(WebSocketExtension::params).unwrap().next(),
            None
        );
    }

    #[test]
    fn test_from_header_value_empty_parameter() {
        let header_value = HeaderValue::from_static("extension-name; param");
        let websocket_extensions = WebSocketExtensions::from(&header_value);

        assert_eq!(websocket_extensions.0.len(), 1);
        assert_eq!(
            websocket_extensions.0.get(0).map(WebSocketExtension::name),
            Some("extension-name")
        );
        let mut params = websocket_extensions.0.get(0).map(WebSocketExtension::params).unwrap();
        assert_eq!(params.next(), Some(("param", None)));
        assert!(params.next().is_none());
    }

    #[test]
    fn test_from_header_value_parameter_with_value() {
        let header_value = HeaderValue::from_static("extension-name; param=5");
        let websocket_extensions = WebSocketExtensions::from(&header_value);

        assert_eq!(websocket_extensions.0.len(), 1);
        assert_eq!(
            websocket_extensions.0.get(0).map(WebSocketExtension::name),
            Some("extension-name")
        );
        let mut params = websocket_extensions.0.get(0).map(WebSocketExtension::params).unwrap();
        assert_eq!(params.next(), Some(("param", Some("5"))));
        assert!(params.next().is_none());
    }

    #[test]
    fn test_from_header_value_multiple_parameters() {
        let header_value =
            HeaderValue::from_static("extension-name; param-1=5; param-2; param-3=yes");
        let websocket_extensions = WebSocketExtensions::from(&header_value);

        assert_eq!(websocket_extensions.0.len(), 1);
        assert_eq!(
            websocket_extensions.0.get(0).map(WebSocketExtension::name),
            Some("extension-name")
        );
        let mut params = websocket_extensions.0.get(0).map(WebSocketExtension::params).unwrap();
        assert_eq!(params.next(), Some(("param-1", Some("5"))));
        assert_eq!(params.next(), Some(("param-2", None)));
        assert_eq!(params.next(), Some(("param-3", Some("yes"))));
        assert!(params.next().is_none());
    }

    #[test]
    fn test_from_header_value_with_multiple_extensions() {
        let header_value = HeaderValue::from_static("extension-name-1; param-1=5; param-2, extension-name-2, extension-name-3; param-1; param-2");
        let websocket_extensions = WebSocketExtensions::from(&header_value);

        assert_eq!(websocket_extensions.0.len(), 3);
        assert_eq!(
            websocket_extensions.0.get(0).map(WebSocketExtension::name),
            Some("extension-name-1")
        );
        assert_eq!(
            websocket_extensions.0.get(1).map(WebSocketExtension::name),
            Some("extension-name-2")
        );
        assert_eq!(
            websocket_extensions.0.get(2).map(WebSocketExtension::name),
            Some("extension-name-3")
        );
        let mut params = websocket_extensions.0.get(0).map(WebSocketExtension::params).unwrap();
        assert_eq!(params.next(), Some(("param-1", Some("5"))));
        assert_eq!(params.next(), Some(("param-2", None)));
        assert!(params.next().is_none());
        assert_eq!(
            websocket_extensions.0.get(1).map(WebSocketExtension::params).unwrap().next(),
            None
        );
        let mut params = websocket_extensions.0.get(2).map(WebSocketExtension::params).unwrap();
        assert_eq!(params.next(), Some(("param-1", None)));
        assert_eq!(params.next(), Some(("param-2", None)));
        assert!(params.next().is_none());
    }

    #[test]
    fn test_websocket_extension_proto() {
        let websocket_extension =
            WebSocketExtension { name: "extension".to_owned(), params: vec![] };
        let proto = websocket_extension.proto();

        assert_eq!(&proto, "extension");
    }

    #[test]
    fn test_websocket_extension_proto_parameter() {
        let websocket_extension = WebSocketExtension {
            name: "extension".to_owned(),
            params: vec![("param".to_owned(), None)],
        };
        let proto = websocket_extension.proto();

        assert_eq!(&proto, "extension; param");
    }

    #[test]
    fn test_websocket_extension_proto_parameter_with_value() {
        let websocket_extension = WebSocketExtension {
            name: "extension".to_owned(),
            params: vec![("param".to_owned(), Some("5".to_owned()))],
        };
        let proto = websocket_extension.proto();

        assert_eq!(&proto, "extension; param=5");
    }

    #[test]
    fn test_websocket_extension_proto_multiple_parameters() {
        let websocket_extension = WebSocketExtension {
            name: "extension".to_owned(),
            params: vec![
                ("param-1".to_owned(), Some("5".to_owned())),
                ("param-2".to_owned(), None),
                ("param-3".to_owned(), Some("yes".to_owned())),
            ],
        };
        let proto = websocket_extension.proto();

        assert_eq!(&proto, "extension; param-1=5; param-2; param-3=yes");
    }
}
