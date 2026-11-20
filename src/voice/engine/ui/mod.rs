//! The voice chat menu (V): the connection, the controls, the hotkeys and the
//! groups, drawn on the designer's own grey panels (shipped in the icon
//! resource pack). It lays itself out from a [`View`] of the voice chat each
//! frame and keeps only what is typed and focused. Every label is localised
//! through [`crate::voice::i18n`].
use crate::voice::Settings;
use crate::voice::i18n::strings;

mod groups;
mod main;
mod settings;
mod volumes;
mod widgets;

use widgets::{CARD_LIGHT, CARD_LINE, Out, draw_tooltip, inside, slider_fraction};

#[cfg(test)]
mod tests;

/// What a hook draws, in GUI pixels. `Sprite` blits part of a texture (a
/// designer panel, an icon), `Fill` draws solid rectangles and `Text` labels.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Fill([f32; 4], u32),
    Text(String, [f32; 2], u32, bool),
    /// Part `src` (`[x, y, w, h]` in the sprite's own pixels) of sprite
    /// `name`, stretched into `dst` (GUI pixels). Maps to `native_sprite`.
    Sprite {
        name: &'static str,
        src: [f32; 4],
        dst: [f32; 4],
    },
}

/// Which screen of the menu is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Main,
    Settings,
    Groups,
    Volumes,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Mute,
    Disable,
    Mode,
    Volume(i32),
    PlayerVolume(u128, u32),
    Gain(i32),
    /// Pointer on the level meter; the fraction along its handle track
    /// (0.0 = quiet, 1.0 = loud).
    ThresholdAt(f32),
    /// Open or close the input/output device list under its picker.
    OpenDrop {
        input: bool,
    },
    /// Pick a device from the open list; `None` is the OS default.
    PickDevice {
        input: bool,
        name: Option<String>,
    },
    /// Scroll the open device list.
    DropScroll(i32),
    Join(u128),
    JoinWithPassword,
    CancelJoin,
    Leave,
    Focus(Field),
    Kind,
    Create,
    ShowCreate,
    CancelCreate,
    Close,
    /// Navigate to another screen, handled inside the menu (not by the session).
    Open(Screen),
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Password,
    JoinPassword,
    Search,
}

/// A device list open under a picker.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dropdown {
    pub input: bool,
    pub scroll: usize,
}

/// A group as the menu lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupRow {
    pub id: u128,
    pub name: String,
    pub has_password: bool,
    pub members: Vec<String>,
    pub ours: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VolumeRow {
    pub uuid: u128,
    pub name: String,
    pub volume: u32,
    pub speaking: bool,
    pub activity: u32,
    pub distance: Option<f64>,
}

/// The voice chat as the menu shows it this frame.
pub struct View {
    pub status: (String, u32),
    pub settings: Settings,
    pub level_db: f32,
    pub connected: bool,
    pub groups_enabled: bool,
    pub groups: Vec<GroupRow>,
    /// The hotkeys: what each does and the key bound to it, already named.
    pub hotkeys: Vec<(String, String)>,
    /// Input and output device names for the settings screen.
    pub devices: (Vec<String>, Vec<String>),
    pub volume_sources: Vec<VolumeRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    pub rect: [f32; 4],
    pub action: Action,
}

pub const NAME_MAX: usize = crate::voice::proto::MAX_GROUP_NAME;
const PASSWORD_MAX: usize = crate::voice::proto::MAX_GROUP_PASSWORD;
const SEARCH_MAX: usize = 48;

#[derive(Default)]
pub struct Menu {
    pub open: bool,
    pub screen: Screen,
    close_requested: bool,
    pub name: String,
    pub password: String,
    pub kind: i16,
    pub focus: Option<Field>,
    /// The password group being joined, and what is typed for it.
    pub joining: Option<u128>,
    pub creating: bool,
    pub join_password: String,
    pub message: Option<(String, u32)>,
    volume_search: String,
    scroll: usize,
    threshold_drag: bool,
    volume_drag: Option<u128>,
    /// The device list open under a picker, if any.
    dropdown: Option<Dropdown>,
    /// Where each device picker's face was drawn last frame, so the dropdown
    /// knows where to hang.
    drop_anchor_in: Option<[f32; 4]>,
    drop_anchor_out: Option<[f32; 4]>,
    /// The character of the key that opened the menu, typed with it.
    skip_char: Option<char>,
    widgets: Vec<Widget>,
}

impl Menu {
    pub fn open(&mut self, opened_by: Option<char>) {
        self.open = true;
        self.screen = Screen::Main;
        self.skip_char = opened_by;
        self.message = None;
    }

    pub fn reset(&mut self) {
        self.open = false;
        self.screen = Screen::Main;
        self.focus = None;
        self.joining = None;
        self.creating = false;
        self.join_password.clear();
        self.volume_search.clear();
        self.dropdown = None;
        self.threshold_drag = false;
        self.volume_drag = None;
        self.skip_char = None;
        self.widgets.clear();
    }

    pub fn close(&mut self) {
        self.reset();
        self.close_requested = true;
    }

