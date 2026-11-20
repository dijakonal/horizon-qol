use super::widgets::*;
use super::*;

fn view() -> View {
    View {
        status: ("Connected".into(), 0x55FF55),
        settings: Settings::default(),
        level_db: -30.0,
        connected: true,
        groups_enabled: true,
        groups: vec![
            GroupRow {
                id: 1,
                name: "Builders".into(),
                has_password: false,
                members: vec!["Alex".into()],
                ours: false,
            },
            GroupRow {
                id: 2,
                name: "Secret".into(),
                has_password: true,
                members: vec![],
                ours: false,
            },
        ],
        hotkeys: vec![
            ("voice_chat.hk_menu".into(), "V".into()),
            ("voice_chat.hk_ptt".into(), "Caps Lock".into()),
            ("voice_chat.hk_whisper".into(), "not bound".into()),
            ("voice_chat.hk_mute".into(), "J".into()),
            ("voice_chat.hk_disable".into(), "K".into()),
        ],
        devices: (vec!["USB Mic".into()], vec!["HDMI Out".into()]),
        volume_sources: vec![
            VolumeRow {
                uuid: 10,
                name: "Alex".into(),
                volume: 100,
                speaking: true,
                activity: 4,
                distance: Some(3.0),
            },
            VolumeRow {
                uuid: 11,
                name: "Audio player · 000000ff".into(),
                volume: 70,
                speaking: false,
                activity: 0,
                distance: None,
            },
        ],
    }
}

fn texts(draws: &[Draw]) -> Vec<String> {
    draws
        .iter()
        .filter_map(|d| {
            if let Draw::Text(t, ..) = d {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect()
}

fn centre(m: &Menu, action: &Action) -> [f32; 2] {
    let w = m.widgets.iter().find(|w| &w.action == action).unwrap();
    [w.rect[0] + w.rect[2] / 2.0, w.rect[1] + w.rect[3] / 2.0]
}

#[test]
fn main_menu_and_sub_screens() {
    crate::voice::i18n::set_language("en_us");
    let mut m = Menu::default();
    m.open(Some('v'));
    let measure = |t: &str| t.len() as f32 * 6.0;

    // Main menu: the designer's panel sprite, the title, the Settings /
    // Groups icon buttons, and no "Settings • Groups" hint line.
    let main = m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure);
    assert!(
        main.iter()
            .any(|d| matches!(d, Draw::Sprite { name, .. } if *name == PANEL_MAIN.0))
    );
    assert!(texts(&main).iter().any(|x| x == "Voice Chat"));
    assert!(
        m.widgets
            .iter()
            .any(|w| w.action == Action::Open(Screen::Settings))
    );
    assert!(
        m.widgets
            .iter()
            .any(|w| w.action == Action::Open(Screen::Groups))
    );
    assert!(
        m.widgets
            .iter()
            .any(|w| w.action == Action::Open(Screen::Volumes))
    );
    assert!(
        !texts(&main).iter().any(|x| x.contains('\u{2022}')),
        "hint line must be gone: {:?}",
        texts(&main)
    );

    // Settings screen: the sub panel, the device sections, the VU strip
    // and the hotkeys. The strip reports the pointer's handle position.
    m.act(Action::Open(Screen::Settings));
    let sd = m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure);
    assert!(
        sd.iter()
            .any(|d| matches!(d, Draw::Sprite { name, .. } if *name == PANEL_SUB.0))
    );
    let s = texts(&sd);
    for want in [
        "Devices",
        "Levels",
        "Microphone",
        "Speakers",
        "Default",
        "Hotkeys",
        "Push to talk",
        "Caps Lock",
        "not bound",
    ] {
        assert!(s.iter().any(|x| x == want), "{want} missing from {s:?}");
    }
    assert!(
        m.widgets
            .iter()
            .any(|w| w.action == Action::OpenDrop { input: true })
    );
    let track = m.hit_rect(&Action::ThresholdAt(0.0)).unwrap();
    assert_eq!(
        m.hit([track[0], track[1] + 5.0]),
        Some(Action::ThresholdAt(0.0))
    );
    assert_eq!(
        m.hit([track[0] + track[2] / 2.0, track[1] + 5.0]),
        Some(Action::ThresholdAt(0.5))
    );
    assert_eq!(
        m.hit([track[0] + track[2] - 1.0, track[1] + 5.0]),
        Some(Action::ThresholdAt(1.0))
    );

    // Groups screen: the groups, their members, and a Join hit area.
    m.act(Action::Open(Screen::Groups));
    let g = texts(&m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure));
    for want in ["Builders", "Alex", "Secret"] {
        assert!(g.iter().any(|x| x == want), "{want} missing from {g:?}");
    }
    let join = centre(&m, &Action::Join(1));
    assert_eq!(m.hit(join), Some(Action::Join(1)));

    m.act(Action::Open(Screen::Volumes));
    let volumes = texts(&m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure));
    for want in [
        "Player volumes",
        "Alex",
        "100%",
        "Audio player · 000000ff",
        "70%",
    ] {
        assert!(
            volumes.iter().any(|x| x == want),
            "{want} missing from {volumes:?}"
        );
    }
    let slider = m.hit_rect(&Action::PlayerVolume(10, 100)).unwrap();
    assert_eq!(
        m.hit([slider[0], slider[1] + 7.0]),
        Some(Action::PlayerVolume(10, 0))
    );
    assert_eq!(
        m.hit([slider[0] + slider[2] / 2.0, slider[1] + 7.0]),
        Some(Action::PlayerVolume(10, 100))
    );
    assert_eq!(
        m.hit([slider[0] + slider[2] - 1.0, slider[1] + 7.0]),
        Some(Action::PlayerVolume(10, 200))
    );
    assert_eq!(
        m.act(Action::PlayerVolume(10, 100)),
        Some(Action::PlayerVolume(10, 100))
    );
    assert_eq!(
        m.drag([slider[0] + 4.0 + (slider[2] - 8.0) * 0.75, slider[1] + 7.0]),
        Some(Action::PlayerVolume(10, 150))
    );
    assert!(m.end_drag());
    let mut many = view();
    for i in 2..14 {
        many.volume_sources.push(VolumeRow {
            uuid: i,
            name: format!("Source {i}"),
            volume: 100,
            speaking: false,
            activity: 0,
            distance: None,
        });
    }
    m.scroll_by(-20);
    let end = texts(&m.draw(&many, [480.0, 270.0], [0.0, 0.0], &measure));
    assert!(
        end.iter().any(|t| t == "Source 13"),
        "volume list did not reach its end: {end:?}"
    );

    // Escape steps back to the main menu, then closes.
    assert_eq!(m.key("key.keyboard.escape", false), None);
    assert_eq!(m.screen, Screen::Main);
    assert_eq!(m.key("key.keyboard.escape", false), Some(Action::Close));
}

