#!/bin/bash

echo "Starting chaos-type-server..."
echo "Press Ctrl+C to stop the server"
echo ""

cd "$(dirname "$0")"

cargo run --bin server
