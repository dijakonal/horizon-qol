use super::hud::*;
use super::*;
use crate::voice::audio::{AudioBackend, AudioStream, Devices, install_backend};
use crate::voice::net::{IDLE_LEVEL_DB, Mode, Shared, Status};
use crate::voice::proto::{PlayerState, Secret};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn world(address: &str) -> World<'_> {
    World {
        address,
        listener: None,
        players: Vec::new(),
        redirect: |h, p| (h.to_owned(), p),
    }
}

/// The tests share the one voice chat.
static SERIAL: Mutex<()> = Mutex::new(());

struct AudioProbe {
    starts: AtomicUsize,
    live: AtomicUsize,
}

static AUDIO_PROBE: AudioProbe = AudioProbe {
    starts: AtomicUsize::new(0),
    live: AtomicUsize::new(0),
};

struct ProbeStream;

impl AudioStream for ProbeStream {
    fn restart(&self, _input: Option<String>, _output: Option<String>) -> Result<(), String> {
        Ok(())
    }
}

impl Drop for ProbeStream {
    fn drop(&mut self) {
        AUDIO_PROBE.live.fetch_sub(1, Ordering::Relaxed);
    }
}

impl AudioBackend for AudioProbe {
    fn start(
        &self,
        _mixer: Arc<Mutex<crate::voice::mix::Mixer>>,
        _mic: Arc<crate::voice::audio::MicQueue>,
        devices: Arc<Mutex<Devices>>,
        _input: Option<String>,
        _output: Option<String>,
    ) -> Result<Box<dyn AudioStream>, String> {
        self.starts.fetch_add(1, Ordering::Relaxed);
        self.live.fetch_add(1, Ordering::Relaxed);
        *devices.lock().unwrap() = Devices {
            input: Ok("test microphone".into()),
            output: Ok("test speakers".into()),
        };
        Ok(Box::new(ProbeStream))
    }

    fn list_devices(&self) -> (Vec<String>, Vec<String>) {
        Default::default()
    }
}

#[test]
fn asks_for_the_secret_with_both_versions() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let none = |_: &str| None;
    let t = tick(Some(world("play.example.net")), &none);
    assert_eq!(t.payloads[0].0, "minecraft:register");
    assert!(
        t.payloads
            .iter()
            .all(|(c, _)| c != "voicechat:request_secret")
    );
    voice().session.as_mut().unwrap().next_ask = Instant::now();
    let t = tick(Some(world("play.example.net")), &none);
    assert_eq!(
        t.payloads,
        vec![(
            "voicechat:request_secret".into(),
            20i32.to_be_bytes().to_vec()
        )]
    );
    voice().session.as_mut().unwrap().next_ask = Instant::now();
    let t = tick(Some(world("play.example.net")), &none);
    assert_eq!(t.payloads[0].1, 21i32.to_be_bytes().to_vec());
    assert!(!payload("minecraft:brand", b""));
    let t = tick(None, &none);
    assert!(t.payloads.is_empty());
    assert!(voice().session.is_none());
}

