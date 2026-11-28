//! "Safe step": pure, tested building blocks for Gemini transcription.
//! NO network here. No guessing a live call — these pieces are verifiable:
//! - PCM16 -> WAV bytes
//! - base64 (no external crate)
//! - transcription request JSON builder
//! - transcript response parser
//! Model `gemini-3.5-transcribe`, method `generateContent`. Exact live schema
//! still to be verified with a real call on a real machine before wiring HTTP.

/// Build a 16-bit PCM mono/multi-channel WAV from raw little-endian PCM16 samples.
pub fn pcm16_to_wav(pcm: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let block_align: u16 = channels * bits_per_sample / 8;
    let byte_rate: u32 = sample_rate * block_align as u32;
    let data_len: u32 = pcm.len() as u32;
    let riff_len: u32 = 36 + data_len;

    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);
    out
}

const B64_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 encode (no external crate).
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(B64_ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { B64_ALPHABET[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64_ALPHABET[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Build `generateContent` request JSON for a WAV audio clip.
/// NOTE: exact live field names to be verified against Gemini before HTTP.
pub fn build_transcribe_request(wav: &[u8]) -> String {
    let data = base64_encode(wav);
    format!(
        "{{\"contents\":[{{\"parts\":[{{\"inlineData\":{{\"mimeType\":\"audio/wav\",\"data\":\"{}\"}}}}]}}]}}",
        json_escape(&data)
    )
}

/// Extract transcript text from a Gemini `generateContent` response.
/// Minimal scan for `"text": "..."`. Returns None if not found. (Not a full JSON parser.)
pub fn parse_transcript_response(json: &str) -> Option<String> {
    // find "text" key
    let key = "\"text\"";
    let mut idx = json.find(key)? + key.len();
    let bytes = json.as_bytes();
    // skip whitespace + colon
    while idx < bytes.len() && (bytes[idx].is_ascii_whitespace() || bytes[idx] == b':') {
        idx += 1;
    }
    if idx >= bytes.len() || bytes[idx] != b'"' {
        return None;
    }
    idx += 1;
    let mut out = String::new();
    let mut chars = json[idx..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => {
                if let Some(n) = chars.next() {
                    match n {
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        'r' => out.push('\r'),
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        other => {
                            out.push('\\');
                            out.push(other);
                        }
                    }
                }
            }
            c => out.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_44_bytes_plus_data() {
        let pcm = vec![0u8; 100];
        let wav = pcm16_to_wav(&pcm, 16_000, 1);
        assert_eq!(wav.len(), 44 + 100);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        // sizes
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 36 + 100);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 100);
        // sample rate
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        // PCM format
        assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()), 1);
    }

    #[test]
    fn base64_encode_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn request_json_is_well_formed_shape() {
        let wav = pcm16_to_wav(&[0, 0, 0, 0], 16_000, 1);
        let req = build_transcribe_request(&wav);
        assert!(req.contains("\"contents\""));
        assert!(req.contains("\"inlineData\""));
        assert!(req.contains("\"mimeType\":\"audio/wav\""));
        assert!(req.contains("\"data\":\""));
    }

    #[test]
    fn parse_extracts_text() {
        let json = r#"{"candidates":[{"content":{"parts":[{"text":"hello world"}]}}]}"#;
        assert_eq!(parse_transcript_response(json), Some("hello world".to_string()));
    }

    #[test]
    fn parse_handles_escapes_and_missing() {
        assert_eq!(parse_transcript_response(r#"{"text":"a\nb\"c"}"#), Some("a\nb\"c".to_string()));
        assert_eq!(parse_transcript_response("{}"), None);
        assert_eq!(parse_transcript_response("nope"), None);
    }
}
