use super::session::Session;
use super::{Key, Settings, World, ui};
use crate::voice::audio::{AudioIo, Devices, MicQueue};
use crate::voice::i18n::{self, fmt, strings};
use crate::voice::mix::Mixer;
use crate::voice::net::{Conn, IDLE_LEVEL_DB, Redirect, Shared, Status};
use crate::voice::proto::{self, Group, PlayerState, Secret};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub(super) struct Voice {
    pub(super) settings: Settings,
    pub(super) loaded: bool,
    pub(super) session: Option<Session>,
    /// Custom payloads for the server, sent at the tick.
    pub(super) outbox: Vec<(String, Vec<u8>)>,
    /// Settings to write back, and chat lines for the player.
    pub(super) store: Vec<(String, String)>,
    pub(super) chat: Vec<String>,
    pub(super) shared: Arc<Shared>,
    pub(super) mixer: Arc<Mutex<Mixer>>,
    pub(super) mic: Arc<MicQueue>,
    pub(super) devices: Arc<Mutex<Devices>>,
    pub(super) redirect: Option<Redirect>,
    pub(super) menu: ui::Menu,
    pub(super) ui_click: bool,
    /// Device names for the settings screen, refreshed while it is open.
    pub(super) device_list: Option<(Vec<String>, Vec<String>)>,
    pub(super) devices_scanned: Option<Instant>,
    pub(super) volume_order: Option<Vec<u128>>,
}

pub(super) fn voice() -> MutexGuard<'static, Voice> {
    static VOICE: LazyLock<Mutex<Voice>> = LazyLock::new(|| {
        Mutex::new(Voice {
            settings: Settings::default(),
            loaded: false,
            session: None,
            outbox: Vec::new(),
            store: Vec::new(),
            chat: Vec::new(),
            shared: Arc::default(),
            mixer: Arc::default(),
            mic: Arc::default(),
            devices: Arc::new(Mutex::new(Devices::default())),
            redirect: None,
            menu: ui::Menu::default(),
            ui_click: false,
            device_list: None,
            devices_scanned: None,
            volume_order: None,
        })
    });
    VOICE.lock().unwrap_or_else(|e| e.into_inner())
}

static SOUND_CARD: AtomicBool = AtomicBool::new(true);

/// For tools and tests: leave the sound card alone, and feed [`mic_queue`]
/// and read [`mixer`] instead.
pub fn use_sound_card(on: bool) {
    SOUND_CARD.store(on, Ordering::Relaxed);
}

pub fn mic_queue() -> Arc<MicQueue> {
    voice().mic.clone()
}

pub fn mixer() -> Arc<Mutex<Mixer>> {
    voice().mixer.clone()
}

/// The settings key a server's remembered version is kept under.
pub(super) fn compat_key(address: &str) -> String {
    let clean: String = address
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("compat.{clean}")
}

impl Voice {
    pub(super) fn apply_controls(&self) {
        let s = &self.settings;
        let mut c = self.shared.controls.lock().unwrap();
        c.mode = s.mode;
        c.muted = s.muted;
        c.disabled = s.disabled;
        c.volume = s.volume as f32 / 100.0;
        c.mic_gain = s.mic_gain as f32 / 100.0;
        c.threshold_db = s.threshold as f32;
        c.player_volumes = s
            .player_volumes
            .iter()
            .map(|(uuid, volume)| (*uuid, *volume as f32 / 100.0))
            .collect();
    }

    pub(super) fn save(&mut self) {
        self.apply_controls();
        let stored = self.settings.stored();
        self.store
            .extend(stored.into_iter().map(|(k, v)| (k.to_owned(), v)));
    }

    pub(super) fn set_threshold(&mut self, frac: f32) {
        self.settings.threshold = (-80.0 + frac.clamp(0.0, 1.0) * 80.0).round() as i32;
        self.apply_controls();
    }

    /// Closes audio while voice chat is disabled, or applies the selected devices.
    pub(super) fn restart_audio(&mut self) {
        self.device_list = None;
        self.mic.clear();
        if self.settings.disabled {
            if let Some(session) = &mut self.session {
                session.audio = None;
            }
            self.mic.clear();
            self.shared.transmitting.store(false, Ordering::Relaxed);
            self.shared
                .mic_level
                .store(IDLE_LEVEL_DB.to_bits(), Ordering::Relaxed);
            *self.devices.lock().unwrap() = Devices::default();
            self.mixer.lock().unwrap().clear();
            return;
        }
        if !SOUND_CARD.load(Ordering::Relaxed) {
            return;
        }
        let Some(session) = &mut self.session else {
            return;
        };
        let input = self.settings.input_device.clone();
        let output = self.settings.output_device.clone();
        if let Some(audio) = &session.audio
            && audio.restart(input.clone(), output.clone()).is_ok()
        {
            return;
        }
        session.audio = match AudioIo::start(
            self.mixer.clone(),
            self.mic.clone(),
            self.devices.clone(),
            input,
            output,
        ) {
            Ok(audio) => Some(audio),
            Err(e) => {
                *self.devices.lock().unwrap() = Devices {
                    input: Err(e.clone()),
                    output: Err(e.clone()),
                };
                None
            }
        };
    }