#[test]
fn shows_other_players_groups() {
    use buf::Writer;
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let none = |_: &str| None;
    tick(None, &none);
    let mut w = world("groups.example.net");
    let positions = vec![(0xA, [0.0, 65.62, 10.0]), (0xB, [3.0, 65.62, 12.0])];
    w.players = positions.clone();
    tick(Some(w), &none);
    // As a 2.6 server sends them: a secret, the groups, then the states.
    let mut secret = Writer::new();
    secret.raw(&[1; 16]).i32(24454).uuid(0xC).u8(0).i32(1275);
    secret
        .raw(&48f64.to_be_bytes())
        .i32(1000)
        .bool(true)
        .utf("127.0.0.1")
        .bool(false);
    {
        let mut v = voice();
        let s = v.session.as_mut().unwrap();
        s.secret = Some(Secret::parse(&secret.finish()).unwrap());
    }
    for (id, name) in [(0x61u128, "Builders"), (0x62, "Miners")] {
        let mut g = Writer::new();
        g.uuid(id)
            .utf(name)
            .bool(false)
            .bool(true)
            .bool(false)
            .i16(0);
        assert!(payload("voicechat:add_group", &g.finish()));
    }
    let mut states = Writer::new();
    states.i32(3);
    for (id, name, group) in [
        (0xAu128, "Alex", 0x61u128),
        (0xB, "Bea", 0x62),
        (0xC, "Me", 0x61),
    ] {
        states
            .bool(false)
            .bool(false)
            .uuid(id)
            .utf(name)
            .bool(true)
            .uuid(group);
    }
    assert!(payload("voicechat:states", &states.finish()));
    let mut joined = Writer::new();
    joined.bool(true).uuid(0x61).bool(false);
    payload("voicechat:joined_group", &joined.finish());
    assert!(
        voice().chat.is_empty(),
        "group action produced a chat message"
    );

    assert_eq!(name_tag_icon("Alex"), Some(IC_GROUP));
    assert_eq!(name_tag_icon("[Builders] Bea"), Some(IC_GROUP));
    let hud: Vec<String> = hud().into_iter().map(|l| l.text).collect();
    assert!(
        !hud.contains(&"Group: Builders".to_owned()),
        "group stayed in bottom HUD: {hud:?}"
    );
    let group = hud_group().expect("current group header");
    assert_eq!(
        (group.text.as_str(), group.icon),
        ("Group: Builders", Some(IC_GROUP))
    );

    // The menu: a password group asks for its password, then joins.
    {
        let mut v = voice();
        let s = v.session.as_mut().unwrap();
        s.groups.get_mut(&0x62).unwrap().has_password = true;
    }
    *voice().shared.status.lock().unwrap() = Status::Connected;
    menu_open(Some('v'));
    let measure = |t: &str| t.len() as f32 * 6.0;
    let keys = vec![("Push to talk".to_owned(), key_label(DEFAULT_PTT))];
    let has = |d: &[ui::Draw], want: &str| {
        d.iter()
            .any(|d| matches!(d, ui::Draw::Text(t, ..) if t == want))
    };
    // The hotkeys are on the Settings screen.
    voice().menu.screen = ui::Screen::Settings;
    let sdraws = menu_draw([480.0, 270.0], [0.0, 0.0], &measure, keys.clone());
    assert!(has(&sdraws, "Caps Lock"));
    // The groups are on the Groups screen.
    voice().menu.screen = ui::Screen::Groups;
    let draws = menu_draw([480.0, 270.0], [0.0, 0.0], &measure, keys.clone());
    assert!(has(&draws, "Miners") && has(&draws, "Alex, Me"));
    let join = voice().menu.hit_rect(&ui::Action::Join(0x62)).unwrap();
    tick(Some(world("groups.example.net")), &none);
    menu_click([join[0] + 1.0, join[1] + 1.0]);
    assert_eq!(voice().menu.joining, Some(0x62));
    menu_char('v');
    for c in "hunter2".chars() {
        menu_char(c);
    }
    menu_key("key.keyboard.enter", false, DEFAULT_MENU);
    let t = tick(Some(world("groups.example.net")), &none);
    assert_eq!(
        t.payloads,
        vec![proto::join_group(0x62, Some("hunter2"))]
            .into_iter()
            .map(|(c, d)| (c.to_owned(), d))
            .collect::<Vec<_>>()
    );
    menu_key(DEFAULT_MENU, false, DEFAULT_MENU);
    assert!(menu_take_close() && !menu_is_open());
    tick(None, &none);
}

