use super::widgets::*;
use super::{Action, Field, Menu, Out, View};
use crate::voice::i18n::{fmt, strings};

const ROW: f32 = 30.0;

impl Menu {
    pub(super) fn draw_groups(
        &self,
        out: &mut Out<'_>,
        view: &View,
        x: f32,
        top: f32,
        w: f32,
        bottom: f32,
    ) {
        if self.creating {
            self.draw_group_create(out, x, top, w, bottom);
            return;
        }
        if !view.connected || !view.groups_enabled {
            let why = if view.connected {
                strings().groups_off_server
            } else {
                strings().not_connected
            };
            out.fill([x, top, w, 50.0], CARD_OUT);
            out.fill([x + 1.0, top + 1.0, w - 2.0, 48.0], CARD_BG);
            out.sprite(
                IC_GROUPS,
                [0.0, 0.0, 16.0, 16.0],
                [x + 10.0, top + 17.0, 16.0, 16.0],
            );
            out.text(&out.fit(why, w - 48.0), [x + 34.0, top + 21.0], INK_SOFT);
            return;
        }

        let mut y = top;
        if let Some(ours) = view.groups.iter().find(|g| g.ours) {
            out.fill([x, y, w, 30.0], CARD_OUT);
            out.fill([x + 1.0, y + 1.0, w - 2.0, 28.0], CARD_BG);
            out.sprite(
                IC_GROUPS,
                [0.0, 0.0, 16.0, 16.0],
                [x + 7.0, y + 7.0, 16.0, 16.0],
            );
            let label = out.fit(&fmt(strings().in_group, &[&ours.name]), w - 94.0);
            out.text(&label, [x + 29.0, y + 4.0], ink(0x55FF55));
            let members = if ours.members.is_empty() {
                strings().nobody.to_owned()
            } else {
                ours.members.join(", ")
            };
            out.text(&out.fit(&members, w - 94.0), [x + 29.0, y + 15.0], INK_SOFT);
            out.icon_square(
                x + w - 30.0,
                y + 3.0,
                IC_LEAVE,
                Action::Leave,
                true,
                strings().leave,
            );
            y += 36.0;
        }

        out.heading(strings().available_groups, x, y + 2.0);
        y += 14.0;

        let create_y = bottom - 16.0;
        let list_bottom = create_y - if self.message.is_some() { 14.0 } else { 4.0 };
        let list_h = (list_bottom - y).max(ROW + 2.0);
        out.fill([x, y, w, list_h], 0xFFA0_A0A0);
        out.fill([x + 1.0, y + 1.0, w - 2.0, list_h - 2.0], 0xF018_181C);
        let fits = (((list_h - 2.0) / ROW).floor().max(1.0)) as usize;
        let scroll = self.scroll.min(view.groups.len().saturating_sub(fits));

        if view.groups.is_empty() {
            out.sprite(
                IC_GROUPS,
                [0.0, 0.0, 16.0, 16.0],
                [x + (w - 24.0) / 2.0, y + list_h / 2.0 - 22.0, 24.0, 24.0],
            );
            let empty = out.fit(strings().no_groups, w - 16.0);
            out.text_sh(
                &empty,
                [x + (w - out.width(&empty)) / 2.0, y + list_h / 2.0 + 10.0],
                0xAAAAAA,
            );
        }

        for (i, g) in view.groups.iter().skip(scroll).take(fits).enumerate() {
            let ry = y + 1.0 + i as f32 * ROW;
            let row = [x + 1.0, ry, w - 2.0, ROW];
            if g.ours {
                out.fill(row, 0x403F_7F46);
            } else if inside(row, out.mouse) {
                out.fill(row, 0x303A_3A44);
            }
            out.fill([x + 1.0, ry + ROW - 1.0, w - 2.0, 1.0], 0x20FF_FFFF);
            out.sprite(
                if g.has_password { IC_LOCK } else { IC_GROUPS },
                [0.0, 0.0, 16.0, 16.0],
                [x + 7.0, ry + 7.0, 16.0, 16.0],
            );
            if self.joining == Some(g.id) {
                let title = out.fit(&fmt(strings().password_for, &[&g.name]), w - 34.0);
                out.text_sh(&title, [x + 28.0, ry + 3.0], 0xFFFFFF);
                let fw = w - 138.0;
                out.field(
                    [x + 28.0, ry + 15.0, fw, 12.0],
                    &self.join_password,
                    strings().password,
                    true,
                    self.focus == Some(Field::JoinPassword),
                    Action::Focus(Field::JoinPassword),
                );
                out.button(
                    [x + w - 104.0, ry + 14.0, 62.0, 14.0],
                    strings().cancel,
                    Action::CancelJoin,
                    None,
                );
                out.icon_square(
                    x + w - 30.0,
                    ry + 3.0,
                    IC_JOIN,
                    Action::JoinWithPassword,
                    true,
                    strings().join,
                );
            } else {
                let name_colour = if g.ours { 0x55FF55 } else { 0xFFFFFF };
                out.text_sh(
                    &out.fit(&g.name, w - 88.0),
                    [x + 28.0, ry + 3.0],
                    name_colour,
                );
                let members = if g.members.is_empty() {
                    strings().nobody.to_owned()
                } else {
                    g.members.join(", ")
                };
                out.text_sh(
                    &out.fit(&members, w - 88.0),
                    [x + 28.0, ry + 15.0],
                    0xAAAAAA,
                );
                if !g.ours {
                    out.icon_square(
                        x + w - 30.0,
                        ry + 3.0,
                        IC_JOIN,
                        Action::Join(g.id),
                        true,
                        strings().join,
                    );
                }
            }
        }

        if let Some((message, rgb)) = &self.message {
            out.text(&out.fit(message, w), [x, list_bottom + 4.0], ink(*rgb));
        }
        out.button(
            [x, create_y, w, 16.0],
            strings().new_group,
            Action::ShowCreate,
            None,
        );
    }

