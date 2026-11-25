#!/usr/bin/env bash
# One-shot installer for VoiceType on Linux Mint (Cinnamon).
set -euo pipefail

echo "==> Installing system dependencies"
sudo apt update
sudo apt install -y build-essential pkg-config libgtk-3-dev \
  libatspi2.0-dev libasound2-dev libx11-dev curl xdotool notify-osd

echo "==> Installing Rust toolchain (if missing)"
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
source "$HOME/.cargo/env"

echo "==> Building VoiceType (release)"
cd "$(dirname "$0")"
cargo build --release

BIN="$PWD/target/release/voicetype"
CFG_DIR="$HOME/.config/voicetype"
DATA_DIR="$HOME/.local/share/voicetype"
mkdir -p "$CFG_DIR" "$DATA_DIR"

if [ ! -f "$CFG_DIR/config.toml" ]; then
  cat > "$CFG_DIR/config.toml" <<'TOML'
google_api_key = "PASTE_YOUR_GOOGLE_CLOUD_API_KEY_HERE"
language = "en-US"
alt_languages = ["bn-BD"]
model = "latest_long"
sample_rate = 16000
silence_threshold = 0.015
max_record_seconds = 60
TOML
  echo "!! Created $CFG_DIR/config.toml — edit it and paste your Google API key."
fi

echo "==> Creating desktop entry + autostart"
mkdir -p "$HOME/.local/share/applications" "$HOME/.config/autostart"
cat > "$HOME/.local/share/applications/voicetype.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=VoiceType
Comment=Voice dictation for Linux Mint
Exec=$BIN
Icon=audio-input-microphone
Terminal=false
Categories=Utility;Accessibility;
DESKTOP
cp "$HOME/.local/share/applications/voicetype.desktop" "$HOME/.config/autostart/voicetype.desktop"

echo
echo "==> Done."
echo "1) Edit $CFG_DIR/config.toml and paste your Google Cloud API key."
echo "2) Run: $BIN --check   (verifies microphone, AT-SPI, X11)"
echo "3) Log out and back in (or run $BIN) to start VoiceType."
echo "   If anything breaks, send me: $DATA_DIR/voicetype.log"
" path="/home/user/playground-4/voicetype/install.sh