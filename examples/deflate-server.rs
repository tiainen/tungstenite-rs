use std::{net::TcpListener, thread::spawn};

use tungstenite::{
    accept_hdr_with_config,
    extensions::{deflate::DeflateConfig, Extensions},
    handshake::server::{Request, Response},
    protocol::WebSocketConfig,
};

fn main() {
    env_logger::init();

    let permessage_deflate = DeflateConfig::default();

    let websocket_config = WebSocketConfig {
        extensions: Extensions { deflate: Some(permessage_deflate) },
        ..Default::default()
    };

    let server = TcpListener::bind("127.0.0.1:3012").unwrap();
    for stream in server.incoming() {
        spawn(move || {
            let callback = |req: &Request, mut response: Response| {
                println!("Received a new ws handshake");
                println!("The request's path is: {}", req.uri().path());
                println!("The request's headers are:");
                for (header, value) in req.headers() {
                    println!("* {header}: {}", value.to_str().unwrap());
                }

                // Let's add an additional header to our response to the client.
                let headers = response.headers_mut();
                headers.append("MyCustomHeader", ":)".parse().unwrap());
                headers.append("SOME_TUNGSTENITE_HEADER", "header_value".parse().unwrap());

                Ok(response)
            };
            let mut websocket =
                accept_hdr_with_config(stream.unwrap(), callback, Some(websocket_config)).unwrap();

            loop {
                let msg = websocket.read().unwrap();
                if msg.is_binary() || msg.is_text() {
                    websocket.send(msg).unwrap();
                }
            }
        });
    }
}
