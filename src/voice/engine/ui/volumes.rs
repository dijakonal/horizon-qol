use super::widgets::{BTN_ON, IC_PLAYER_VOLUME, IC_VOLUMES, inside};
use super::{Action, Field, Menu, Out, View};
use crate::voice::i18n::strings;

const VOLUME_ROW: f32 = 34.0;

impl Menu {
    pub(super) fn draw_volumes(
        &self,
        out: &mut Out<'_>,
        view: &View,
        x: f32,
        top: f32,
        w: f32,
        bottom: f32,
    ) {
        out.field(
            [x, top, w, 16.0],
            &self.volume_search,
            strings().search_audio_sources,
            false,
            self.focus == Some(Field::Search),
            Action::Focus(Field::Search),
        );
        let query = self.volume_search.trim().to_lowercase();
        let sources: Vec<_> = view
            .volume_sources
            .iter()
            .filter(|source| query.is_empty() || source.name.to_lowercase().contains(&query))
            .collect();
        let y = top + 20.0;
        let list_h = bottom - y;
        out.fill([x, y, w, list_h], 0xFFA0_A0A0);
        out.fill([x + 1.0, y + 1.0, w - 2.0, list_h - 2.0], 0xF018_181C);
        let fits = (((list_h - 2.0) / VOLUME_ROW).floor().max(1.0)) as usize;
        let scroll = self.scroll.min(sources.len().saturating_sub(fits));
        if sources.is_empty() {
            out.sprite(
                IC_VOLUMES,
                [0.0, 0.0, 16.0, 16.0],
                [x + (w - 24.0) / 2.0, y + list_h / 2.0 - 22.0, 24.0, 24.0],
            );
            let message = if query.is_empty() {
                strings().no_audio_sources
            } else {
                strings().no_matching_sources
            };
            let empty = out.fit(message, w - 16.0);
            out.text_sh(
                &empty,
                [x + (w - out.width(&empty)) / 2.0, y + list_h / 2.0 + 10.0],
                0xAAAAAA,
            );
        }
        for (i, source) in sources.into_iter().skip(scroll).take(fits).enumerate() {
            let ry = y + 1.0 + i as f32 * VOLUME_ROW;
            let row = [x + 1.0, ry, w - 2.0, VOLUME_ROW];
            if inside(row, out.mouse) {
                out.fill(row, 0x303A_3A44);
            }
            if source.speaking {
                out.fill([x + 1.0, ry, 2.0, VOLUME_ROW - 1.0], BTN_ON);
            }
            out.fill([x + 1.0, ry + VOLUME_ROW - 1.0, w - 2.0, 1.0], 0x20FF_FFFF);
            out.sprite(
                IC_PLAYER_VOLUME,
                [0.0, 0.0, 16.0, 16.0],
                [x + 7.0, ry + 3.0, 12.0, 12.0],
            );
            out.text_sh(
                &out.fit(&source.name, w - 75.0),
                [x + 25.0, ry + 3.0],
                if source.speaking { 0x55FF55 } else { 0xFFFFFF },
            );
            let value = format!("{}%", source.volume);
            let vw = 34.0;
            let tw = out.width(&value);
            out.text_sh(
                &value,
                [x + w - 7.0 - vw + (vw - tw) / 2.0, ry + 3.0],
                0xE0E0E0,
            );
            out.volume_slider(
                [x + 25.0, ry + 17.0, w - 32.0, 14.0],
                source.uuid,
                source.volume,
                self.volume_drag == Some(source.uuid),
            );
        }
    }
}
