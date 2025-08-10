#!/bin/bash

echo "Starting Collaborative Typing Game Client..."
echo "Make sure the server is running first!"
echo ""

cd "$(dirname "$0")"

cargo run --bin client
