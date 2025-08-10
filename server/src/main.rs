use clap::{Parser, ValueEnum};
use futures::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::accept_async;

#[tokio::main]
async fn main() {
    let port = get_port();
    let listener = TcpListener::bind(format!("127.0.0.1::{port}"))
        .await
        .unwrap();
    println!("[Server] Listening on port {port}");

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
                            println!("Updated sentence: {s}");
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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(long)]
    pub port: Option<u16>,
}

fn get_port() -> u16 {
    let args = Cli::parse();
    args.port.unwrap_or(9001)
}
