#!/bin/bash

echo "Starting Collaborative Typing Game Server..."
echo "Press Ctrl+C to stop the server"
echo ""

cd "$(dirname "$0")"

# Build and run the server
cargo run --bin server