#[test]
fn push_to_talk_toggle_and_hud_icons() {
    use buf::Writer;
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_language("en_us");
    let none = |_: &str| None;
    tick(None, &none);

    // Push to talk, held: sets the transmit intent down, then up. This is
    // what the key hook calls while the key is held and when it is let go.
    key(Key::PushToTalk, true);
    assert!(voice().shared.controls.lock().unwrap().ptt);
    key(Key::PushToTalk, false);
    assert!(!voice().shared.controls.lock().unwrap().ptt);

    // Mute changes the state without a chat notification.
    let before = voice().settings.muted;
    key(Key::Mute, true);
    assert_ne!(voice().settings.muted, before);
    assert!(voice().chat.is_empty());
    key(Key::Mute, true); // back to where it was

    // Before the secret arrives, the HUD already shows the asking status
    // with the disconnected icon (the server may not have the mod at all).
    tick(Some(world("hud.example.net")), &none);
    let pre: Vec<_> = hud().into_iter().map(|l| (l.text, l.icon)).collect();
    assert_eq!(
        pre,
        vec![("Asking the server...".to_owned(), Some(IC_DISCONNECTED))],
        "{pre:?}"
    );

    // A connected session: the HUD shows a status line with an icon on the
    // left (what the user sees outside the menu).
    let mut secret = Writer::new();
    secret.raw(&[1; 16]).i32(24454).uuid(0xC).u8(0).i32(1275);
    secret
        .raw(&48f64.to_be_bytes())
        .i32(1000)
        .bool(true)
        .utf("127.0.0.1")
        .bool(false);
    {
        let mut v = voice();
        v.session.as_mut().unwrap().secret = Some(Secret::parse(&secret.finish()).unwrap());
    }
    *voice().shared.status.lock().unwrap() = Status::Connected;
    let icons = || hud().into_iter().filter_map(|l| l.icon).collect::<Vec<_>>();
    // Connected but quiet: no idle microphone or mode rows.
    assert!(
        icons().is_empty(),
        "idle HUD should stay empty: {:?}",
        icons()
    );
    // Muted shows the mute icon; voice chat off shows the speaker-off icon.
    voice().settings.muted = true;
    assert!(icons().contains(&IC_MUTE), "{:?}", icons());
    voice().settings.muted = false;
    voice().settings.disabled = true;
    assert!(icons().contains(&IC_DISABLED), "{:?}", icons());
    voice().settings.disabled = false;
    voice().shared.transmitting.store(true, Ordering::Relaxed);
    assert!(
        icons().contains(&IC_TALKING),
        "active voice is missing: {:?}",
        icons()
    );
    voice().shared.controls.lock().unwrap().whisper = true;
    assert!(
        icons().contains(&IC_WHISPERING),
        "whispering is missing: {:?}",
        icons()
    );
    voice().shared.controls.lock().unwrap().whisper = false;
    voice().shared.transmitting.store(false, Ordering::Relaxed);
    tick(None, &none);
}

#[test]
fn group_roster_stays_visible_and_prioritises_speakers() {
    use buf::Writer;
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let none = |_: &str| None;
    tick(None, &none);
    tick(Some(world("roster.example.net")), &none);
    let mut secret = Writer::new();
    secret.raw(&[1; 16]).i32(24454).uuid(0xC).u8(0).i32(1275);
    secret
        .raw(&48f64.to_be_bytes())
        .i32(1000)
        .bool(true)
        .utf("127.0.0.1")
        .bool(false);
    {
        let mut v = voice();
        let s = v.session.as_mut().unwrap();
        s.secret = Some(Secret::parse(&secret.finish()).unwrap());
        s.group = Some(0x77);
        for (id, name) in [(0xA, "A"), (0xB, "B"), (0xC, "C")] {
            s.insert_state(PlayerState {
                uuid: id,
                name: name.into(),
                muted: false,
                disabled: false,
                disconnected: false,
                group: Some(0x77),
            });
        }
    }
    assert_eq!(
        hud_heads(usize::MAX).len(),
        3,
        "inactive group members are missing"
    );
    let now = Instant::now();
    {
        let v = voice();
        let mut talking = v.shared.talking.lock().unwrap();
        talking.insert(0xA, (now - Duration::from_secs(3), false));
        talking.insert(0xB, (now, false));
        talking.insert(0xC, (now - Duration::from_secs(1), false));
    }
    for (id, count) in [(0xB, 2), (0xC, 5), (0xA, 1)] {
        voice()
            .shared
            .activity
            .lock()
            .unwrap()
            .insert(id, net::Activity { last: now, count });
    }
    let all = hud_heads(usize::MAX);
    let order: Vec<_> = all.iter().map(|h| h.uuid).collect();
    assert_eq!(
        order,
        vec![0xA, 0xB, 0xC],
        "speech must not reorder the roster"
    );
    let heads = hud_heads(2);
    let order: Vec<_> = heads.iter().map(|h| h.uuid).collect();
    assert_eq!(
        order,
        vec![0xB, 0xC],
        "overflow should keep the most active players"
    );
    assert!(heads[0].talking && !heads[1].talking);
    tick(None, &none);
}

