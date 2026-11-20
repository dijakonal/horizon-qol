//! Translations for the voice chat interface and its entries in game menus.
//! Templates use `{}` for arguments, filled in order by [`fmt`].
use std::cell::Cell;

mod de_de;
mod en_us;
mod es_es;
mod fr_fr;
mod it_it;
mod pl_pl;
mod pt_br;
mod ru_ru;
mod uk_ua;
mod zh_cn;

pub struct Strings {
    pub activation: &'static str,
    pub activation_level: &'static str,
    pub asking_server: &'static str,
    pub audio_player: &'static str,
    pub automatic: &'static str,
    pub available_groups: &'static str,
    pub back: &'static str,
    pub cancel: &'static str,
    pub change_keys: &'static str,
    pub connected_proto: &'static str,
    pub connecting: &'static str,
    pub create: &'static str,
    pub creating: &'static str,
    pub default_device: &'static str,
    pub devices: &'static str,
    pub group: &'static str,
    pub group_name_hint: &'static str,
    pub group_password_hint: &'static str,
    pub group_type: &'static str,
    pub groups: &'static str,
    pub groups_off_server: &'static str,
    pub hk_disable: &'static str,
    pub hk_menu: &'static str,
    pub hk_mute: &'static str,
    pub hk_ptt: &'static str,
    pub hk_whisper: &'static str,
    pub hotkeys: &'static str,
    pub hud_group: &'static str,
    pub hud_ptt: &'static str,
    pub hud_voice: &'static str,
    pub hud_with: &'static str,
    pub in_group: &'static str,
    pub join: &'static str,
    pub joined_group: &'static str,
    pub joined_named: &'static str,
    pub joining: &'static str,
    pub kind_isolated: &'static str,
    pub kind_isolated_desc: &'static str,
    pub kind_normal: &'static str,
    pub kind_normal_desc: &'static str,
    pub kind_open: &'static str,
    pub kind_open_desc: &'static str,
    pub leave: &'static str,
    pub leaving: &'static str,
    pub left_group: &'static str,
    pub levels: &'static str,
    pub mic_error: &'static str,
    pub mic_gain: &'static str,
    pub microphone: &'static str,
    pub microphone_muted: &'static str,
    pub microphone_on: &'static str,
    pub muted: &'static str,
    pub name: &'static str,
    pub new_group: &'static str,
    pub no_audio_sources: &'static str,
    pub no_groups: &'static str,
    pub no_matching_sources: &'static str,
    pub no_voice_server: &'static str,
    pub nobody: &'static str,
    pub not_bound: &'static str,
    pub not_connected: &'static str,
    pub not_on_server: &'static str,
    pub off: &'static str,
    pub on: &'static str,
    pub password: &'static str,
    pub password_for: &'static str,
    pub password_optional: &'static str,
    pub password_tag: &'static str,
    pub player_volumes: &'static str,
    pub proto_auto: &'static str,
    pub protocol: &'static str,
    pub push_to_talk: &'static str,
    pub search_audio_sources: &'static str,
    pub settings: &'static str,
    pub speakers: &'static str,
    pub spk_error: &'static str,
    pub status_on_screen: &'static str,
    pub talking: &'static str,
    pub tip_mic_off: &'static str,
    pub tip_mic_on: &'static str,
    pub tip_mode_ptt: &'static str,
    pub tip_mode_voice: &'static str,
    pub tip_voice_off: &'static str,
    pub tip_voice_on: &'static str,
    pub type_label: &'static str,
    pub type_name_first: &'static str,
    pub type_password_first: &'static str,
    pub voice_activation: &'static str,
    pub voice_chat: &'static str,
    pub voice_chat_off: &'static str,
    pub voice_chat_on: &'static str,
    pub voice_chat_prefix: &'static str,
    pub voice_volume: &'static str,
    pub whispering: &'static str,
    pub wrong_password: &'static str,
}

const LANGUAGES: &[(&str, &Strings)] = &[
    ("en_us", &en_us::STRINGS),
    ("ru_ru", &ru_ru::STRINGS),
    ("uk_ua", &uk_ua::STRINGS),
    ("de_de", &de_de::STRINGS),
    ("es_es", &es_es::STRINGS),
    ("fr_fr", &fr_fr::STRINGS),
    ("pt_br", &pt_br::STRINGS),
    ("zh_cn", &zh_cn::STRINGS),
    ("pl_pl", &pl_pl::STRINGS),
    ("it_it", &it_it::STRINGS),
];

thread_local! {
    static LANGUAGE: Cell<&'static Strings> = const { Cell::new(&en_us::STRINGS) };
}

pub fn set_language(code: &str) {
    let code = code.to_lowercase();
    let language = LANGUAGES
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .or_else(|| {
            let prefix = code.split('_').next()?;
            LANGUAGES
                .iter()
                .find(|(candidate, _)| candidate.split('_').next() == Some(prefix))
        })
        .map_or(&en_us::STRINGS, |(_, strings)| *strings);
    LANGUAGE.with(|current| current.set(language));
}

pub fn strings() -> &'static Strings {
    LANGUAGE.with(Cell::get)
}

pub fn game_translation(key: &str) -> Option<String> {
    let strings = strings();
    let text = match key {
        "voice_chat.hk_disable" => strings.hk_disable,
        "voice_chat.hk_menu" => strings.hk_menu,
        "voice_chat.hk_mute" => strings.hk_mute,
        "voice_chat.hk_ptt" => strings.hk_ptt,
        "voice_chat.hk_whisper" => strings.hk_whisper,
        _ => return None,
    };
    Some(text.to_owned())
}

pub fn fmt<S: AsRef<str>>(template: &str, args: &[S]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    let mut args = args.iter();
    while let Some(pos) = rest.find("{}") {
        out.push_str(&rest[..pos]);
        if let Some(arg) = args.next() {
            out.push_str(arg.as_ref());
        } else {
            out.push_str("{}");
        }
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_and_formats() {
        set_language("en_us");
        assert_eq!(strings().settings, "Settings");
        assert_eq!(strings().devices, "Devices");
        assert_eq!(strings().default_device, "Default");
        assert_eq!(fmt(strings().joining, &["Builders"]), "Joining Builders...");
        set_language("ru_ru");
        assert_eq!(strings().settings, "Настройки");
        assert_eq!(strings().devices, "Устройства");
        assert_eq!(strings().default_device, "По умолчанию");
        assert_eq!(
            fmt(strings().hud_group, &["Строители"]),
            "Группа: Строители"
        );
        set_language("xx_yy");
        assert_eq!(strings().back, "Back");
        set_language("de_at");
        assert_eq!(strings().back, "Zurück");
        set_language("en_us");
    }

    #[test]
    fn game_menus_use_the_same_translations() {
        set_language("ru_ru");
        assert_eq!(
            game_translation("voice_chat.hk_ptt").as_deref(),
            Some("Рация (зажать)")
        );
        assert_eq!(game_translation("options.key.sneak"), None);
        set_language("en_us");
    }
}