    pub(super) fn send(&mut self, (channel, data): (&str, Vec<u8>)) {
        self.outbox.push((channel.to_owned(), data));
    }

    /// Tells the server whether the player muted or turned voice chat off.
    pub(super) fn send_state(&mut self) {
        let Some(compat) = self
            .session
            .as_ref()
            .filter(|s| s.secret.is_some())
            .map(Session::compat)
        else {
            return;
        };
        let (muted, disabled) = (self.settings.muted, self.settings.disabled);
        self.send(proto::update_state(compat, muted, disabled));
    }

    fn leave(&mut self) {
        self.shared.connection.fetch_add(1, Ordering::Relaxed);
        self.session = None;
        self.mic.clear();
        self.shared.transmitting.store(false, Ordering::Relaxed);
        self.shared
            .mic_level
            .store(IDLE_LEVEL_DB.to_bits(), Ordering::Relaxed);
        self.shared.talking.lock().unwrap().clear();
        self.shared.activity.lock().unwrap().clear();
        self.shared.sources.lock().unwrap().clear();
        *self.shared.status.lock().unwrap() = Status::Idle;
        *self.shared.mic_error.lock().unwrap() = None;
        self.mixer.lock().unwrap().clear();
    }

    fn on_secret(&mut self, data: &[u8]) {
        let Ok(secret) = Secret::parse(data) else {
            self.chat
                .push("Voice chat: the server sent a secret this version can't read".into());
            return;
        };
        let Some(redirect) = self.redirect else {
            return;
        };
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if session.remembered != Some(secret.compat) {
            session.remembered = Some(secret.compat);
            self.store
                .push((compat_key(&session.address), secret.compat.to_string()));
        }
        // A new secret replaces the connection.
        session.conn = None;
        let server = match proto::split_host_port(&session.address) {
            Some((host, port)) => (host, port.unwrap_or(25565)),
            None => (session.address.clone(), 25565),
        };
        session.secret = Some(secret.clone());
        session.conn = match Conn::start(
            secret,
            server,
            redirect,
            self.shared.clone(),
            self.mixer.clone(),
            self.mic.clone(),
        ) {
            Ok(conn) => Some(conn),
            Err(e) => {
                self.chat.push(format!("Voice chat can't start: {e}"));
                None
            }
        };
        if session.audio.is_none() {
            self.restart_audio();
        }
        self.send_state();
    }

