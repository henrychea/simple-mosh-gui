//! Detection and process handling for `mosh-server`.
//!
//! `mosh-server new` prints its handshake, detaches from the terminal and keeps
//! serving the session in the background, so a spawned child exits almost
//! immediately. The session is therefore tracked by the pid printed in
//! `[mosh-server detached, pid = N]`.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

/// A `mosh-server` binary found on this machine.
#[derive(Debug, Clone)]
pub struct MoshServer {
    pub path: PathBuf,
    /// e.g. `Some("1.4.0")`, read from the sibling `mosh --version`.
    pub version: Option<String>,
    /// `Some(err)` when the binary exists but cannot be executed — typically
    /// the Android app sandbox blocking Termux's copy.
    pub exec_error: Option<String>,
}

/// Every location we look for `mosh-server`, most likely first.
pub fn candidates() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin")); // Homebrew, Apple silicon
    dirs.push(PathBuf::from("/usr/local/bin")); // Homebrew, Intel / manual installs
    dirs.push(PathBuf::from("/opt/local/bin")); // MacPorts
    #[cfg(target_os = "android")]
    {
        dirs.push(PathBuf::from("/data/data/com.termux/files/usr/bin")); // Termux
        dirs.push(PathBuf::from("/data/local/tmp"));
    }
    dirs.into_iter()
        .map(|dir| dir.join("mosh-server"))
        .collect()
}

/// The first executable `mosh-server` in [`candidates`].
pub fn find() -> Option<MoshServer> {
    for path in candidates() {
        if !is_executable(&path) {
            continue;
        }
        // `--help` prints the usage and exits without starting a session.
        let exec_error = match run_capture(&path, &["--help"]) {
            Ok(_) => None,
            Err(err) => Some(err.to_string()),
        };
        let version = if exec_error.is_none() {
            version_of(&path)
        } else {
            None
        };
        return Some(MoshServer {
            path,
            version,
            exec_error,
        });
    }
    None
}

/// Parse a `MOSH CONNECT <port> <key>` line.
pub fn parse_connect(line: &str) -> Option<(u16, String)> {
    let mut words = line.split_whitespace();
    if words.next()? != "MOSH" || words.next()? != "CONNECT" {
        return None;
    }
    let port = words.next()?.parse::<u16>().ok()?;
    let key = words.next()?;
    if key.is_empty() {
        return None;
    }
    Some((port, key.to_string()))
}

/// Parse a `[mosh-server detached, pid = 348]` line.
pub fn parse_detached_pid(line: &str) -> Option<u32> {
    let rest = line.trim().strip_prefix("[mosh-server detached, pid = ")?;
    rest.strip_suffix(']')?.trim().parse().ok()
}

/// Options for a new `mosh-server new` process.
#[derive(Debug, Clone, Default)]
pub struct StartOptions {
    pub colors_256: bool,
    pub port: Option<u16>,
}

/// A started `mosh-server`, detached or not.
pub struct Session {
    child: Option<Child>,
    spawned: Instant,
    /// pid printed in `[mosh-server detached, pid = N]`.
    pub detached_pid: Option<u32>,
    pub port: Option<u16>,
    pub key: Option<String>,
    rx: Receiver<String>,
}

impl Session {
    pub fn start(server: &Path, opts: &StartOptions) -> std::io::Result<Session> {
        let mut cmd = Command::new(server);
        cmd.arg("new");
        if opts.colors_256 {
            cmd.args(["-c", "256"]);
        }
        if let Some(port) = opts.port {
            cmd.arg("-p").arg(port.to_string());
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // A GUI app can start with a bare environment; give mosh-server what it needs.
        if std::env::var_os("SHELL").is_none() {
            cmd.env(
                "SHELL",
                if cfg!(target_os = "macos") {
                    "/bin/zsh"
                } else {
                    "/bin/sh"
                },
            );
        }
        if std::env::var_os("PATH").is_none() {
            cmd.env("PATH", default_path());
        }
        if !locale_has_utf8() {
            // mosh-server refuses to start without a UTF-8 locale (common when
            // a GUI app inherits a bare environment); pass the same kind of
            // fallback the mosh client uses when starting a server over ssh.
            cmd.args(["-l", "LANG=en_US.UTF-8"]);
        }

        let mut child = cmd.spawn()?;
        let (tx, rx) = mpsc::channel();
        if let Some(stdout) = child.stdout.take() {
            pump(stdout, tx.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            pump(stderr, tx);
        }
        Ok(Session {
            child: Some(child),
            spawned: Instant::now(),
            detached_pid: None,
            port: None,
            key: None,
            rx,
        })
    }

    /// Output lines received since the last call, updating port/key/pid.
    pub fn drain(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        while let Ok(line) = self.rx.try_recv() {
            if let Some((port, key)) = parse_connect(&line) {
                self.port = Some(port);
                self.key = Some(key);
            }
            if let Some(pid) = parse_detached_pid(&line) {
                self.detached_pid = Some(pid);
            }
            lines.push(line);
        }
        lines
    }

    /// Whether the server process (detached or not) is still alive.
    pub fn is_running(&mut self) -> bool {
        if let Some(pid) = self.detached_pid {
            return process_alive(pid);
        }
        let spawned = self.spawned;
        match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(None) => true,
                // The child exited; its handshake and detach line can still be
                // in flight, so wait briefly for the pump before calling it dead.
                _ => spawned.elapsed() < Duration::from_secs(2),
            },
            None => false,
        }
    }

