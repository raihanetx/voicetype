# VoiceType — Architecture & Project Plan

> Living architecture document. Captures the product concept, how focus
> detection works, the detection/transcription pipeline, the app window and UI
> prototype, the codebase map, what is done, and what is still open — so
> context is never lost between sessions.

Last updated: 2026-11-28

---

## 1. The product in one paragraph

VoiceType is a **voice-to-text (voice typing) tool for Linux Mint**. It runs
quietly in the background. When the user clicks into **any text field, in any
app** (browser, coding agent, terminal, text editor, chat), a **microphone icon
appears on the right side of that field**. The user clicks the mic, speaks, and
the speech is transcribed with **Gemini `gemini-3.5-transcribe`** and
**auto-pasted into the field** they were in.

The purpose: the user dislikes typing long prompts. Speaking is faster. So
VoiceType turns speech into typed text wherever the cursor is.

It also has a small **app window** (opened from the app menu) for setup and
diagnostics — see §6.

---

## 2. Core interaction (the flow the user wants)

```
1. User clicks into a text field in ANY app.
2. Mic icon appears automatically on the RIGHT side of that field.
   (Reason: user can't/won't type. Speaking replaces typing.)
3. User clicks the mic.
4. User speaks.
5. Audio -> Gemini gemini-3.5-transcribe -> text.
6. Text is inserted into the same field, at the cursor.
7. User leaves it running in the background.
```

Mic visibility rule (decided): **appears on click/focus only**, disappears when
the field loses focus. Not on hover.

Mic look (decided): custom-drawn microphone glyph, **no background circle**;
right side of the field, vertically centred; faint hover halo only.

---

## 3. How the software "knows" the user clicked a field (key concept)

VoiceType is **always running in the background** (started at login). It is not
"closed until you open the window." It has no visible window by default; it is
a **listener**.

Mechanism: **AT-SPI**, the Linux accessibility bus.

1. Mint runs a system-wide accessibility bus (`org.a11y.Bus`). Every app
   announces state changes to it — including *"a text field just got focus."*
2. VoiceType subscribes to those announcements
   (`register_event::<StateChangedEvent>()`).
3. On a `StateChanged` event with `state == Focused`, it:
   - reads the widget's **role** (Entry, Text, Terminal, etc.),
   - asks the widget **where it is on screen** via the AT-SPI **Component**
     interface (`get_extents`),
   - draws the mic at the field's right edge.
4. When the field is unfocused, it hides the mic.

So detection does **not** depend on VoiceType's own window being open. The
window (for setup/logs) is separate from detection.

**Honest limitation:** some apps (certain browsers, Electron apps) do not fully
announce their text fields to AT-SPI. In those apps the mic may not appear.
This is the app not reporting, not a bug in VoiceType.

---

## 4. Detection pipeline (technical)

Two independent detectors run in parallel, both feeding the same show/hide
logic (so the mic appears even if one path fails):

**A. Event path** — `atspi_watch::run_watcher()`:
```
AT-SPI StateChanged:Focused
      │
      ├─ role is text-like?  (Entry / PasswordText / Text / Terminal /
      │                       Paragraph / DocumentText / DocumentFrame)
      ├─ get_extents(Screen) -> (x, y, w, h)
      └─ overlay.show_mic(x + w + 4, y, h)
```

**B. Polling fallback** — `atspi_watch::run_poller()` (every 700 ms):
```
desktop root -> children (apps)
      │ for each app: depth/node-bounded DFS (depth<=12, <=400 nodes)
      ├─ node state contains Focused AND role is text-like?
      │     -> get_extents -> overlay.show_mic(...)
      └─ none focused -> overlay.hide_mic()
```

At startup the app also calls `set_session_accessibility(true)` to enable the
accessibility bridge (without it, many apps emit no focus events).

**After the mic shows** — user clicks it (`overlay.rs` sends `Btn::Mic`), then
`main.rs start_pipeline()`:
```
Btn::Mic
      │
      ├─ audio::Recorder::record() -> temp WAV
      │     (cpal; mono; auto-stop on ~2s silence or max_record_seconds)
      ├─ stt::transcribe() -> text  (Gemini Interactions API)
      └─ inject::inject_text() into the focused ObjectRef
            ├─ try AT-SPI EditableText.insert_text at caret
            └─ fallback: xdotool type  (X11)
```