#[test]
fn volume_search_filters_immediately() {
    crate::voice::i18n::set_language("en_us");
    let measure = |t: &str| t.len() as f32 * 6.0;
    let mut m = Menu::default();
    m.open(Some('v'));
    m.act(Action::Open(Screen::Volumes));
    let initial = texts(&m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure));
    assert!(
        initial
            .iter()
            .any(|t| t == "Search players or audio sources")
    );

    m.act(Action::Focus(Field::Search));
    for c in "ALEX".chars() {
        m.char_typed(c);
    }
    let filtered = texts(&m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure));
    assert!(filtered.iter().any(|t| t == "ALEX_"));
    assert!(filtered.iter().any(|t| t == "Alex"));
    assert!(!filtered.iter().any(|t| t == "Audio player · 000000ff"));
    assert_eq!(m.key("key.keyboard.enter", false), None);

    m.volume_search = "nobody".into();
    let empty = texts(&m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure));
    assert!(
        empty
            .iter()
            .any(|t| t == "No matching players or audio sources")
    );
    assert!(!empty.iter().any(|t| t == "Alex"));
}

#[test]
fn settings_cards_fit_regular_and_compact_screens() {
    crate::voice::i18n::set_language("en_us");
    let measure = |t: &str| t.len() as f32 * 6.0;
    for (screen_h, panel_h, gap) in [(270.0, 263.0, 3.0), (240.0, 235.0, 5.0)] {
        let mut m = Menu::default();
        m.open(Some('v'));
        m.screen = Screen::Settings;
        let draws = m.draw(&view(), [480.0, screen_h], [0.0, 0.0], &measure);
        let mut cards: Vec<[f32; 4]> = draws
            .iter()
            .filter_map(|d| match d {
                Draw::Fill(rect, colour) if *colour == CARD_OUT && rect[3] > 40.0 => Some(*rect),
                _ => None,
            })
            .collect();
        cards.sort_by(|a, b| a[1].total_cmp(&b[1]));
        assert_eq!(cards.len(), 3);
        let panel_top = (screen_h - panel_h) / 2.0;
        let panel_bottom = panel_top + panel_h;
        assert!(cards[0][1] >= panel_top + 22.0);
        assert!(cards[2][1] + cards[2][3] <= panel_bottom);
        for pair in cards.windows(2) {
            assert_eq!(pair[1][1] - pair[0][1] - pair[0][3], gap);
        }
        for widget in &m.widgets {
            assert!(widget.rect[1] >= panel_top && widget.rect[1] + widget.rect[3] <= panel_bottom);
        }
    }
}