#[test]
fn speakers_without_groups_get_voice_icons() {
    use buf::Writer;
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let none = |_: &str| None;
    tick(None, &none);
    tick(Some(world("voice-label.example.net")), &none);
    let mut secret = Writer::new();
    secret.raw(&[1; 16]).i32(24454).uuid(0xC).u8(0).i32(1275);
    secret
        .raw(&48f64.to_be_bytes())
        .i32(1000)
        .bool(true)
        .utf("127.0.0.1")
        .bool(false);
    {
        let mut v = voice();
        v.settings.hud = true;
        let session = v.session.as_mut().unwrap();
        session.secret = Some(Secret::parse(&secret.finish()).unwrap());
        session.insert_state(PlayerState {
            uuid: 0xA,
            name: "Talker".into(),
            muted: false,
            disabled: false,
            disconnected: false,
            group: None,
        });
        v.shared
            .players
            .lock()
            .unwrap()
            .insert(0xA, [0.0, 65.62, 10.0]);
        v.shared
            .talking
            .lock()
            .unwrap()
            .insert(0xA, (Instant::now(), false));
    }
    assert_eq!(name_tag_icon("Talker"), Some(IC_OTHER_TALKING));
    assert_eq!(name_tag_icon("[Team] Talker"), Some(IC_OTHER_TALKING));
    assert_eq!(name_tag_icon("Talker2"), None);
    voice()
        .shared
        .talking
        .lock()
        .unwrap()
        .insert(0xA, (Instant::now(), true));
    assert_eq!(name_tag_icon("Talker"), Some(IC_OTHER_WHISPERING));
    voice()
        .shared
        .talking
        .lock()
        .unwrap()
        .insert(0xA, (Instant::now() - Duration::from_secs(3), false));
    assert_eq!(name_tag_icon("Talker"), None);
    for (muted, disabled, disconnected, icon) in [
        (true, false, false, IC_MUTE),
        (false, true, false, IC_DISABLED),
        (false, false, true, IC_DISCONNECTED),
    ] {
        {
            let mut v = voice();
            let player = v.session.as_mut().unwrap().states.get_mut(&0xA).unwrap();
            player.muted = muted;
            player.disabled = disabled;
            player.disconnected = disconnected;
        }
        assert_eq!(name_tag_icon("Talker"), Some(icon));
    }
    tick(None, &none);
}

#[test]
fn hud_shows_no_voice_server_with_disconnected_icon() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_language("en_us");
    let none = |_: &str| None;
    tick(None, &none);
    tick(Some(world("silent.example.net")), &none);
    // Every ask has been tried and none answered: no voice chat here.
    {
        let mut v = voice();
        let plan_len = v.session.as_ref().unwrap().plan(v.settings.protocol).len();
        v.session.as_mut().unwrap().asked = vec![proto::COMPAT_26; plan_len];
    }
    let lines = hud();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].text, "No voice chat on this server");
    assert_eq!(lines[0].rgb, GREY);
    assert_eq!(lines[0].icon, Some(IC_DISCONNECTED));
    tick(None, &none);
}

