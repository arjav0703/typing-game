use futures::{SinkExt, StreamExt};
use tokio::io::{self, AsyncBufReadExt};
use tokio_tungstenite::connect_async;

#[tokio::main]
async fn main() {
    let url = "ws://127.0.0.1:9001";
    let (ws_stream, _) = connect_async(url).await.unwrap();
    let (mut write, mut read) = ws_stream.split();

    // Task to receive sentence updates
    let read_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            println!("\nCurrent Sentence: {}", msg.to_text().unwrap_or(""));
            print!("Your letter: ");
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }
    });

    // Read one letter at a time from stdin
    let stdin = io::BufReader::new(io::stdin());
    let mut lines = stdin.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(c) = line.chars().next() {
            let _ = write.send(c.to_string().into()).await;
        }
        print!("Your letter: ");
        use std::io::Write;
        std::io::stdout().flush().unwrap();
    }

    read_task.abort();
}
