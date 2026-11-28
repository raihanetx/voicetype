# VoiceType

Voice typing for Linux Mint. Click into any text field, a microphone icon
appears on its right; click it, speak, and your speech is turned into text and
pasted into that field — using Google **Gemini `gemini-3.5-transcribe`**.

The purpose: skip typing (including long prompts to coding agents) by speaking
instead.

---

## Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — the full architecture and plan:
  concept, how focus detection works, the detection/transcription pipeline,
  the app window + UI prototype, codebase map, current status, roadmap, and
  the decisions log. **Read this first.**
- **[prototype/index.html](prototype/index.html)** — the UI prototype
  (HTML + Tailwind + JS). Open it in a browser, or:

  ```bash
  cd prototype && python3 -m http.server 8099
  # http://localhost:8099/index.html
  ```

---

## Quick start (build & run)

```bash
sudo apt install -y build-essential pkg-config libgtk-3-dev \
  libatspi2.0-dev libasound2-dev libx11-dev curl xdotool zenity libnotify-bin
cargo build --release

# check everything (key, mic, accessibility, display)
./target/release/voicetype --check

# run
./target/release/voicetype
```

On first run it asks for a Gemini API key (from
https://aistudio.google.com/apikey). It is stored in
`~/.config/voicetype/config.toml`.

Logs: `~/.local/share/voicetype/voicetype.log`

---

## Current status

The core works (Gemini transcription, key validation, recording, text
injection, logging). The **open blocker** is reliably making the mic appear
when a text field is focused — see
[ARCHITECTURE.md §10](ARCHITECTURE.md#10-current-status-as-of-this-doc) for the
details and the exact next step.