---

## 5. Transcription (Gemini)

- Endpoint: `POST https://generativelanguage.googleapis.com/v1beta/interactions`
- Auth: header `x-goog-api-key: <key>` (NOT `?key=`).
- Model: `gemini-3.5-transcribe`.
- Audio: inline base64 WAV, `mime_type: audio/wav`.
- Config: `generation_config.transcription_config` with
  `language_codes` (primary + alternatives) and `mode: verbatim`.
- Response text: concatenated `steps[].content[]` blocks where `type == text`.
- Key validation: `stt::validate_key()` sends a tiny text interaction; used at
  startup and in `--check`.

Note: earlier versions used Google Cloud Speech-to-Text
(`speech:recognize`, model `latest_long`). That is **removed**. Old configs
with legacy model names are auto-migrated to `gemini-3.5-transcribe` on load.

---

## 6. The app window (UI) — prototype defined, not yet built in Rust

The Rust app currently has **no window**. The intended window is designed in the
HTML prototype (§7) and has three states:

### 6.1 First-run / Setup
- Opens on first launch (or when no API key is set).
- Asks for the **Gemini API key** (link to aistudio.google.com/apikey).
- "Validate & continue" -> validates the key -> advances.

### 6.2 Setup done + Demo
- Confirms "Setup done".
- Shows an **example input field** with the mic on its right; clicking the
  field reveals the mic (focus rule), clicking the mic runs the
  record -> transcribe -> insert demo so the user sees how it works.

### 6.3 Ready (running)
- Confirms the app is running.
- **All logs** block + **Error logs** block, each with its **own Copy button**.
  (Deliberately no typical status rows/tables — just logs.)
- A **debug (gear) icon** in the header/corner opens the same session report.

### 6.4 Visual direction
- **Linux Mint / Cinnamon feel**: dark Mint-Y window (`#363636`), Mint green
  accent (`#87b158`), Cantarell-ish font, soft graded backdrop.
- Centered window.
- No browser "traffic light" dots.
- Mic glyph has no background.

---

## 7. UI prototype

Location: `prototype/index.html`
Tech: **HTML + Tailwind CSS (Play CDN) + vanilla JS**. No `<style>` block, no
hand-written CSS.

How to view:

```bash
cd prototype
python3 -m http.server 8099
# open http://localhost:8099/index.html
```

It demonstrates the 3 states above with working interactions:
- step 1: key validation,
- step 2: focus the field -> mic fades in on the right -> click -> red dot
  (recording) -> yellow (transcribing) -> text inserted,
- step 3: All logs / Error logs with Copy, and a debug modal.

The prototype is the **design source of truth** for the real window. When the
Rust window is built, it should match this.

---

## 8. Codebase map (Rust)

| File | Responsibility |
|---|---|
| `src/main.rs` | Startup, logging, first-run key setup + validation, `--check`, the event loop (`start_pipeline`, mic vs debug click routing), `show_report`, `notify_error`, `notify_ok`, `log_path`. |
| `src/atspi_watch.rs` | Two detectors: `run_watcher` (AT-SPI `StateChanged:Focused` events) and `run_poller` (700ms tree-walk fallback). Enables the a11y bridge, decides text-like roles, reads extents, shows/hides the mic, records window id via `xdotool`. Logs every focus event (diagnostics). |
| `src/overlay.rs` | X11 drawing of two override-redirect windows: the **mic** (custom-drawn, states Idle/Recording/Processing/Error) and a persistent **debug gear**. Sends `Btn::Mic` / `Btn::Debug` on click. |
| `src/audio.rs` | `Recorder` using cpal: mono capture -> 16-bit WAV; RMS-based auto-stop after ~2s silence. |
| `src/stt.rs` | Gemini `transcribe()` and `validate_key()` via the Interactions API. |
| `src/inject.rs` | Inserts text: AT-SPI `EditableText.insert_text` at caret, fallback `xdotool type`. |
| `src/config.rs` | Config at `~/.config/voicetype/config.toml`; `serde(default)`; logs parse errors; migrates legacy model names. |
| `src/session_log.rs` | In-memory ring buffer of session events (for the debug report), mirrored to the file log. |