#[test]
fn hud_heads_lists_talking_and_muted() {
    use buf::Writer;
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_language("en_us");
    let none = |_: &str| None;
    tick(None, &none);
    tick(Some(world("heads.example.net")), &none);
    let mut secret = Writer::new();
    secret.raw(&[1; 16]).i32(24454).uuid(0xC).u8(0).i32(1275);
    secret
        .raw(&48f64.to_be_bytes())
        .i32(1000)
        .bool(true)
        .utf("127.0.0.1")
        .bool(false);
    {
        let mut v = voice();
        v.session.as_mut().unwrap().secret = Some(Secret::parse(&secret.finish()).unwrap());
    }
    {
        let mut v = voice();
        let s = v.session.as_mut().unwrap();
        s.insert_state(PlayerState {
            uuid: 0xA,
            name: "Talker".into(),
            muted: false,
            disabled: false,
            disconnected: false,
            group: None,
        });
        s.insert_state(PlayerState {
            uuid: 0xB,
            name: "MutedGuy".into(),
            muted: true,
            disabled: false,
            disconnected: false,
            group: None,
        });
        s.insert_state(PlayerState {
            uuid: 0xC,
            name: "Quiet".into(),
            muted: false,
            disabled: false,
            disconnected: false,
            group: None,
        });
    }
    // Talker is talking now; MutedGuy has a muted microphone.
    voice()
        .shared
        .talking
        .lock()
        .unwrap()
        .insert(0xA, (Instant::now(), false));
    let heads = hud_heads(usize::MAX);
    assert_eq!(heads.len(), 2, "{heads:?}");
    assert!(
        heads
            .iter()
            .any(|h| h.name == "Talker" && h.talking && !h.muted)
    );
    assert!(
        heads
            .iter()
            .any(|h| h.name == "MutedGuy" && !h.talking && h.muted)
    );
    assert_eq!(
        heads.iter().find(|h| h.name == "Talker").map(|h| h.uuid),
        Some(0xA)
    );
    assert_eq!(
        heads.iter().find(|h| h.name == "MutedGuy").map(|h| h.uuid),
        Some(0xB)
    );
    voice()
        .shared
        .talking
        .lock()
        .unwrap()
        .insert(0xA, (Instant::now() - Duration::from_secs(1), false));
    let recent = hud_heads(usize::MAX);
    assert!(
        recent.iter().any(|h| h.uuid == 0xA && !h.talking),
        "recent speaker did not become inactive"
    );
    voice()
        .shared
        .talking
        .lock()
        .unwrap()
        .insert(0xA, (Instant::now() - Duration::from_secs(3), false));
    assert!(
        !hud_heads(usize::MAX).iter().any(|h| h.uuid == 0xA),
        "old speaker did not expire"
    );
    voice().shared.transmitting.store(true, Ordering::Relaxed);
    tick(Some(world("heads.example.net")), &none);
    voice().shared.transmitting.store(false, Ordering::Relaxed);
    assert!(
        hud_heads(usize::MAX).iter().any(|h| h.uuid == 0xC),
        "local transmission did not add the player's own head"
    );
    // Without a voice chat server there are no heads at all.
    voice().session.as_mut().unwrap().secret = None;
    assert!(hud_heads(usize::MAX).is_empty());
    tick(None, &none);
}

#[test]
fn settings_round_trip_and_cycle() {
    let mut s = Settings::default();
    s.click(0);
    s.click(3);
    s.click(6);
    // Device choices survive the round trip (None stores as "").
    s.input_device = Some("USB Mic".into());
    s.output_device = Some("HDMI Out".into());
    s.player_volumes.insert(0xAB, 70);
    let stored: HashMap<_, _> = s.stored().into_iter().collect();
    let back = Settings::load(&|k| stored.get(k).cloned());
    assert_eq!(back, s);
    assert_eq!(
        (back.mode, back.volume, back.protocol),
        (Mode::Voice, 125, Protocol::V26)
    );
    // A None device loads back from its empty string.
    let mut bare = s.clone();
    bare.output_device = None;
    let stored: HashMap<_, _> = bare.stored().into_iter().collect();
    let back = Settings::load(&|k| stored.get(k).cloned());
    assert_eq!(back.output_device, None);
    assert_eq!(key_label("key.keyboard.caps.lock"), "Caps Lock");
    assert_eq!(
        compat_key("Play.Example.net:25565"),
        "compat.play.example.net_25565"
    );
}

#[test]
fn player_volumes_apply_to_any_audio_source_and_persist() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_language("en_us");
    use_sound_card(false);
    {
        let mut v = voice();
        v.settings.player_volumes.clear();
        v.menu_action(ui::Action::PlayerVolume(0xAB, 90));
    }
    assert_eq!(voice().settings.player_volumes.get(&0xAB), Some(&90));
    assert_eq!(
        voice()
            .shared
            .controls
            .lock()
            .unwrap()
            .player_volumes
            .get(&0xAB),
        Some(&0.9)
    );
    let stored: HashMap<_, _> = take_store().into_iter().collect();
    assert_eq!(
        stored.get("player_volumes").map(String::as_str),
        Some("000000000000000000000000000000ab:90")
    );

    voice()
        .shared
        .sources
        .lock()
        .unwrap()
        .insert(0xAB, Instant::now());
    let rows = voice().volume_sources();
    assert!(
        rows.iter()
            .any(|r| r.uuid == 0xAB && r.name == "Audio player · 000000ab" && r.volume == 90)
    );
    {
        let mut v = voice();
        v.settings.player_volumes.clear();
        v.apply_controls();
        v.shared.sources.lock().unwrap().clear();
    }
    take_store();
    use_sound_card(true);
}