    /// Terminate the session.
    pub fn stop(&mut self) {
        if let Some(pid) = self.detached_pid {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn version_of(server: &Path) -> Option<String> {
    let mosh = server.with_file_name("mosh");
    if !is_executable(&mosh) {
        return None;
    }
    parse_version(&run_capture(&mosh, &["--version"]).ok()?)
}

/// Pull `1.4.0` out of `mosh 1.4.0 [build mosh 1.4.0]`.
fn parse_version(text: &str) -> Option<String> {
    let mut words = text.split_whitespace();
    while let Some(word) = words.next() {
        if word == "mosh"
            && let Some(candidate) = words.next()
        {
            let version = candidate.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
            if version.starts_with(|c: char| c.is_ascii_digit()) && version.contains('.') {
                return Some(version.to_string());
            }
        }
    }
    None
}

fn run_capture(program: &Path, args: &[&str]) -> std::io::Result<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(text)
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn pump(reader: impl Read + Send + 'static, tx: Sender<String>) {
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// `true` if `value` names a UTF-8 charset (`en_US.UTF-8`, `utf8`, ...).
fn charset_is_utf8(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("utf-8") || value.contains("utf8")
}

/// Whether the inherited environment has a UTF-8 charset, using the POSIX
/// precedence `LC_ALL` > `LC_CTYPE` > `LANG`.
fn locale_has_utf8() -> bool {
    let value = ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|value| !value.is_empty()));
    value.map(|value| charset_is_utf8(&value)).unwrap_or(false)
}

fn default_path() -> &'static str {
    if cfg!(target_os = "macos") {
        "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    } else if cfg!(target_os = "android") {
        "/system/bin:/system/xbin:/data/data/com.termux/files/usr/bin"
    } else {
        "/usr/local/bin:/usr/bin:/bin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connect_line() {
        assert_eq!(
            parse_connect("MOSH CONNECT 60001 pq9E/jtVmonVTkk/ZQqd4A"),
            Some((60001, "pq9E/jtVmonVTkk/ZQqd4A".to_string()))
        );
        assert_eq!(
            parse_connect("MOSH CONNECT 60000 jYDEgw6VbIivDU3ojJAR6A"),
            Some((60000, "jYDEgw6VbIivDU3ojJAR6A".to_string()))
        );
    }

    #[test]
    fn rejects_non_connect_lines() {
        for line in [
            "mosh-server (mosh 1.4.0) [build mosh 1.4.0]",
            "[mosh-server detached, pid = 348]",
            "MOSH CONNECT abc key",
            "MOSH CONNECT 60001",
            "hello MOSH CONNECT 60001 key",
            "",
        ] {
            assert_eq!(parse_connect(line), None, "{line:?}");
        }
    }

    #[test]
    fn parses_detached_pid() {
        assert_eq!(
            parse_detached_pid("[mosh-server detached, pid = 348]"),
            Some(348)
        );
        assert_eq!(
            parse_detached_pid("[mosh-server detached, pid = 20443]"),
            Some(20443)
        );
        assert_eq!(parse_detached_pid("MOSH CONNECT 60001 key"), None);
        assert_eq!(parse_detached_pid("[mosh-server detached, pid = ]"), None);
    }

    #[test]
    fn parses_version() {
        assert_eq!(
            parse_version("mosh 1.4.0 [build mosh 1.4.0]").as_deref(),
            Some("1.4.0")
        );
        assert_eq!(
            parse_version("Usage: mosh-server new [-s] [-v]").as_deref(),
            None
        );
    }

    #[test]
    fn detects_utf8_charsets() {
        assert!(charset_is_utf8("en_US.UTF-8"));
        assert!(charset_is_utf8("en_US.utf8"));
        assert!(!charset_is_utf8("C"));
        assert!(!charset_is_utf8(""));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn candidates_include_homebrew() {
        assert!(candidates().contains(&PathBuf::from("/opt/homebrew/bin/mosh-server")));
    }
}