    pub fn take_close(&mut self) -> bool {
        std::mem::take(&mut self.close_requested)
    }

    /// The game's screen is (`ours`) or isn't the menu: forget a menu the
    /// game closed.
    pub fn sync_open(&mut self, ours: bool) {
        if !ours && self.open {
            self.reset();
        }
    }

    /// The action under the pointer, from the last frame drawn.
    pub fn hit(&self, at: [f32; 2]) -> Option<Action> {
        self.widgets
            .iter()
            .rev()
            .find(|w| inside(w.rect, at))
            .map(|w| match &w.action {
                Action::ThresholdAt(_) => Action::ThresholdAt(slider_fraction(w.rect, at)),
                Action::PlayerVolume(uuid, _) => Action::PlayerVolume(
                    *uuid,
                    (slider_fraction(w.rect, at) * 200.0).round() as u32,
                ),
                other => other.clone(),
            })
    }

    /// Where the widget doing `action` was drawn last.
    #[cfg(test)]
    pub fn hit_rect(&self, action: &Action) -> Option<[f32; 4]> {
        self.widgets
            .iter()
            .find(|w| &w.action == action)
            .map(|w| w.rect)
    }

    pub fn drag(&self, at: [f32; 2]) -> Option<Action> {
        if self.threshold_drag {
            let rect = self
                .widgets
                .iter()
                .find(|w| matches!(w.action, Action::ThresholdAt(_)))?
                .rect;
            return Some(Action::ThresholdAt(slider_fraction(rect, at)));
        }
        let uuid = self.volume_drag?;
        let rect = self
            .widgets
            .iter()
            .find(|w| matches!(w.action, Action::PlayerVolume(id, _) if id == uuid))?
            .rect;
        Some(Action::PlayerVolume(
            uuid,
            (slider_fraction(rect, at) * 200.0).round() as u32,
        ))
    }

    pub fn end_drag(&mut self) -> bool {
        let threshold = std::mem::take(&mut self.threshold_drag);
        let volume = self.volume_drag.take().is_some();
        threshold || volume
    }

    fn field_mut(&mut self, f: Field) -> &mut String {
        match f {
            Field::Name => &mut self.name,
            Field::Password => &mut self.password,
            Field::JoinPassword => &mut self.join_password,
            Field::Search => &mut self.volume_search,
        }
    }

    pub fn char_typed(&mut self, c: char) {
        if let Some(skip) = self.skip_char.take()
            && skip.eq_ignore_ascii_case(&c)
        {
            return;
        }
        let Some(f) = self.focus else { return };
        let max = match f {
            Field::Name => NAME_MAX,
            Field::Search => SEARCH_MAX,
            _ => PASSWORD_MAX,
        };
        let field = self.field_mut(f);
        let password = matches!(f, Field::Password | Field::JoinPassword);
        if !c.is_control() && field.chars().count() < max && !(password && c == ' ') {
            field.push(c);
        }
    }

    /// Editing keys. Returns the action Enter asks for, if any.
    pub fn key(&mut self, key: &str, shift: bool) -> Option<Action> {
        self.skip_char = None;
        match key {
            "key.keyboard.escape" => {
                if self.dropdown.is_some() {
                    self.dropdown = None;
                    return None;
                }
                if self.joining.is_some() {
                    return Some(Action::CancelJoin);
                }
                if self.focus.is_some() {
                    self.focus = None;
                    return None;
                }
                if self.creating {
                    self.creating = false;
                    return None;
                }
                // Escape on a sub-screen steps back to the main menu.
                if self.screen != Screen::Main {
                    self.screen = Screen::Main;
                    return None;
                }
                return Some(Action::Close);
            }
            "key.keyboard.backspace" => {
                if let Some(f) = self.focus {
                    self.field_mut(f).pop();
                }
            }
            "key.keyboard.tab" => {
                self.focus = if self.screen == Screen::Volumes {
                    Some(Field::Search)
                } else {
                    match (self.focus, shift) {
                        (Some(Field::JoinPassword), _) => Some(Field::JoinPassword),
                        (Some(Field::Name), false) | (Some(Field::Password), true) => {
                            Some(Field::Password)
                        }
                        _ => Some(Field::Name),
                    }
                };
                if self.joining.is_some() {
                    self.focus = Some(Field::JoinPassword);
                }
            }
            "key.keyboard.enter" | "key.keyboard.keypad.enter" => match self.focus {
                Some(Field::JoinPassword) => return Some(Action::JoinWithPassword),
                Some(Field::Search) | None => {}
                Some(_) => return Some(Action::Create),
            },
            "key.keyboard.up" => self.scroll = self.scroll.saturating_sub(1),
            "key.keyboard.down" => self.scroll += 1,
            _ => {}
        }
        None
    }

    /// Whether typing goes into a field, so other keys (the menu key) type.
    pub fn typing(&self) -> bool {
        self.focus.is_some()
    }

    pub fn scroll_by(&mut self, steps: i32) {
        self.scroll = (self.scroll as i32 - steps).max(0) as usize;
    }

