# 🚀 Collaborative Typing Game

A real-time multiplayer typing game built with Rust, featuring a beautiful terminal user interface (TUI) powered by ratatui. Players collaborate to build sentences together word by word!

![Typing Game Demo](https://img.shields.io/badge/Rust-🦀-orange) ![Terminal UI](https://img.shields.io/badge/TUI-ratatui-blue) ![WebSocket](https://img.shields.io/badge/Real--time-WebSocket-green)

## 🛠️ Prerequisites

- **Rust** 1.70+ (install from [rustup.rs](https://rustup.rs/))
- A modern terminal that supports colors and Unicode characters (Windows Terminal, iTerm2, etc.)

## 🚀 Quick Start

### 1. Clone & Build

```bash
git clone https://github.com/arjav0703/typing-game.git
cd typing-game
cargo build --release
```

### 2. Start the Server

In one terminal window:

```bash
cargo run --bin server
```

You should see:
```
Server running on ws://127.0.0.1:9001
```

### 3. Start Client(s)

In another terminal window (or multiple for multiplayer fun):

```bash
cargo run --bin client
```

