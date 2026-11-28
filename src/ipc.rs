//! Task 4: "start now" signal to a running instance.
//!
//! Minimal local IPC over a Unix domain socket. No external crates.
//! - `voicetype start-now` -> tell the running instance to start.
//! - `voicetype stop`      -> tell the running instance to stop.
//!
//! Separate from starting recording (that's a later task); re-checking what is
//! currently focused on `start-now` happens with AT-SPI (Task 5). Nothing here
//! auto-starts at login.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

/// Default socket file name inside the runtime directory.
pub const SOCKET_NAME: &str = "voicetype.sock";

/// Commands sent over the socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// "Start now."
    StartNow,
    /// "Stop VoiceType."
    Stop,
}

impl Command {
    pub fn as_str(self) -> &'static str {
        match self {
            Command::StartNow => "start-now",
            Command::Stop => "stop",
        }
    }

    pub fn parse(s: &str) -> Option<Command> {
        match s.trim() {
            "start-now" | "start" => Some(Command::StartNow),
            "stop" => Some(Command::Stop),
            _ => None,
        }
    }
}

/// Socket path inside a runtime directory.
pub fn socket_path(dir: &Path) -> PathBuf {
    dir.join(SOCKET_NAME)
}

/// Send a command to a running instance listening on `path`.
pub fn send_command(path: &Path, cmd: Command) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(path)?;
    stream.write_all(cmd.as_str().as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(()) // stream closes on drop
}

/// Read one command from a connected stream (until EOF).
pub fn recv_command(stream: &mut UnixStream) -> std::io::Result<Option<Command>> {
    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    Ok(Command::parse(&buf))
}

/// Bind a listener at `path`, removing a stale socket file first.
pub fn bind_listener(path: &Path) -> std::io::Result<UnixListener> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    UnixListener::bind(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn parse_commands() {
        assert_eq!(Command::parse("start-now"), Some(Command::StartNow));
        assert_eq!(Command::parse("start"), Some(Command::StartNow));
        assert_eq!(Command::parse("  start-now\n"), Some(Command::StartNow));
        assert_eq!(Command::parse("stop"), Some(Command::Stop));
        assert_eq!(Command::parse("nope"), None);
        assert_eq!(Command::parse(""), None);
    }

    #[test]
    fn as_str_roundtrip() {
        for c in [Command::StartNow, Command::Stop] {
            assert_eq!(Command::parse(c.as_str()), Some(c));
        }
    }

    #[test]
    fn socket_path_joins_name() {
        let p = socket_path(Path::new("/run/user/1000"));
        assert_eq!(p, PathBuf::from("/run/user/1000/voicetype.sock"));
    }

    #[test]
    fn unix_socket_roundtrip() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("voicetype-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = socket_path(&dir);

        let listener = bind_listener(&path).unwrap();
        let server = thread::spawn(move || {
            let (mut conn, _addr) = listener.accept().unwrap();
            recv_command(&mut conn).unwrap()
        });

        send_command(&path, Command::StartNow).unwrap();
        let got = server.join().unwrap();
        assert_eq!(got, Some(Command::StartNow));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