    fn draw_group_create(&self, out: &mut Out<'_>, x: f32, top: f32, w: f32, bottom: f32) {
        out.settings_card([x, top, w, 148.0], strings().new_group);
        let ix = x + 8.0;
        let iw = w - 16.0;
        out.text(strings().name, [ix, top + 24.0], INK_SOFT);
        out.field(
            [ix, top + 34.0, iw, 16.0],
            &self.name,
            strings().group_name_hint,
            false,
            self.focus == Some(Field::Name),
            Action::Focus(Field::Name),
        );
        out.text(strings().password_optional, [ix, top + 60.0], INK_SOFT);
        out.field(
            [ix, top + 70.0, iw, 16.0],
            &self.password,
            strings().group_password_hint,
            true,
            self.focus == Some(Field::Password),
            Action::Focus(Field::Password),
        );
        out.text(strings().group_type, [ix, top + 96.0], INK_SOFT);
        let (kind, description) = match self.kind {
            0 => (strings().kind_normal, strings().kind_normal_desc),
            1 => (strings().kind_open, strings().kind_open_desc),
            _ => (strings().kind_isolated, strings().kind_isolated_desc),
        };
        out.button(
            [ix, top + 106.0, iw, 16.0],
            &format!("\u{2039} {} \u{203A}", fmt(strings().type_label, &[kind])),
            Action::Kind,
            None,
        );
        out.text(&out.fit(description, iw), [ix, top + 128.0], INK_SOFT);
        if let Some((message, rgb)) = &self.message {
            out.text(&out.fit(message, w), [x, top + 158.0], ink(*rgb));
        }
        let half = (w - 6.0) / 2.0;
        out.button(
            [x, bottom - 16.0, half, 16.0],
            strings().cancel,
            Action::CancelCreate,
            None,
        );
        out.button(
            [x + half + 6.0, bottom - 16.0, half, 16.0],
            strings().create,
            Action::Create,
            None,
        );
    }
}
