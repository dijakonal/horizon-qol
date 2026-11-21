//! Replay: recording the session for Horizon's Replay Studio, as ReplayMod records.
//!
//! Horizon records a session's packets as `.mcpr` (ReplayMod's format) and plays them back in
//! its Replay Studio (Options > Horizon Settings > Replay Studio), with a timeline, keyframes,
//! camera paths and a free camera. What it lacks is starting a recording other than by a chat
//! command. The Replay module starts and stops recordings (its key, or by itself on joining a
//! world or server) and shows one running. The mod asks here; the game-side tick code
//! (`src/voice/mixins/tick.rs`) records, and says here how the recording goes.

use std::sync::Mutex;

/// How the recording goes, as the game last said.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub active: bool,
    pub elapsed_ms: u64,
    pub bytes: u64,
    /// The file, under the game folder (`recordings/...mcpr`).
    pub path: String,
    /// A replay plays: what's on screen is a recording, which isn't recorded again.
    pub replaying: bool,
}

/// What became of a start or stop the mod asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub start: bool,
    pub ok: bool,
    /// The file on success, else why not.
    pub text: String,
}

/// A start (with the recording's name) or a stop (None) for the game to do.
static REQUEST: Mutex<Option<Option<String>>> = Mutex::new(None);
static STATUS: Mutex<Option<Status>> = Mutex::new(None);
static OUTCOMES: Mutex<Vec<Outcome>> = Mutex::new(Vec::new());

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Asks the game to start a recording named `name`.
pub fn start(name: &str) {
    *lock(&REQUEST) = Some(Some(name.to_owned()));
}

/// Asks the game to stop the recording, which saves it.
pub fn stop() {
    *lock(&REQUEST) = Some(None);
}

/// For the game: the start (`Some(name)`) or stop (`None`) asked for since the last tick.
pub fn take_request() -> Option<Option<String>> {
    lock(&REQUEST).take()
}

/// For the game: how the recording goes now.
pub fn set_status(active: bool, elapsed_ms: u64, bytes: u64, path: String, replaying: bool) {
    *lock(&STATUS) = Some(Status {
        active,
        elapsed_ms,
        bytes,
        path,
        replaying,
    });
}

/// For the game: what became of a start or stop.
pub fn push_outcome(start: bool, ok: bool, text: String) {
    lock(&OUTCOMES).push(Outcome { start, ok, text });
}

/// How the recording goes, as the game last said; None before it has said (no world).
pub fn status() -> Option<Status> {
    lock(&STATUS).clone()
}

/// Forgets the status: the world it was about is gone.
pub fn clear_status() {
    *lock(&STATUS) = None;
}

/// What became of the starts and stops since the last call.
pub fn take_outcomes() -> Vec<Outcome> {
    std::mem::take(&mut *lock(&OUTCOMES))
}

/// A recording's name: the world's or the server's, as letters, digits, `-` and `_`.
pub fn recording_name(place: &str) -> String {
    let name: String = place
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let name = name.trim_matches('_');
    if name.is_empty() {
        "recording".to_owned()
    } else {
        name.to_owned()
    }
}

/// A duration as the HUD shows it: `1:05`, or `1:02:05` past an hour.
pub fn clock(ms: u64) -> String {
    let s = ms / 1000;
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// A size as the status says it: `512 KB`, `4.2 MB`.
pub fn size(bytes: u64) -> String {
    if bytes < 1024 * 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// When the module records by itself: its Auto-record setting's options.
pub const AUTO: &[&str] = &["Off", "Servers", "Single player", "Everywhere"];

/// Whether Auto-record option `auto` records on a server (`server`) or in a world.
pub fn auto_records(auto: usize, server: bool) -> bool {
    match auto {
        1 => server,
        2 => !server,
        3 => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_times_and_when_it_records() {
        assert_eq!(
            recording_name("play.example.net:25565"),
            "play_example_net_25565"
        );
        assert_eq!(recording_name("My World!"), "My_World");
        assert_eq!(recording_name("../"), "recording");
        assert_eq!(clock(65_000), "1:05");
        assert_eq!(clock(3_725_000), "1:02:05");
        assert_eq!(size(4_404_019), "4.2 MB");
        assert_eq!(size(524_288), "512 KB");
        assert!(auto_records(3, true) && auto_records(3, false));
        assert!(auto_records(1, true) && !auto_records(1, false));
        assert!(!auto_records(2, true) && auto_records(2, false));
        assert!(!auto_records(0, true));
    }
}
