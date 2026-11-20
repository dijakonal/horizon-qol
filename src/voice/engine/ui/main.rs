use super::widgets::*;
use super::{Action, Menu, Out, Screen, View};
use crate::voice::i18n::strings;
use crate::voice::net::Mode;

impl Menu {
    pub(super) fn draw_main(&self, out: &mut Out<'_>, view: &View, size: [f32; 2]) {
        let (w, h) = MAIN_SIZE;
        let x0 = ((size[0] - w) / 2.0).round();
        let y0 = ((size[1] - h) / 2.0).round();
        out.panel_main([x0, y0, w, h]);

        let title = strings().voice_chat;
        out.text(
            title,
            [x0 + (w - out.width(title)) / 2.0, y0 + 8.0],
            TITLE_INK,
        );

        let (status, rgb) = &view.status;
        let st = out.fit(status, w - 16.0);
        out.text(&st, [x0 + (w - out.width(&st)) / 2.0, y0 + 22.0], ink(*rgb));

        let s = &view.settings;
        let in_group = view.groups.iter().any(|g| g.ours);
        let n = 6.0;
        let gap = 4.0;
        let row_w = n * 24.0 + (n - 1.0) * gap;
        let sx = x0 + (w - row_w) / 2.0;
        let iy = y0 + 38.0;
        let at = |i: f32| sx + i * (24.0 + gap);
        out.icon_square(
            at(0.0),
            iy,
            if s.muted { IC_MIC_OFF } else { IC_MIC_ON },
            Action::Mute,
            !s.muted,
            if s.muted {
                strings().tip_mic_off
            } else {
                strings().tip_mic_on
            },
        );
        out.icon_square(
            at(1.0),
            iy,
            if s.disabled { IC_SPK_OFF } else { IC_SPK_ON },
            Action::Disable,
            !s.disabled,
            if s.disabled {
                strings().tip_voice_off
            } else {
                strings().tip_voice_on
            },
        );
        let voice = s.mode == Mode::Voice;
        out.icon_square(
            at(2.0),
            iy,
            if voice { IC_VOICE } else { IC_PTT },
            Action::Mode,
            true,
            if voice {
                strings().tip_mode_voice
            } else {
                strings().tip_mode_ptt
            },
        );
        out.icon_square(
            at(3.0),
            iy,
            IC_VOLUMES,
            Action::Open(Screen::Volumes),
            true,
            strings().player_volumes,
        );
        out.icon_square(
            at(4.0),
            iy,
            IC_SETTINGS,
            Action::Open(Screen::Settings),
            true,
            strings().settings,
        );
        out.icon_square(
            at(5.0),
            iy,
            IC_GROUPS,
            Action::Open(Screen::Groups),
            in_group,
            strings().groups,
        );
    }
}
