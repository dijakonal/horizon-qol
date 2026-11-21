//! What Horizon QoL adds to the voice engine: a snapshot of its state (for the mod's sounds and
//! its Voice Chat page), reading and changing its settings from the mod's own menu, and opening
//! the voice menu on one of its pages.

use super::voice::voice;
use super::{Settings, ui};
use crate::voice::net::Status;
use std::sync::atomic::Ordering;

/// Where the voice connection is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    /// Not in a server's world.
    Away,
    /// Asking the server whether it has voice chat.
    Asking,
    /// The server has no voice chat.
    NoVoice,
    Connecting,
    Connected,
    Failed(String),
}

/// The voice chat as the mod's sounds and pages read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub link: Link,
    /// The status line the voice menu shows, and its colour (0xRRGGBB).
    pub status: (String, u32),
    pub muted: bool,
    pub disabled: bool,
    pub transmitting: bool,
    pub whisper: bool,
    /// The microphone's level in dB (-80 is silence), and the activation threshold.
    pub level_db: f32,
    pub threshold_db: f32,
    /// The group the player is in, and how many players are in it with them.
    pub group: Option<String>,
    pub group_members: usize,
}

pub fn snapshot() -> Snapshot {
    let v = voice();
    let status = v.status_line();
    let link = match v.session.as_ref() {
        None => Link::Away,
        Some(s) => match &s.secret {
            None if s.asked.len() >= s.plan(v.settings.protocol).len() => Link::NoVoice,
            None => Link::Asking,
            Some(_) => match &*v.shared.status.lock().unwrap() {
                Status::Connected => Link::Connected,
                Status::Failed(e) => Link::Failed(e.clone()),
                _ => Link::Connecting,
            },
        },
    };
    let (group, group_members) = v
        .session
        .as_ref()
        .and_then(|s| {
            let id = s.group?;
            let name = s
                .groups
                .get(&id)
                .map_or_else(String::new, |g| g.name.clone());
            let members = s
                .states
                .values()
                .filter(|p| p.group == Some(id) && !p.disconnected)
                .count();
            Some((Some(name), members))
        })
        .unwrap_or((None, 0));
    Snapshot {
        link,
        status,
        muted: v.settings.muted,
        disabled: v.settings.disabled,
        transmitting: v.shared.transmitting.load(Ordering::Relaxed),
        whisper: v.shared.controls.lock().unwrap().whisper,
        level_db: v.shared.level(),
        threshold_db: v.settings.threshold as f32,
        group,
        group_members,
    }
}

/// The voice settings as they are now.
pub fn settings() -> Settings {
    voice().settings.clone()
}

/// Changes the voice settings as the voice menu would: they are stored, applied to the voice
/// threads, the audio restarts when the devices or the off switch changed, and the server is
/// told when mute or off changed.
pub fn update(f: impl FnOnce(&mut Settings)) {
    let mut v = voice();
    let before = v.settings.clone();
    f(&mut v.settings);
    if v.settings == before {
        return;
    }
    v.save();
    let audio = |s: &Settings| (s.disabled, s.input_device.clone(), s.output_device.clone());
    if audio(&before) != audio(&v.settings) {
        v.restart_audio();
    }
    if (before.muted, before.disabled) != (v.settings.muted, v.settings.disabled) {
        v.send_state();
    }
}

/// Opens the voice menu on `page` (its groups, player volumes or audio settings).
pub fn menu_open_at(page: ui::Screen) {
    let mut v = voice();
    v.menu.open(None);
    v.menu.screen = page;
}
