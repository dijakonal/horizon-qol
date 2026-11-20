use super::hud::{GREEN, GREY, RED, YELLOW};
use super::voice::Voice;
use super::{ui, voice};
use crate::voice::audio;
use crate::voice::i18n::{fmt, strings};
use crate::voice::net::{IDLE_LEVEL_DB, Status};
use crate::voice::proto::{self, COMPAT_27};
use std::collections::HashMap;
use std::time::{Duration, Instant};

impl Voice {
    pub(super) fn click(&mut self, button: usize) {
        let before = (self.settings.muted, self.settings.disabled);
        self.settings.click(button);
        self.save();
        if before != (self.settings.muted, self.settings.disabled) {
            self.send_state();
        }
        if before.1 != self.settings.disabled {
            self.restart_audio();
        }
    }

    pub(super) fn status_line(&self) -> (String, u32) {
        let Some(s) = self.session.as_ref() else {
            return (strings().not_on_server.to_owned(), GREY);
        };
        match &s.secret {
            None if s.asked.len() >= s.plan(self.settings.protocol).len() => {
                (strings().no_voice_server.to_owned(), GREY)
            }
            None => (strings().asking_server.to_owned(), YELLOW),
            Some(secret) => {
                let version = if secret.compat >= COMPAT_27 {
                    "2.7"
                } else {
                    "2.6.x"
                };
                match &*self.shared.status.lock().unwrap() {
                    Status::Connected => (fmt(strings().connected_proto, &[version]), GREEN),
                    Status::Failed(e) => (e.clone(), RED),
                    _ => (strings().connecting.to_owned(), YELLOW),
                }
            }
        }
    }