#[test]
fn volume_sources_prioritise_speakers_activity_and_proximity() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let none = |_: &str| None;
    tick(None, &none);
    tick(Some(world("volume-order.example.net")), &none);
    let now = Instant::now();
    {
        let mut v = voice();
        let session = v.session.as_mut().unwrap();
        for (uuid, name) in [(1, "Quiet"), (2, "Near"), (3, "Busy"), (4, "Live")] {
            session.insert_state(PlayerState {
                uuid,
                name: name.into(),
                muted: false,
                disabled: false,
                disconnected: false,
                group: None,
            });
        }
        *v.shared.listener.lock().unwrap() = mix::Listener {
            eye: [0.0, 64.0, 0.0],
            yaw: 0.0,
        };
        let mut players = v.shared.players.lock().unwrap();
        players.insert(1, [1.0, 64.0, 0.0]);
        players.insert(2, [2.0, 64.0, 0.0]);
        players.insert(3, [20.0, 64.0, 0.0]);
        players.insert(4, [30.0, 64.0, 0.0]);
        drop(players);
        v.shared.talking.lock().unwrap().insert(4, (now, false));
        let mut activity = v.shared.activity.lock().unwrap();
        activity.insert(
            2,
            net::Activity {
                last: now,
                count: 2,
            },
        );
        activity.insert(
            3,
            net::Activity {
                last: now,
                count: 8,
            },
        );
        activity.insert(
            4,
            net::Activity {
                last: now,
                count: 1,
            },
        );
    }
    let rows = voice().volume_sources();
    let order: Vec<_> = rows.iter().map(|row| row.uuid).collect();
    assert_eq!(order, vec![4, 3, 2, 1]);
    assert!(rows[0].speaking);

    voice().menu.screen = ui::Screen::Volumes;
    let measure = |text: &str| text.len() as f32 * 6.0;
    menu_draw([480.0, 270.0], [0.0, 0.0], &measure, Vec::new());
    {
        let v = voice();
        v.shared.talking.lock().unwrap().clear();
        v.shared
            .talking
            .lock()
            .unwrap()
            .insert(1, (Instant::now(), false));
        v.shared.activity.lock().unwrap().insert(
            1,
            net::Activity {
                last: Instant::now(),
                count: 100,
            },
        );
    }
    let rows = voice().view(Vec::new()).volume_sources;
    let order: Vec<_> = rows.iter().map(|row| row.uuid).collect();
    assert_eq!(order, vec![4, 3, 2, 1], "open list must not reorder");

    voice().menu.screen = ui::Screen::Main;
    menu_draw([480.0, 270.0], [0.0, 0.0], &measure, Vec::new());
    voice().menu.screen = ui::Screen::Volumes;
    menu_draw([480.0, 270.0], [0.0, 0.0], &measure, Vec::new());
    let rows = voice().view(Vec::new()).volume_sources;
    let order: Vec<_> = rows.iter().map(|row| row.uuid).collect();
    assert_eq!(order, vec![1, 3, 2, 4], "reopening must refresh order");
    tick(None, &none);
}

#[test]
fn mic_level_is_silent_before_and_after_connection() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(Shared::default().level(), IDLE_LEVEL_DB);
    tick(None, &|_: &str| None);
    tick(Some(world("idle.example.net")), &|_: &str| None);
    voice()
        .shared
        .mic_level
        .store((-20.0f32).to_bits(), Ordering::Relaxed);
    assert_eq!(voice().view(Vec::new()).level_db, IDLE_LEVEL_DB);
    tick(None, &|_: &str| None);
    assert_eq!(voice().shared.level(), IDLE_LEVEL_DB);
    assert_eq!(voice().view(Vec::new()).level_db, IDLE_LEVEL_DB);
}

