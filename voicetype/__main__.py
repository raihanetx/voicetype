"""Entry point for VoiceType.

Skeleton only: start-now signal, AT-SPI detection, mic overlay, audio capture,
Gemini transcription, and text insertion are not implemented yet. See
ARCHITECTURE.md and the build plan.
"""

from __future__ import annotations

from voicetype import __version__


def main() -> int:
    print(f"VoiceType {__version__} (skeleton — not implemented yet)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
