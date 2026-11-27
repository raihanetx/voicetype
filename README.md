# VoiceType

Voice dictation for Linux Mint. Click into any text field, a microphone icon
appears at the field's edge; click it, speak, and your speech is transcribed
(Gemini `gemini-3.5-transcribe`, English + Bangla) and inserted into the field.

A small debug (gear) icon stays in the top-right corner: click it to see a full
report of everything that happened this session — so problems can be shared.

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

On first run it asks for your Gemini API key (from
https://aistudio.google.com/apikey). It is stored in
`~/.config/voicetype/config.toml`:

```toml
google_api_key = "YOUR_GEMINI_API_KEY"
language = "en-US"
alt_languages = ["bn-BD"]
model = "gemini-3.5-transcribe"
sample_rate = 16000
silence_threshold = 0.015
max_record_seconds = 60
```

Enable accessibility (AT-SPI) in Mint: Settings -> Accessibility -> enable
"Assistive technologies" / screen reader support.

## Run

```bash
./target/release/voicetype
```

On success you get a "VoiceType setup done" notification. Then focus any text
field, click the mic icon, speak, and click the mic again (or just stop — it
auto-stops after ~2s of silence).

## Troubleshooting

```bash
./target/release/voicetype --check
```

Prints whether your API key (with a live Gemini key test), microphone, AT-SPI
and X11 are working. Or click the debug gear icon in the corner for the session
report, and read the file log:

```bash
cat ~/.local/share/voicetype/voicetype.log
```

## Notes

- Targets X11 (Cinnamon default). Wayland is not supported for the overlay.
- Requires `xdotool` for fallback text injection into apps that don't
  implement the AT-SPI EditableText interface.
- Transcription uses the Gemini Interactions API
  (`generativelanguage.googleapis.com/v1beta/interactions`) with
  `gemini-3.5-transcribe`.
