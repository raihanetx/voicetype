# VoiceType

Voice dictation for Linux Mint. Click into any text field, a mic icon appears;
click it, speak, and your speech is transcribed (Google Cloud Speech-to-Text,
English + Bangla) and inserted into the field.

## Build (on Linux Mint)

```bash
sudo apt install -y build-essential pkg-config libgtk-3-dev \
  libatspi2.0-dev libasound2-dev libx11-dev curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
cargo build --release
```

Binary: `target/release/voicetype`

## Configure

Create `~/.config/voicetype/config.toml`:

```toml
google_api_key = "YOUR_GOOGLE_CLOUD_API_KEY"
language = "en-US"
alt_languages = ["bn-BD"]
model = "latest_long"
sample_rate = 16000
silence_threshold = 0.015
max_record_seconds = 60
```

Enable accessibility (AT-SPI) in Mint: Settings -> Accessibility -> enable
"Assistive technologies" / screen reader support. The app also calls the
session accessibility setter at startup.

## Run

```bash
./target/release/voicetype
```

## Troubleshooting (for non-technical users)

Run the built-in self-check:

```bash
./target/release/voicetype --check
```

It prints whether your API key, microphone, AT-SPI and X11 are working.

If something crashes or doesn't work, send me the log file:

```bash
cat ~/.local/share/voicetype/voicetype.log
```

That file contains the exact error messages so I can diagnose without you
explaining technical details.

Keep it running in the background (add to Startup Applications).

## Notes

- Targets X11 (Cinnamon default). Wayland not supported for the overlay.
- Requires `xdotool` for fallback text injection into apps that don't
  implement the AT-SPI EditableText interface.
- "Google 3.5" is interpreted as Google Cloud Speech-to-Text v1 REST API
  (`speech.googleapis.com/v1/speech:recognize`) with `latest_long` model.