#[test]
fn group_list_and_create_form_fit_and_navigate() {
    crate::voice::i18n::set_language("en_us");
    let mut m = Menu::default();
    m.open(Some('v'));
    m.screen = Screen::Groups;
    let measure = |t: &str| t.len() as f32 * 6.0;
    let mut listed = view();
    listed.groups.push(GroupRow {
        id: 100,
        name: "Alpha".into(),
        has_password: false,
        members: vec!["Me".into()],
        ours: true,
    });
    let list = m.draw(&listed, [480.0, 270.0], [0.0, 0.0], &measure);
    assert!(m.widgets.iter().any(|w| w.action == Action::ShowCreate));
    assert!(texts(&list).iter().any(|t| t == "Available groups"));
    assert!(
        list.iter()
            .any(|d| matches!(d, Draw::Sprite { name, .. } if *name == IC_JOIN))
    );
    assert!(
        list.iter()
            .any(|d| matches!(d, Draw::Sprite { name, .. } if *name == IC_LEAVE))
    );
    assert!(!texts(&list).iter().any(|t| t == "Join" || t == "Leave"));
    for widget in &m.widgets {
        assert!(widget.rect[1] >= 15.0 && widget.rect[1] + widget.rect[3] <= 255.0);
    }

    let mut many = listed;
    for i in 3..12 {
        many.groups.push(GroupRow {
            id: i,
            name: format!("Group {i}"),
            has_password: false,
            members: Vec::new(),
            ours: false,
        });
    }
    m.scroll_by(-20);
    let end = texts(&m.draw(&many, [480.0, 270.0], [0.0, 0.0], &measure));
    assert!(
        end.iter().any(|t| t == "Group 11"),
        "continuous scroll did not reach the end: {end:?}"
    );
    assert!(!end.iter().any(|t| t.contains(" of ")));

    m.act(Action::ShowCreate);
    assert!(m.creating && m.focus == Some(Field::Name));
    let form = m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure);
    let form_text = texts(&form);
    for want in [
        "New group",
        "Group type",
        "name",
        "password (optional)",
        "No password",
        "Hear nearby players; they can't hear the group.",
    ] {
        assert!(
            form_text.iter().any(|t| t == want),
            "{want} missing from {form_text:?}"
        );
    }
    assert!(m.widgets.iter().any(|w| w.action == Action::Create));
    m.act(Action::Kind);
    let open = texts(&m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure));
    assert!(
        open.iter()
            .any(|t| t == "Nearby players and the group hear each other.")
    );
    m.act(Action::Kind);
    let isolated = texts(&m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure));
    assert!(
        isolated
            .iter()
            .any(|t| t == "Only group members hear each other.")
    );
    m.act(Action::Back);
    assert!(!m.creating && m.screen == Screen::Groups);
    m.act(Action::ShowCreate);
    m.act(Action::CancelCreate);
    assert!(!m.creating && m.focus.is_none());
}

#[test]
fn vu_meter_layers_full_track_veil_and_vanilla_handle() {
    crate::voice::i18n::set_language("en_us");
    let mut m = Menu::default();
    m.open(Some('v'));
    m.screen = Screen::Settings;
    let measure = |t: &str| t.len() as f32 * 6.0;
    let draws = m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure);
    let (track_index, src, dst) = draws
        .iter()
        .enumerate()
        .find_map(|(i, d)| match d {
            Draw::Sprite { name, src, dst } if *name == TRACK => Some((i, *src, *dst)),
            _ => None,
        })
        .unwrap();
    assert_eq!(src, [0.0, 0.0, 226.0, 18.0]);
    let (veil_index, veil) = draws
        .iter()
        .enumerate()
        .find_map(|(i, d)| match d {
            Draw::Fill(rect, colour) if *colour == 0x8010_1014 => Some((i, *rect)),
            _ => None,
        })
        .unwrap();
    assert!(track_index < veil_index);
    assert!(veil[0] > dst[0] + dst[2] / 2.0 && veil[2] < dst[2]);
    assert!(
        draws.iter().enumerate().any(|(i, d)| i > veil_index
            && matches!(d, Draw::Sprite { name, .. } if *name == SLIDER_HANDLE))
    );

    let track = m.hit_rect(&Action::ThresholdAt(0.0)).unwrap();
    let hover = [track[0] + track[2] / 2.0, track[1] + 4.0];
    let hovered = m.draw(&view(), [480.0, 270.0], hover, &measure);
    assert!(
        hovered
            .iter()
            .any(|d| matches!(d, Draw::Sprite { name, .. } if *name == SLIDER_HANDLE_HOVER))
    );

    let mut idle_view = view();
    idle_view.level_db = crate::voice::net::IDLE_LEVEL_DB;
    let idle = m.draw(&idle_view, [480.0, 270.0], [0.0, 0.0], &measure);
    let track = idle
        .iter()
        .find_map(|d| match d {
            Draw::Sprite { name, dst, .. } if *name == TRACK => Some(*dst),
            _ => None,
        })
        .unwrap();
    let veil = idle
        .iter()
        .find_map(|d| match d {
            Draw::Fill(rect, colour) if *colour == 0x8010_1014 => Some(*rect),
            _ => None,
        })
        .unwrap();
    assert_eq!(veil, track);
}