### Config (`~/.config/voicetype/config.toml`)

```toml
google_api_key = "..."          # Gemini key
language = "en-US"              # primary BCP-47
alt_languages = ["bn-BD"]       # code-switching
model = "gemini-3.5-transcribe"
sample_rate = 16000
silence_threshold = 0.015       # RMS threshold
max_record_seconds = 60
auto_record_on_focus = false
```

### Logs
- File: `~/.local/share/voicetype/voicetype.log`
- Launcher stdout/stderr: `~/.local/share/voicetype/launch.log` (via wrapper
  script created by `install.sh`).

### CLI
- `voicetype` — run.
- `voicetype --check` — diagnostics (key live-test, mic, AT-SPI, X11, model).

---

## 9. Build & release

- CI: `.github/workflows/build.yml` (GitHub Actions) — builds debug + release +
  `.deb`, uploads artifacts.
- Installer: `install.sh` — apt deps, cargo build, config template (optional
  `GEMINI_API_KEY=` env), desktop entry + autostart via a launcher wrapper.
- Releases: published from CI artifacts (binary + `.deb`).

---

## 10. Current status (as of this doc)

**Works / verified**
- Gemini transcription code + key validation (a real key validated at runtime).
- Config auto-migration from legacy `latest_long` -> `gemini-3.5-transcribe`.
- CI builds green; binary runs; `--check` reports model + live key test.
- App connects to the accessibility bus ("connected to accessibility bus").
- Overlay draws the debug gear.
- **v0.3.0-detect (latest):** enables the session accessibility bridge at
  startup AND adds a polling fallback that walks the accessibility tree every
  700 ms for the focused text field — so the mic can appear even when an app
  emits no focus events. Compiled + CI green; binary contains the new code.
  NOT yet verified on a real Mint desktop.

**OPEN — the current blocker**
- **The mic was not appearing when clicking a text field.** Root cause
  suspected: accessibility bridge was never enabled. v0.3.0-detect fixes that
  and adds polling. Awaiting a real-desktop test:
  `voicetype --check` should show `AT-SPI bridge enabled: true`; the log
  should show `accessibility bridge enabled` and/or `poller found focused`.

**Not built yet**
- The Mint **setup / status / logs window** in Rust (§6). Exists only as the
  HTML prototype.

---

## 11. Roadmap / next steps (in order)

1. **Confirm focus detection on the real desktop** (blocker). Install
   `v0.3.0-detect`, run `--check`, click a field, read the log:
   - `accessibility bridge enabled` + `poller found focused ...` -> working.
   - `focus event: role=Entry enabled=1` but no `mic shown` -> geometry bug.
   - `poller: no focused text field` while a field IS focused -> tree walk
     not reaching it (depth/node cap or app not exposed); adjust.
   - `AT-SPI bridge enabled: false` -> enabling failed; investigate session.
2. **Build the app window in Rust** to match `prototype/index.html`:
   setup -> validate key -> demo -> ready; All/Error logs with Copy; debug
   gear. (X11 drawing or a light GTK/webview — decide when building; must stay
   light and Mint-native.)
3. **End-to-end proof**: speak -> text appears in a real app field.
4. Polish: injection reliability across apps, Wayland note, packaging.

---

## 12. Decisions log (so we don't relitigate)

- Mic is at the **right edge** of the field, not literally *inside* another
  app's text box (Linux cannot draw inside third-party widgets).
- Mic shows **on focus/click only**, not hover.
- Mic glyph has **no background**.
- Status area is **logs only** (All logs + Error logs, each with Copy) — no
  status tables/rows.
- Look & feel: **Linux Mint / Cinnamon**, dark Mint-Y, Mint green; centered.
- Transcription: **Gemini `gemini-3.5-transcribe`** only.
- Debug icon shows the **full session report**, copyable.
- First run: ask key -> "setup done" -> show demo -> ready.
