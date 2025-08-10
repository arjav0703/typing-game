#!/bin/bash

echo "Starting chaos-type-client..."
echo "Make sure the server is running first!"
echo ""

cd "$(dirname "$0")"

cargo run --bin client