    fn on_payload(&mut self, channel: &str, data: &[u8]) {
        if channel == "voicechat:secret" {
            self.on_secret(data);
            return;
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let compat = session.compat();
        match channel {
            "voicechat:states" => {
                if let Ok(states) = PlayerState::parse_all(data, compat) {
                    session.set_states(states);
                }
            }
            "voicechat:state" => {
                if let Ok(s) = PlayerState::parse(data, compat) {
                    session.insert_state(s);
                }
            }
            "voicechat:remove_state" => {
                if let Ok(id) = proto::parse_uuid(data) {
                    session.remove_state(id);
                }
            }
            "voicechat:add_group" => {
                if let Ok(g) = Group::parse(data) {
                    session.groups.insert(g.id, g);
                }
            }
            "voicechat:remove_group" => {
                if let Ok(id) = proto::parse_uuid(data) {
                    session.groups.remove(&id);
                }
            }
            "voicechat:joined_group" => {
                if let Ok((group, wrong_password)) = proto::parse_joined_group(data) {
                    if wrong_password {
                        self.menu.say(strings().wrong_password.to_owned(), true);
                    } else {
                        session.group = group;
                        let name = group
                            .and_then(|g| session.groups.get(&g))
                            .map(|g| g.name.clone());
                        let said = match &name {
                            Some(n) => fmt(strings().joined_named, &[n]),
                            None if group.is_some() => strings().joined_group.to_owned(),
                            None => strings().left_group.to_owned(),
                        };
                        self.menu.say(said, false);
                    }
                }
            }
            _ => {}
        }
    }

    fn tick(&mut self, world: World, get: &dyn Fn(&str) -> Option<String>) {
        if !self.loaded {
            self.loaded = true;
            self.settings = Settings::load(get);
            self.apply_controls();
        }
        self.redirect = Some(world.redirect);
        if self
            .session
            .as_ref()
            .is_some_and(|s| s.address != world.address)
        {
            self.leave();
        }
        if self.session.is_none() {
            let remembered = get(&compat_key(world.address)).and_then(|v| v.parse().ok());
            self.session = Some(Session::new(world.address, remembered));
            self.send(("minecraft:register", proto::register_payload()));
        }
        if let Some(l) = world.listener {
            *self.shared.listener.lock().unwrap() = l;
        }
        *self.shared.players.lock().unwrap() = world.players.into_iter().collect();

        let protocol = self.settings.protocol;
        let session = self.session.as_mut().expect("made above");
        let now = Instant::now();
        if self.shared.transmitting.load(Ordering::Relaxed)
            && let Some(secret) = &session.secret
        {
            let whisper = self.shared.controls.lock().unwrap().whisper;
            self.shared
                .talking
                .lock()
                .unwrap()
                .insert(secret.player, (now, whisper));
        }
        if session.secret.is_none() && now >= session.next_ask {
            let plan = session.plan(protocol);
            if let Some(&compat) = plan.get(session.asked.len()) {
                session.asked.push(compat);
                session.next_ask = now + Duration::from_secs(3);
                self.send(proto::request_secret(compat));
            }
        }
        // The connection timed out: ask again, as the mod reconnects.
        let session = self.session.as_mut().expect("made above");
        let failed = matches!(*self.shared.status.lock().unwrap(), Status::Failed(_));
        if failed && session.secret.is_some() && now - session.joined > Duration::from_secs(5) {
            let compat = session.compat();
            session.secret = None;
            session.conn = None;
            session.asked.clear();
            session.next_ask = now + Duration::from_secs(5);
            session.remembered = Some(compat);
        }
    }
}

/// A tick in a server's world (`world`), or out of one (None). Returns the
/// custom payloads to send the server and the settings to store.
pub fn tick(world: Option<World>, get: &dyn Fn(&str) -> Option<String>) -> Tick {
    let mut v = voice();
    match world {
        Some(w) => v.tick(w, get),
        None => {
            if v.session.is_some() {
                v.leave();
            }
        }
    }
    Tick {
        payloads: std::mem::take(&mut v.outbox),
        store: std::mem::take(&mut v.store),
        chat: std::mem::take(&mut v.chat),
    }
}

#[derive(Default)]
pub struct Tick {
    pub payloads: Vec<(String, Vec<u8>)>,
    pub store: Vec<(String, String)>,
    pub chat: Vec<String>,
}

/// A custom payload from the server: true when it was the voice chat's.
pub fn payload(channel: &str, data: &[u8]) -> bool {
    if !channel.starts_with("voicechat:") {
        return false;
    }
    voice().on_payload(channel, data);
    true
}

/// Applies a voice chat key.
pub fn key(which: Key, down: bool) {
    let mut v = voice();
    match which {
        Key::PushToTalk => v.shared.controls.lock().unwrap().ptt = down,
        Key::Whisper => v.shared.controls.lock().unwrap().whisper = down,
        Key::Mute if down => {
            v.settings.muted = !v.settings.muted;
            v.save();
            v.send_state();
        }
        Key::Disable if down => v.click(2),
        _ => {}
    }
}

pub fn settings_buttons() -> Vec<String> {
    voice().settings.buttons()
}

pub fn settings_click(button: usize) {
    voice().click(button);
}

pub fn load(get: &dyn Fn(&str) -> Option<String>) {
    let mut v = voice();
    if !v.loaded {
        v.loaded = true;
        v.settings = Settings::load(get);
        v.apply_controls();
    }
}

/// Reloads settings changed outside the mod and applies device and connection changes.
pub fn reload(get: &dyn Fn(&str) -> Option<String>) {
    let mut v = voice();
    let audio = (
        v.settings.disabled,
        v.settings.input_device.clone(),
        v.settings.output_device.clone(),
    );
    v.settings = Settings::load(get);
    v.apply_controls();
    if audio
        != (
            v.settings.disabled,
            v.settings.input_device.clone(),
            v.settings.output_device.clone(),
        )
    {
        v.restart_audio();
    }
    v.send_state();
}

/// Sets the player's language for the menu, HUD and chat, from the game's
/// `settings.language` (`en_us`, `ru_ru`, …). Call it on the game thread
/// before drawing; unknown languages fall back to English.
pub fn set_language(code: &str) {
    i18n::set_language(code);
}

/// Settings changed since the last call, to store.
pub fn take_store() -> Vec<(String, String)> {
    std::mem::take(&mut voice().store)
}