#[test]
fn device_dropdown_lists_and_picks() {
    crate::voice::i18n::set_language("en_us");
    let mut m = Menu::default();
    m.open(Some('v'));
    m.screen = Screen::Settings;
    let measure = |t: &str| t.len() as f32 * 6.0;
    let v = view();
    m.draw(&v, [480.0, 270.0], [-1.0, -1.0], &measure);
    // Opening the microphone picker shows the default and the devices.
    assert_eq!(m.act(Action::OpenDrop { input: true }), None);
    assert!(m.dropdown.is_some());
    let d = texts(&m.draw(&v, [480.0, 270.0], [-1.0, -1.0], &measure));
    assert!(d.iter().any(|x| x == "USB Mic"), "{d:?}");
    // Picking a device closes the list and hands the choice to the session.
    let pick = Action::PickDevice {
        input: true,
        name: Some("USB Mic".into()),
    };
    assert_eq!(m.hit(centre(&m, &pick)), Some(pick.clone()));
    assert_eq!(m.act(pick.clone()), Some(pick));
    assert!(m.dropdown.is_none());
    // Escape closes the list before it leaves the screen.
    m.act(Action::OpenDrop { input: true });
    assert_eq!(m.key("key.keyboard.escape", false), None);
    assert!(m.dropdown.is_none());
    assert_eq!(m.screen, Screen::Settings);
}

#[test]
fn hovering_an_icon_shows_a_tooltip() {
    crate::voice::i18n::set_language("en_us");
    let mut m = Menu::default();
    m.open(Some('v'));
    let measure = |t: &str| t.len() as f32 * 6.0;
    // Draw once to lay out the widgets, find the mute button, hover it.
    m.draw(&view(), [480.0, 270.0], [-1.0, -1.0], &measure);
    let mute = centre(&m, &Action::Mute);
    let draws = m.draw(&view(), [480.0, 270.0], mute, &measure);
    assert!(
        texts(&draws).iter().any(|t| t.contains("Microphone")),
        "tooltip missing: {:?}",
        texts(&draws)
    );
}

#[test]
fn localises_the_menu() {
    crate::voice::i18n::set_language("ru_ru");
    let mut m = Menu::default();
    m.open(Some('v'));
    let measure = |t: &str| t.len() as f32 * 6.0;
    let main = texts(&m.draw(&view(), [480.0, 270.0], [0.0, 0.0], &measure));
    assert!(main.iter().any(|x| x == "Голосовой чат"), "{main:?}");
    crate::voice::i18n::set_language("en_us");
}

