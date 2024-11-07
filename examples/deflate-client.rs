use tungstenite::client::connect_with_config;
#[cfg(feature = "deflate")]
use tungstenite::extensions::{deflate::DeflateConfig, Extensions};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::Message;

fn main() {
    env_logger::init();

    #[cfg(feature = "deflate")]
    let permessage_deflate = DeflateConfig::default();
    #[cfg(feature = "deflate")]
    let websocket_config = WebSocketConfig {
        extensions: Extensions { deflate: Some(permessage_deflate) },
        ..Default::default()
    };
    #[cfg(not(feature = "deflate"))]
    let websocket_config = WebSocketConfig::default();

    let (mut socket, response) =
        connect_with_config("ws://localhost:3012/socket", Some(websocket_config), 3)
            .expect("Can't connect");

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");
    for (header, value) in response.headers() {
        println!("* {header}: {}", value.to_str().unwrap());
    }

    socket.send(Message::Text("Hello WebSocket".into())).unwrap();
    loop {
        let msg = socket.read().expect("Error reading message");
        println!("Received: {msg}");
    }
    // socket.close(None);
}