#[test]
fn threshold_slider_drags_live_and_saves_on_release() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    use_sound_card(false);
    tick(None, &|_: &str| None);
    menu_open(Some('v'));
    voice().menu.screen = ui::Screen::Settings;
    let measure = |t: &str| t.len() as f32 * 6.0;
    menu_draw([480.0, 270.0], [0.0, 0.0], &measure, Vec::new());
    let track = voice()
        .menu
        .hit_rect(&ui::Action::ThresholdAt(0.0))
        .unwrap();
    let centre = [track[0] + track[2] / 2.0, track[1] + 5.0];
    take_ui_click();
    menu_click(centre);
    assert!(take_ui_click());
    assert!(!take_ui_click());
    menu_click([0.0, 0.0]);
    assert!(!take_ui_click());
    assert_eq!(voice().settings.threshold, -40);
    take_store();

    let outside = [track[0] + track[2] + 20.0, track[1] + 5.0];
    menu_drag(outside);
    assert_eq!(voice().settings.threshold, 0);
    assert!(take_store().is_empty());
    let draws = menu_draw([480.0, 270.0], outside, &measure, Vec::new());
    assert!(draws.iter().any(|d| matches!(d, ui::Draw::Sprite { name, .. } if *name == "gui/sprites/widget/slider_handle_highlighted")));

    menu_release();
    let stored: HashMap<_, _> = take_store().into_iter().collect();
    assert_eq!(stored.get("threshold").map(String::as_str), Some("0"));
    menu_drag([track[0] - 20.0, track[1] + 5.0]);
    assert_eq!(voice().settings.threshold, 0);
    menu_release();
    assert!(take_store().is_empty());

    voice().set_threshold(0.375);
    voice().menu.reset();
    use_sound_card(true);
}

#[test]
fn pick_device_and_meter_click_update_settings_offline() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // No sound card: changing devices or the threshold must not touch
    // the audio.
    use_sound_card(false);
    {
        let mut v = voice();
        v.settings.input_device = None;
        v.menu_action(ui::Action::PickDevice {
            input: true,
            name: Some("USB Mic".into()),
        });
        v.menu_action(ui::Action::PickDevice {
            input: false,
            name: None,
        });
        // The meter spans -80..0 dB, quietest at the left.
        v.menu_action(ui::Action::ThresholdAt(0.5));
    }
    assert_eq!(voice().settings.input_device, Some("USB Mic".into()));
    assert_eq!(voice().settings.output_device, None);
    assert_eq!(voice().settings.threshold, -40);
    voice().menu.end_drag();
    use_sound_card(true);
}

#[test]
fn disabling_voice_chat_closes_audio_for_every_control() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let none = |_: &str| None;
    tick(None, &none);
    tick(Some(world("audio-lifecycle.example.net")), &none);
    install_backend(&AUDIO_PROBE);
    {
        let mut v = voice();
        v.settings.disabled = false;
        v.apply_controls();
        v.restart_audio();
    }
    assert_eq!(AUDIO_PROBE.live.load(Ordering::Relaxed), 1);
    assert_eq!(AUDIO_PROBE.starts.load(Ordering::Relaxed), 1);

    voice().mic.push(&[0.25; 32]);
    voice().shared.transmitting.store(true, Ordering::Relaxed);
    voice()
        .shared
        .mic_level
        .store((-20.0f32).to_bits(), Ordering::Relaxed);
    key(Key::Disable, true);
    assert!(voice().settings.disabled);
    assert!(voice().session.as_ref().unwrap().audio.is_none());
    assert_eq!(AUDIO_PROBE.live.load(Ordering::Relaxed), 0);
    assert!(voice().mic.samples.lock().unwrap().is_empty());
    assert!(!voice().shared.transmitting.load(Ordering::Relaxed));
    assert_eq!(voice().shared.level(), IDLE_LEVEL_DB);

    key(Key::Disable, true);
    assert_eq!(AUDIO_PROBE.live.load(Ordering::Relaxed), 1);
    assert_eq!(AUDIO_PROBE.starts.load(Ordering::Relaxed), 2);
    voice().menu_action(ui::Action::Disable);
    assert_eq!(AUDIO_PROBE.live.load(Ordering::Relaxed), 0);
    voice().menu_action(ui::Action::Disable);
    assert_eq!(AUDIO_PROBE.live.load(Ordering::Relaxed), 1);
    assert_eq!(AUDIO_PROBE.starts.load(Ordering::Relaxed), 3);

    reload(&|key| (key == "disabled").then(|| "true".into()));
    assert!(voice().settings.disabled);
    assert_eq!(AUDIO_PROBE.live.load(Ordering::Relaxed), 0);
    reload(&|key| (key == "disabled").then(|| "false".into()));
    assert!(!voice().settings.disabled);
    assert_eq!(AUDIO_PROBE.live.load(Ordering::Relaxed), 1);
    assert_eq!(AUDIO_PROBE.starts.load(Ordering::Relaxed), 4);

    tick(None, &none);
    assert_eq!(AUDIO_PROBE.live.load(Ordering::Relaxed), 0);
    take_store();
}
