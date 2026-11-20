use super::session::Session;
use super::voice;
use crate::voice::i18n::{fmt, strings};
use crate::voice::net::Status;
use crate::voice::proto::PlayerState;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// A line of the status the HUD shows: its text, colour, and an optional
/// icon sprite (`horizon_qol:gui/voice/icon_*`) drawn to its left, Simple Voice
/// Chat-style.
#[derive(Debug)]
pub struct Line {
    pub text: String,
    pub rgb: u32,
    pub icon: Option<&'static str>,
}

// The HUD status icons (drawn in assets/horizon_qol/textures/gui/voice/).
pub(super) const IC_DISCONNECTED: &str = "horizon_qol:gui/voice/icon_disconnected";
pub(super) const IC_DISABLED: &str = "horizon_qol:gui/voice/icon_speaker_off";
pub(super) const IC_MUTE: &str = "horizon_qol:gui/voice/icon_mute";
pub(super) const IC_TALKING: &str = "horizon_qol:gui/voice/icon_talking";
pub(super) const IC_WHISPERING: &str = "horizon_qol:gui/voice/icon_whispering";
pub(super) const IC_GROUP: &str = "horizon_qol:gui/voice/icon_group";
pub(super) const IC_OTHER_TALKING: &str = "horizon_qol:gui/voice/icon_other_talking";
pub(super) const IC_OTHER_WHISPERING: &str = "horizon_qol:gui/voice/icon_other_whispering";

pub(super) const GREY: u32 = 0xAAAAAA;
pub(super) const GREEN: u32 = 0x55FF55;
pub(super) const AQUA: u32 = 0x55FFFF;
pub(super) const RED: u32 = 0xFF5555;
pub(super) const YELLOW: u32 = 0xFFFF55;

/// Connection, warnings and the active voice status, top line first.
pub fn hud() -> Vec<Line> {
    let v = voice();
    let Some(session) = v.session.as_ref().filter(|_| v.settings.hud) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    let line = |text: String, rgb| Line {
        text,
        rgb,
        icon: None,
    };
    let iline = |text: String, rgb, icon| Line {
        text,
        rgb,
        icon: Some(icon),
    };
    if session.secret.is_none() {
        // No voice chat here (yet): the status alone, with the disconnected
        // icon, so the HUD stays visible while asking.
        let (text, rgb) = v.status_line();
        lines.push(iline(text, rgb, IC_DISCONNECTED));
        return lines;
    }
    let status = v.shared.status.lock().unwrap().clone();
    match status {
        Status::Connected => {}
        Status::Failed(ref e) => lines.push(iline(
            fmt(strings().voice_chat_prefix, &[e]),
            RED,
            IC_DISCONNECTED,
        )),
        _ => lines.push(iline(
            fmt(strings().voice_chat_prefix, &[strings().connecting]),
            YELLOW,
            IC_DISCONNECTED,
        )),
    }
    if let Some(e) = v.shared.mic_error.lock().unwrap().as_ref() {
        lines.push(line(fmt(strings().voice_chat_prefix, &[e]), RED));
    }
    if let Err(e) = &v.devices.lock().unwrap().input
        && e != "not open"
    {
        lines.push(line(fmt(strings().mic_error, &[e]), RED));
    }
    if let Err(e) = &v.devices.lock().unwrap().output
        && e != "not open"
    {
        lines.push(line(fmt(strings().spk_error, &[e]), RED));
    }
    if v.settings.disabled {
        lines.push(iline(strings().voice_chat_off.to_owned(), RED, IC_DISABLED));
    } else if v.settings.muted {
        lines.push(iline(strings().microphone_muted.to_owned(), RED, IC_MUTE));
    } else if v.shared.transmitting.load(Ordering::Relaxed) {
        let whisper = v.shared.controls.lock().unwrap().whisper;
        lines.push(if whisper {
            iline(strings().whispering.to_owned(), AQUA, IC_WHISPERING)
        } else {
            iline(strings().talking.to_owned(), GREEN, IC_TALKING)
        });
    }
    lines
}

