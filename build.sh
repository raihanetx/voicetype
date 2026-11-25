#!/usr/bin/env bash
set -euo pipefail
sudo apt install -y build-essential pkg-config libgtk-3-dev \
  libatspi2.0-dev libasound2-dev libx11-dev
cargo build --release
echo "Built: target/release/voicetype"