/// Writes each screen's draw list to JSON in the scratchpad for a visual
/// preview (`DUMP_MENU=<dir> cargo test dump_menu_json`). Off by default.
#[test]
fn dump_menu_json() {
    let Ok(dir) = std::env::var("DUMP_MENU") else {
        return;
    };
    crate::voice::i18n::set_language(
        &std::env::var("DUMP_LANG").unwrap_or_else(|_| "en_us".into()),
    );
    let measure = |t: &str| t.chars().count() as f32 * 6.0;
    let mut v = view();
    v.status = ("Connected (protocol 2.7)".into(), 0x55FF55);
    v.settings.mic_gain = 125;
    v.settings.input_device = Some("USB Microphone".into());
    v.settings.output_device = Some("Headphones".into());
    v.hotkeys[2].1 = "Left Alt".into();
    v.groups = vec![
        GroupRow {
            id: 3,
            name: "Expedition".into(),
            has_password: false,
            members: vec![
                "JKearnsl".into(),
                "Alice".into(),
                "Bob".into(),
                "Charlie".into(),
            ],
            ours: true,
        },
        GroupRow {
            id: 4,
            name: "Builders".into(),
            has_password: false,
            members: vec!["Alice".into(), "Bob".into()],
            ours: false,
        },
        GroupRow {
            id: 5,
            name: "Miners".into(),
            has_password: false,
            members: vec!["Charlie".into()],
            ours: false,
        },
        GroupRow {
            id: 6,
            name: "Redstone Lab".into(),
            has_password: true,
            members: vec![],
            ours: false,
        },
    ];
    v.volume_sources = vec![
        VolumeRow {
            uuid: 10,
            name: "Alice".into(),
            volume: 125,
            speaking: true,
            activity: 12,
            distance: Some(3.0),
        },
        VolumeRow {
            uuid: 11,
            name: "Bob".into(),
            volume: 80,
            speaking: false,
            activity: 8,
            distance: Some(5.0),
        },
        VolumeRow {
            uuid: 12,
            name: "JKearnsl".into(),
            volume: 100,
            speaking: false,
            activity: 6,
            distance: Some(0.0),
        },
        VolumeRow {
            uuid: 13,
            name: "Charlie".into(),
            volume: 60,
            speaking: false,
            activity: 3,
            distance: Some(8.0),
        },
        VolumeRow {
            uuid: 14,
            name: "Audio player · 000000ff".into(),
            volume: 70,
            speaking: false,
            activity: 1,
            distance: None,
        },
    ];
    let json = |d: &Draw| -> String {
        match d {
            Draw::Fill(r, a) => format!(
                "{{\"t\":\"fill\",\"r\":[{},{},{},{}],\"c\":{}}}",
                r[0], r[1], r[2], r[3], a
            ),
            Draw::Text(s, p, c, sh) => format!(
                "{{\"t\":\"text\",\"s\":{:?},\"p\":[{},{}],\"c\":{},\"sh\":{}}}",
                s, p[0], p[1], c, sh
            ),
            Draw::Sprite { name, src, dst } => format!(
                "{{\"t\":\"sprite\",\"n\":{:?},\"src\":[{},{},{},{}],\"dst\":[{},{},{},{}]}}",
                name, src[0], src[1], src[2], src[3], dst[0], dst[1], dst[2], dst[3]
            ),
        }
    };
    for (file, screen, mouse, size) in [
        ("main", Screen::Main, [-1.0, -1.0], [480.0, 270.0]),
        ("settings", Screen::Settings, [0.0, 0.0], [480.0, 270.0]),
        (
            "settings_compact",
            Screen::Settings,
            [0.0, 0.0],
            [480.0, 240.0],
        ),
        ("groups", Screen::Groups, [0.0, 0.0], [480.0, 270.0]),
        ("groups_create", Screen::Groups, [0.0, 0.0], [480.0, 270.0]),
        ("volumes", Screen::Volumes, [0.0, 0.0], [480.0, 270.0]),
    ] {
        let mut m = Menu::default();
        m.open(Some('v'));
        m.screen = screen;
        if file == "groups_create" {
            m.act(Action::ShowCreate);
            m.name = "North Expedition".into();
            m.password = "secret".into();
            m.kind = 1;
            m.focus = None;
        }
        let draws = m.draw(&v, size, mouse, &measure);
        let body: Vec<String> = draws.iter().map(json).collect();
        std::fs::write(
            format!("{dir}/menu_{file}.json"),
            format!("[{}]", body.join(",")),
        )
        .unwrap();
    }
}

#[test]
fn typing_a_new_group_and_creating_it() {
    crate::voice::i18n::set_language("en_us");
    let mut m = Menu::default();
    m.open(Some('v'));
    m.char_typed('v');
    assert_eq!(m.act(Action::Focus(Field::Name)), None);
    for c in "Team A".chars() {
        m.char_typed(c);
    }
    m.key("key.keyboard.tab", false);
    for c in "p w".chars() {
        m.char_typed(c);
    }
    assert_eq!((m.name.as_str(), m.password.as_str()), ("Team A", "pw"));
    m.act(Action::Kind);
    assert_eq!(m.kind, 1);
    assert_eq!(m.key("key.keyboard.enter", false), Some(Action::Create));
    assert_eq!(m.key("key.keyboard.escape", false), None);
    assert_eq!(m.key("key.keyboard.escape", false), Some(Action::Close));
}