/// The current group for the top-left roster header.
pub fn hud_group() -> Option<Line> {
    let v = voice();
    let session = v.session.as_ref().filter(|_| v.settings.hud)?;
    let group = session.group.and_then(|id| session.groups.get(&id))?;
    Some(Line {
        text: fmt(strings().hud_group, &[&group.name]),
        rgb: GREEN,
        icon: Some(IC_GROUP),
    })
}

/// A group member, recent speaker, or muted player shown in the HUD roster.
#[derive(Debug)]
pub struct Head {
    pub uuid: u128,
    pub name: String,
    pub talking: bool,
    pub muted: bool,
}

/// Group members stay in the roster; outside speakers remain for two seconds.
/// Activity selects whom to keep only when `limit` cannot fit everyone.
pub fn hud_heads(limit: usize) -> Vec<Head> {
    let v = voice();
    let Some(session) = v
        .session
        .as_ref()
        .filter(|s| v.settings.hud && s.secret.is_some())
    else {
        return Vec::new();
    };
    let now = Instant::now();
    let talking = v.shared.talking.lock().unwrap();
    let mut heads = Vec::new();
    for (id, player) in &session.states {
        if player.disconnected {
            continue;
        }
        let last = talking.get(id).map(|(at, _)| *at);
        let speaking = last.is_some_and(|at| now - at < Duration::from_millis(300));
        let recent = last.is_some_and(|at| now - at < Duration::from_secs(2));
        let in_group = session
            .group
            .is_some_and(|group| player.group == Some(group));
        if in_group || recent || player.muted {
            heads.push(Head {
                uuid: *id,
                name: player.name.clone(),
                talking: speaking,
                muted: player.muted,
            });
        }
    }
    let by_name = |a: &Head, b: &Head| a.name.to_lowercase().cmp(&b.name.to_lowercase());
    heads.sort_by(by_name);
    if heads.len() > limit {
        let activity = v.shared.activity.lock().unwrap();
        heads.sort_by(|a, b| {
            activity
                .get(&b.uuid)
                .map_or(0, |a| a.count)
                .cmp(&activity.get(&a.uuid).map_or(0, |a| a.count))
                .then_with(|| by_name(a, b))
        });
        heads.truncate(limit);
        heads.sort_by(by_name);
    }
    heads
}

fn player_icon(
    session: &Session,
    talking: &HashMap<u128, (Instant, bool)>,
    now: Instant,
    id: u128,
    player: &PlayerState,
) -> Option<(&'static str, u32)> {
    if player.disconnected {
        Some((IC_DISCONNECTED, RED))
    } else if player.muted {
        Some((IC_MUTE, RED))
    } else if player.disabled {
        Some((IC_DISABLED, RED))
    } else if let Some(group) = player.group {
        session
            .groups
            .get(&group)
            .filter(|g| !g.hidden || session.group == Some(group))?;
        Some((
            IC_GROUP,
            if session.group == Some(group) {
                GREEN
            } else {
                YELLOW
            },
        ))
    } else {
        let (_, whispering) = talking
            .get(&id)
            .filter(|(at, _)| now - *at < Duration::from_secs(2))?;
        Some((
            if *whispering {
                IC_OTHER_WHISPERING
            } else {
                IC_OTHER_TALKING
            },
            if *whispering { AQUA } else { GREEN },
        ))
    }
}

/// The icon to draw inside a player's name tag, including team formatting.
pub fn name_tag_icon(text: &str) -> Option<&'static str> {
    let v = voice();
    let session = v
        .session
        .as_ref()
        .filter(|s| v.settings.hud && s.secret.is_some())?;
    let now = Instant::now();
    let talking = v.shared.talking.lock().unwrap();
    let (id, player) = session.player_for_name_tag(text)?;
    player_icon(session, &talking, now, id, player).map(|(icon, _)| icon)
}
