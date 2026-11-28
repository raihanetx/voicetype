# VoiceType

Voice-to-text for **Linux Mint**. Start it yourself with a **"start now"**
signal; when it's on, clicking any text field in any app shows a mic on its
right side; speak; speech is transcribed with **Gemini `gemini-3.5-transcribe`**
and inserted at the cursor. When it's off, nothing happens. **No auto-start at
login.** See `ARCHITECTURE.md` for the concept.

## Build (no local toolchain needed)

Source is built on **GitHub Actions**. Push `main`; CI runs `cargo build` +
`cargo test`, then uploads a native `voicetype` binary artifact
(`target/release/voicetype`). Download it from the Actions run. This repo does
not require Rust installed locally.

Local build, if you do have Rust:

```sh
cargo build --release
cargo test
```

## Run

```sh
voicetype            # skeleton
voicetype start-now  # (IPC planned; sends "start now" to a running instance)
voicetype stop
```

## Secrets (never committed)

- **Gemini API key** — local secure storage / `GEMINI_API_KEY`, read-only
  validated; never printed. Speech-to-text only.
- **GitHub token** — per-command push/CI only; never stored in the repo.

## Status

Early skeleton. Synchronized core is built and unit-tested on CI: state
machine + ordered event queue, start-now IPC, AT-SPI boundary + focus
classifier, mic overlay placement, audio core, Gemini transcription boundary,
synchronized insertion, pipeline, on/off indicator, secrets. **Live backends
(real AT-SPI, native audio, Gemini HTTP, transparent overlay, tray, packaging)
and real Linux Mint verification are not done yet.**

`prototype/` holds the original static UI prototype (reference only).
