use super::widgets::{INK, INK_FAINT, INK_SOFT, inside};
use super::{Action, Menu, Out, View, Widget};
use crate::voice::i18n::{fmt, game_translation, strings};

impl Menu {
    pub(super) fn draw_controls(
        &mut self,
        out: &mut Out<'_>,
        view: &View,
        x: f32,
        top: f32,
        w: f32,
        compact: bool,
    ) {
        let s = &view.settings;
        let (devices_h, levels_h, hotkeys_h, gap) = if compact {
            (53.0, 76.0, 71.0, 5.0)
        } else {
            (55.0, 76.0, 97.0, 3.0)
        };
        let ix = x + 8.0;
        let iw = w - 16.0;
        let mut y = top;

        out.settings_card([x, y, w, devices_h], strings().devices);
        let input_y = y + if compact { 20.0 } else { 21.0 };
        let input = s
            .input_device
            .clone()
            .unwrap_or_else(|| strings().default_device.to_owned());
        out.device_picker(
            ix,
            input_y,
            iw,
            strings().microphone,
            &input,
            Action::OpenDrop { input: true },
        );
        self.drop_anchor_in = out.widgets.last().map(|wd| wd.rect);
        let output_y = y + if compact { 36.0 } else { 38.0 };
        let output = s
            .output_device
            .clone()
            .unwrap_or_else(|| strings().default_device.to_owned());
        out.device_picker(
            ix,
            output_y,
            iw,
            strings().speakers,
            &output,
            Action::OpenDrop { input: false },
        );
        self.drop_anchor_out = out.widgets.last().map(|wd| wd.rect);
        y += devices_h + gap;

        out.settings_card([x, y, w, levels_h], strings().levels);
        let volume_y = y + if compact { 20.0 } else { 21.0 };
        out.stepper(
            [ix, volume_y],
            iw,
            strings().voice_volume,
            &format!("{}%", s.volume),
            Action::Volume(-25),
            Action::Volume(25),
        );
        let gain_y = y + if compact { 34.0 } else { 36.0 };
        out.stepper(
            [ix, gain_y],
            iw,
            strings().mic_gain,
            &format!("{}%", s.mic_gain),
            Action::Gain(-25),
            Action::Gain(25),
        );
        let activation_y = y + if compact { 49.0 } else { 52.0 };
        out.text(strings().activation_level, [ix, activation_y], INK);
        let value = format!("{} dB", s.threshold);
        out.text(
            &value,
            [ix + iw - out.width(&value), activation_y],
            INK_SOFT,
        );
        let meter_y = y + if compact { 59.0 } else { 63.0 };
        out.vu_meter(
            ix,
            meter_y,
            iw,
            view.level_db,
            s.threshold,
            self.threshold_drag,
        );
        y += levels_h + gap;

        out.settings_card([x, y, w, hotkeys_h], strings().hotkeys);
        if compact {
            let column_gap = 6.0;
            let column_w = (iw - column_gap) / 2.0;
            for (i, (what, key)) in view.hotkeys.iter().enumerate() {
                let column = i / 3;
                let row = i % 3;
                let rx = ix + column as f32 * (column_w + column_gap);
                let ry = y + 20.0 + row as f32 * 13.0;
                let bound = *key != strings().not_bound && key != "not bound";
                let key_w = out.key_cap(rx + column_w, ry, column_w * 0.55, key, bound);
                out.text(
                    &out.fit(&hotkey_desc(what), column_w - key_w - 6.0),
                    [rx, ry],
                    INK_SOFT,
                );
            }
        } else {
            for (i, (what, key)) in view.hotkeys.iter().enumerate() {
                let ry = y + 21.0 + i as f32 * 13.0;
                let bound = *key != strings().not_bound && key != "not bound";
                let key_w = out.key_cap(ix + iw, ry, 90.0, key, bound);
                out.text(
                    &out.fit(&hotkey_desc(what), iw - key_w - 8.0),
                    [ix, ry],
                    INK_SOFT,
                );
            }
        }
        let hint_y = y + if compact { 60.0 } else { 86.0 };
        let change_keys = fmt(strings().change_keys, &[crate::voice::engine::NAME]);
        out.text(&out.fit(&change_keys, iw), [ix, hint_y], INK_FAINT);
    }

    /// The open device list: a dark popup under its picker with the OS
    /// default and the devices, the current one marked, scrolling when there
    /// are many.
    pub(super) fn draw_dropdown(&self, out: &mut Out<'_>, view: &View) {
        let Some(dd) = self.dropdown else { return };
        let anchor = if dd.input {
            self.drop_anchor_in
        } else {
            self.drop_anchor_out
        };
        let Some([ax, ay, aw, _]) = anchor else {
            return;
        };
        let mut entries: Vec<Option<String>> = vec![None];
        entries.extend(
            (if dd.input {
                &view.devices.0
            } else {
                &view.devices.1
            })
            .iter()
            .cloned()
            .map(Some),
        );
        let current = if dd.input {
            view.settings.input_device.as_deref()
        } else {
            view.settings.output_device.as_deref()
        };
        const VISIBLE: usize = 6;
        let scroll = dd.scroll.min(entries.len().saturating_sub(VISIBLE));
        let rows = entries.len().min(VISIBLE);
        let h = rows as f32 * 14.0 + 2.0;
        let (x, y, w) = (ax, ay + 14.0, aw);
        out.fill([x - 1.0, y - 1.0, w + 2.0, h + 2.0], 0xFF10_1014);
        out.fill([x, y, w, h], 0xF01E_1E22);
        for (i, name) in entries.iter().skip(scroll).take(VISIBLE).enumerate() {
            let ry = y + 1.0 + i as f32 * 14.0;
            let row = [x, ry, w - 10.0, 14.0];
            let label = name
                .clone()
                .unwrap_or_else(|| strings().default_device.to_owned());
            let is_current = match name {
                None => current.is_none(),
                Some(n) => current == Some(n.as_str()),
            };
            if inside(row, out.mouse) {
                out.fill(row, 0xFF3E_3E46);
            }
            if is_current {
                out.fill([x, ry, 2.0, 14.0], 0xFF4A_B64E);
            }
            out.text_sh(&out.fit(&label, w - 22.0), [x + 4.0, ry + 3.0], 0xFFFFFFFF);
            out.widgets.push(Widget {
                rect: row,
                action: Action::PickDevice {
                    input: dd.input,
                    name: name.clone(),
                },
            });
        }
        if entries.len() > VISIBLE {
            out.drop_arrow(x + w - 10.0, y + 1.0, "\u{25B2}", Action::DropScroll(-1));
            out.drop_arrow(
                x + w - 10.0,
                y + h - 10.0,
                "\u{25BC}",
                Action::DropScroll(1),
            );
        }
    }
}

/// The localised description corresponding to a label in `HOTKEYS`.
fn hotkey_desc(what: &str) -> String {
    game_translation(what).unwrap_or_else(|| what.to_owned())
}
