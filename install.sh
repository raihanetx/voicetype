#!/usr/bin/env bash
# One-shot installer for VoiceType on Linux Mint (Cinnamon).
set -euo pipefail

echo "==> Installing system dependencies"
sudo apt update
sudo apt install -y build-essential pkg-config libgtk-3-dev \
  libatspi2.0-dev libasound2-dev libx11-dev curl xdotool notify-osd zenity libnotify-bin

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
  # Optional: pass the key non-interactively, e.g. GEMINI_API_KEY=... ./install.sh
  KEYVAL="${GEMINI_API_KEY:-PASTE_YOUR_GOOGLE_CLOUD_API_KEY_HERE}"
  cat > "$CFG_DIR/config.toml" <<TOML
google_api_key = "$KEYVAL"
language = "en-US"
alt_languages = ["bn-BD"]
model = "gemini-3.5-transcribe"
sample_rate = 16000
silence_threshold = 0.015
max_record_seconds = 60
auto_record_on_focus = false
TOML
  echo "!! Created $CFG_DIR/config.toml — set your Gemini API key there (or re-run with GEMINI_API_KEY=...)."
fi

echo "==> Creating launcher wrapper"
cat > "$DATA_DIR/voicetype-launch.sh" <<EOF2
#!/bin/sh
exec "$BIN" > "$DATA_DIR/launch.log" 2>&1
EOF2
chmod +x "$DATA_DIR/voicetype-launch.sh"

echo "==> Creating desktop entry + autostart"
mkdir -p "$HOME/.local/share/applications" "$HOME/.config/autostart"
cat > "$HOME/.local/share/applications/voicetype.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=VoiceType
Comment=Voice dictation for Linux Mint
Exec=$DATA_DIR/voicetype-launch.sh
Icon=audio-input-microphone
Terminal=false
Categories=Utility;Accessibility;
DESKTOP
cp "$HOME/.local/share/applications/voicetype.desktop" "$HOME/.config/autostart/voicetype.desktop"

echo
echo "==> Done."
echo "1) Edit $CFG_DIR/config.toml and paste your Google Cloud API key."
echo "2) Run: $BIN --check   (verifies microphone, AT-SPI, X11)"
echo "3) Start VoiceType from the menu or: $DATA_DIR/voicetype-launch.sh"
echo "   If it still doesn't open, send me these two files:"
echo "     $DATA_DIR/voicetype.log"
echo "     $DATA_DIR/launch.log"
" path="/home/user/playground-4/voicetype/install.sh