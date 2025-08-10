use futures::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::accept_async;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:9001").await.unwrap();
    println!("Server running on ws://127.0.0.1:9001");

    let sentence = Arc::new(Mutex::new(String::new()));
    let (tx, _rx) = broadcast::channel(100);

    while let Ok((stream, _)) = listener.accept().await {
        let sentence = Arc::clone(&sentence);
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let ws_stream = accept_async(stream).await.unwrap();
            let (mut write, mut read) = ws_stream.split();

            // Send current sentence to new client
            {
                let current = sentence.lock().unwrap().clone();
                let _ = write.send(current.clone().into()).await;
            }

            loop {
                tokio::select! {
                    Some(Ok(msg)) = read.next() => {
                        let msg_text = msg.to_text().unwrap_or("");
                        if msg_text.len() == 1 {
                            let mut s = sentence.lock().unwrap();
                            s.push_str(msg_text);
                            println!("Updated sentence: {}", s);
                            let _ = tx.send(s.clone());
                        }
                    }

                    Ok(msg) = rx.recv() => {
                        let _ = write.send(msg.into()).await;
                    }
                }
            }
        });
    }
}