    pub fn draw(
        &mut self,
        view: &View,
        size: [f32; 2],
        mouse: [f32; 2],
        measure: &dyn Fn(&str) -> f32,
    ) -> Vec<Draw> {
        let mut out = Out {
            draws: Vec::new(),
            widgets: Vec::new(),
            mouse,
            measure,
            tip: None,
        };
        out.fill([0.0, 0.0, size[0], size[1]], 0x8000_0000);
        match self.screen {
            Screen::Main => self.draw_main(&mut out, view, size),
            Screen::Settings | Screen::Groups | Screen::Volumes => {
                self.draw_sub(&mut out, view, size)
            }
        }
        if let Some(tip) = out.tip.take() {
            draw_tooltip(&mut out, &tip, size);
        }
        self.widgets = out.widgets;
        out.draws
    }

    fn draw_sub(&mut self, out: &mut Out<'_>, view: &View, size: [f32; 2]) {
        let settings = self.screen == Screen::Settings;
        let compact = settings && size[1] < 270.0;
        let narrow = matches!(self.screen, Screen::Settings | Screen::Volumes);
        let w = (size[0] - 16.0)
            .min(if narrow { 320.0 } else { 340.0 })
            .max(if narrow { 220.0 } else { 240.0 });
        let h = if settings {
            if compact { 235.0 } else { 263.0 }
        } else {
            (size[1] - 16.0).clamp(216.0, 240.0)
        };
        let x0 = ((size[0] - w) / 2.0).round();
        let y0 = ((size[1] - h) / 2.0).round();
        out.panel_sub([x0, y0, w, h]);

        let pad = 10.0;
        let title = match self.screen {
            Screen::Settings => strings().settings,
            Screen::Groups => strings().groups,
            Screen::Volumes => strings().player_volumes,
            Screen::Main => strings().voice_chat,
        };
        out.heading(title, x0 + pad, y0 + 7.0);
        out.button(
            [x0 + w - pad - 48.0, y0 + 5.0, 48.0, 14.0],
            strings().back,
            Action::Back,
            None,
        );

        let cx = x0 + pad;
        let cw = w - pad * 2.0;
        out.fill([cx, y0 + 20.0, cw, 1.0], CARD_LINE);
        out.fill([cx, y0 + 21.0, cw, 1.0], CARD_LIGHT);
        match self.screen {
            Screen::Settings => {
                self.draw_controls(
                    out,
                    view,
                    cx,
                    y0 + if compact { 22.0 } else { 23.0 },
                    cw,
                    compact,
                );
                self.draw_dropdown(out, view);
            }
            Screen::Groups => self.draw_groups(out, view, cx, y0 + 24.0, cw, y0 + h - 8.0),
            Screen::Volumes => self.draw_volumes(out, view, cx, y0 + 24.0, cw, y0 + h - 8.0),
            Screen::Main => {}
        }
    }

    /// What a click on the menu's own parts does; the rest is the voice
    /// chat's to do (returned).
    pub fn act(&mut self, action: Action) -> Option<Action> {
        // Anything but the device list's own parts closes an open list.
        if self.dropdown.is_some()
            && !matches!(action, Action::OpenDrop { .. } | Action::DropScroll(_))
        {
            self.dropdown = None;
        }
        match action {
            Action::Focus(f) => self.focus = Some(f),
            Action::ThresholdAt(frac) => {
                self.threshold_drag = true;
                return Some(Action::ThresholdAt(frac));
            }
            Action::PlayerVolume(uuid, volume) => {
                self.volume_drag = Some(uuid);
                return Some(Action::PlayerVolume(uuid, volume));
            }
            Action::Kind => self.kind = (self.kind + 1) % 3,
            Action::ShowCreate => {
                self.creating = true;
                self.focus = Some(Field::Name);
                self.message = None;
                self.scroll = 0;
            }
            Action::CancelCreate => {
                self.creating = false;
                self.focus = None;
                self.message = None;
            }
            Action::OpenDrop { input } => {
                self.dropdown = match self.dropdown {
                    Some(dd) if dd.input == input => None,
                    _ => Some(Dropdown { input, scroll: 0 }),
                };
            }
            Action::DropScroll(d) => {
                if let Some(dd) = &mut self.dropdown {
                    dd.scroll = (dd.scroll as i64 + d as i64).max(0) as usize;
                }
            }
            Action::CancelJoin => {
                self.joining = None;
                self.join_password.clear();
                self.focus = None;
            }
            // Navigation between the menu's screens stays inside the menu.
            Action::Open(screen) => {
                self.screen = screen;
                self.focus = None;
                self.creating = false;
                self.message = None;
                self.dropdown = None;
                self.scroll = 0;
            }
            Action::Back => {
                if self.creating {
                    self.creating = false;
                } else {
                    self.screen = Screen::Main;
                }
                self.focus = None;
                self.dropdown = None;
            }
            Action::Close => self.close(),
            other => {
                if !matches!(other, Action::JoinWithPassword | Action::Create) {
                    self.focus = None;
                }
                return Some(other);
            }
        }
        None
    }

    pub fn say(&mut self, text: impl Into<String>, error: bool) {
        self.message = Some((text.into(), if error { 0xFF5555 } else { 0x55FF55 }));
    }
}