    pub(super) fn volume_sources(&self) -> Vec<ui::VolumeRow> {
        let mut names: HashMap<u128, String> = self
            .session
            .as_ref()
            .map(|s| {
                s.states
                    .values()
                    .filter(|p| !p.disconnected)
                    .map(|p| (p.uuid, p.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let sources = self.shared.sources.lock().unwrap();
        for uuid in sources.keys().chain(self.settings.player_volumes.keys()) {
            names.entry(*uuid).or_insert_with(|| {
                fmt(
                    strings().audio_player,
                    &[&format!("{:08x}", uuid & 0xFFFF_FFFF)],
                )
            });
        }
        drop(sources);
        let now = Instant::now();
        let talking = self.shared.talking.lock().unwrap().clone();
        let activity = self.shared.activity.lock().unwrap().clone();
        let players = self.shared.players.lock().unwrap().clone();
        let listener = *self.shared.listener.lock().unwrap();
        let mut rows: Vec<_> = names
            .into_iter()
            .map(|(uuid, name)| ui::VolumeRow {
                uuid,
                name,
                volume: self
                    .settings
                    .player_volumes
                    .get(&uuid)
                    .copied()
                    .unwrap_or(100),
                speaking: talking
                    .get(&uuid)
                    .is_some_and(|(at, _)| now - *at < Duration::from_millis(300)),
                activity: activity.get(&uuid).map_or(0, |a| a.count),
                distance: players.get(&uuid).map(|at| {
                    (0..3)
                        .map(|axis| (at[axis] - listener.eye[axis]).powi(2))
                        .sum::<f64>()
                        .sqrt()
                }),
            })
            .collect();
        rows.sort_by(|a, b| {
            b.speaking
                .cmp(&a.speaking)
                .then_with(|| b.activity.cmp(&a.activity))
                .then_with(|| {
                    a.distance
                        .unwrap_or(f64::INFINITY)
                        .total_cmp(&b.distance.unwrap_or(f64::INFINITY))
                })
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.uuid.cmp(&b.uuid))
        });
        if let Some(order) = &self.volume_order {
            let rank: HashMap<_, _> = order
                .iter()
                .enumerate()
                .map(|(index, uuid)| (*uuid, index))
                .collect();
            rows.sort_by_key(|row| rank.get(&row.uuid).copied().unwrap_or(order.len()));
        }
        rows
    }

    pub(super) fn view(&self, hotkeys: Vec<(String, String)>) -> ui::View {
        let session = self.session.as_ref();
        let screen = self.menu.screen;
        let connected = session.is_some_and(|s| s.secret.is_some())
            && *self.shared.status.lock().unwrap() == Status::Connected;
        let mut groups: Vec<ui::GroupRow> = session
            .filter(|_| matches!(screen, ui::Screen::Main | ui::Screen::Groups))
            .map(|s| {
                let mut members = s.memberships();
                s.groups
                    .values()
                    .filter(|g| match screen {
                        ui::Screen::Main => s.group == Some(g.id),
                        _ => !g.hidden || s.group == Some(g.id),
                    })
                    .map(|g| ui::GroupRow {
                        id: g.id,
                        name: g.name.clone(),
                        has_password: g.has_password,
                        members: members.remove(&g.id).unwrap_or_default(),
                        ours: s.group == Some(g.id),
                    })
                    .collect()
            })
            .unwrap_or_default();
        groups.sort_by_key(|g| (!g.ours, g.name.to_lowercase()));
        ui::View {
            status: self.status_line(),
            settings: self.settings.clone(),
            level_db: if connected {
                self.shared.level()
            } else {
                IDLE_LEVEL_DB
            },
            connected,
            groups_enabled: session
                .and_then(|s| s.secret.as_ref())
                .is_some_and(|s| s.groups),
            groups,
            hotkeys: if screen == ui::Screen::Settings {
                hotkeys
            } else {
                Vec::new()
            },
            devices: if screen == ui::Screen::Settings {
                self.device_list.clone().unwrap_or_default()
            } else {
                Default::default()
            },
            volume_sources: if screen == ui::Screen::Volumes {
                self.volume_sources()
            } else {
                Vec::new()
            },
        }
    }

    /// What a click or Enter in the menu asks of the voice chat.
    pub(super) fn menu_action(&mut self, action: ui::Action) {
        use ui::Action as A;
        let Some(action) = self.menu.act(action) else {
            return;
        };
        let group_name = |v: &Voice, id: u128| {
            v.session
                .as_ref()
                .and_then(|s| s.groups.get(&id))
                .map(|g| (g.name.clone(), g.has_password))
        };
        match action {
            A::Mode => self.click(0),
            A::Mute => self.click(1),
            A::Disable => self.click(2),
            A::Volume(d) => {
                self.settings.volume = (self.settings.volume as i32 + d).clamp(0, 200) as u32;
                self.save();
            }
            A::PlayerVolume(uuid, volume) => {
                self.settings.player_volumes.insert(uuid, volume.min(200));
                self.save();
            }
            A::Gain(d) => {
                self.settings.mic_gain = (self.settings.mic_gain as i32 + d).clamp(25, 500) as u32;
                self.save();
            }
            A::PickDevice { input, name } => {
                // The dropdown hands back the picked device (None = the OS
                // default); re-open the streams on the new choice.
                if input {
                    self.settings.input_device = name;
                } else {
                    self.settings.output_device = name;
                }
                self.save();
                self.restart_audio();
            }
            A::ThresholdAt(frac) => {
                self.set_threshold(frac);
                self.save();
            }
            A::Join(id) => match group_name(self, id) {
                Some((_, true)) => {
                    self.menu.joining = Some(id);
                    self.menu.join_password.clear();
                    self.menu.focus = Some(ui::Field::JoinPassword);
                }
                Some((name, false)) => {
                    self.send(proto::join_group(id, None));
                    self.menu.say(fmt(strings().joining, &[&name]), false);
                }
                None => {}
            },
            A::JoinWithPassword => {
                let Some(id) = self.menu.joining.take() else {
                    return;
                };
                let password = std::mem::take(&mut self.menu.join_password);
                self.menu.focus = None;
                if password.is_empty() {
                    self.menu.say(strings().type_password_first, true);
                    self.menu.joining = Some(id);
                    self.menu.focus = Some(ui::Field::JoinPassword);
                    return;
                }
                let name = group_name(self, id).map(|(n, _)| n).unwrap_or_default();
                self.send(proto::join_group(id, Some(&password)));
                self.menu.say(fmt(strings().joining, &[&name]), false);
            }
            A::Leave => {
                self.send(proto::leave_group());
                self.menu.say(strings().leaving, false);
            }
            A::Create => {
                let name = self.menu.name.trim().to_owned();
                if name.is_empty() {
                    self.menu.say(strings().type_name_first, true);
                    self.menu.focus = Some(ui::Field::Name);
                    return;
                }
                if self.session.as_ref().is_none_or(|s| s.secret.is_none()) {
                    self.menu.say(strings().not_connected, true);
                    return;
                }
                let password = std::mem::take(&mut self.menu.password);
                let password = (!password.is_empty()).then_some(password);
                self.send(proto::create_group(
                    &name,
                    password.as_deref(),
                    self.menu.kind,
                ));
                self.menu.name.clear();
                self.menu.focus = None;
                self.menu.creating = false;
                self.menu.say(fmt(strings().creating, &[&name]), false);
            }
            _ => {}
        }
    }
}

/// Opens the menu, opened by a key typing `opened_by`.
pub fn menu_open(opened_by: Option<char>) {
    voice().menu.open(opened_by);
}

pub fn menu_is_open() -> bool {
    voice().menu.open
}

/// The game's screen is (`ours`) or isn't the menu.
pub fn menu_sync(ours: bool) {
    voice().menu.sync_open(ours);
}

/// Whether the game should close the menu now (once).
pub fn menu_take_close() -> bool {
    voice().menu.take_close()
}

/// Draws the menu; `hotkeys` are what each key does and its key's name.
pub fn menu_draw(
    size: [f32; 2],
    mouse: [f32; 2],
    measure: &dyn Fn(&str) -> f32,
    hotkeys: Vec<(String, String)>,
) -> Vec<ui::Draw> {
    let mut v = voice();
    if v.menu.screen == ui::Screen::Settings
        && (v.device_list.is_none()
            || v.devices_scanned
                .is_none_or(|at| at.elapsed() >= Duration::from_secs(1)))
    {
        v.device_list = Some(audio::list_devices());
        v.devices_scanned = Some(Instant::now());
    }
    if v.menu.screen == ui::Screen::Volumes {
        if v.volume_order.is_none() {
            v.volume_order = Some(v.volume_sources().into_iter().map(|row| row.uuid).collect());
        }
    } else {
        v.volume_order = None;
    }
    let view = v.view(hotkeys);
    v.menu.draw(&view, size, mouse, measure)
}

pub fn menu_click(at: [f32; 2]) {
    let mut v = voice();
    if let Some(action) = v.menu.hit(at) {
        if !matches!(&action, ui::Action::Focus(_)) {
            v.ui_click = true;
        }
        v.menu_action(action);
    }
}

pub fn take_ui_click() -> bool {
    std::mem::take(&mut voice().ui_click)
}

pub fn menu_drag(at: [f32; 2]) {
    let mut v = voice();
    match v.menu.drag(at) {
        Some(ui::Action::ThresholdAt(frac)) => v.set_threshold(frac),
        Some(ui::Action::PlayerVolume(uuid, volume)) => {
            v.settings.player_volumes.insert(uuid, volume);
            v.apply_controls();
        }
        _ => {}
    }
}

pub fn menu_release() {
    let mut v = voice();
    if v.menu.end_drag() {
        v.save();
    }
}

/// A key on the menu: `menu_key` (the open key) closes it unless a field
/// takes the typing.
pub fn menu_key(key: &str, shift: bool, menu_key: &str) {
    let mut v = voice();
    if key == menu_key && !v.menu.typing() {
        v.menu.close();
        return;
    }
    if let Some(action) = v.menu.key(key, shift) {
        v.menu_action(action);
    }
}

pub fn menu_char(c: char) {
    voice().menu.char_typed(c);
}

pub fn menu_scroll(steps: f64) {
    voice().menu.scroll_by(steps.signum() as i32);
}

/// `key.keyboard.v` → `v`: the character a key types, for single letters
/// and digits.
pub fn key_char(key: &str) -> Option<char> {
    let name = key.strip_prefix("key.keyboard.")?;
    let mut chars = name.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}

/// Loads the stored settings before the first world, for the settings page.
/// `key.keyboard.caps.lock` → `Caps Lock`.
pub fn key_label(key: &str) -> String {
    let name = key
        .strip_prefix("key.keyboard.")
        .or_else(|| key.strip_prefix("key.mouse."))
        .unwrap_or(key);
    if name == "unknown" {
        return strings().not_bound.to_owned();
    }
    name.split('.')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}
