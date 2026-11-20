use super::{Action, Draw, Widget};

// The designer's panels (their opaque content is the top-left WxH of each
// 256x256 sheet). The main menu is nine-sliced up to 224x72 so the status
// line fits; the sub-screens stretch a panel to the room they need, nine-
// sliced so the bevel and corner rivets stay crisp.
pub(super) const PANEL_MAIN: (&str, f32, f32) =
    ("horizon_qol:gui/voice/gui_voicechat", 195.0, 76.0);
pub(super) const MAIN_SIZE: (f32, f32) = (224.0, 72.0);
pub(super) const MAIN_BORDER: f32 = 8.0;
pub(super) const PANEL_SUB: (&str, f32, f32) =
    ("horizon_qol:gui/voice/gui_voicechat_settings", 248.0, 198.0);
pub(super) const SUB_BORDER: f32 = 8.0;

// The designer's activation meter: a 12-segment VU strip (green through red)
// in the corner of a 256x256 sheet. The strip's own background is (26, 26, 26).
pub(super) const TRACK: &str = "horizon_qol:gui/voice/voice_activation_slider";
pub(super) const SLIDER_HANDLE: &str = "gui/sprites/widget/slider_handle";
pub(super) const SLIDER_HANDLE_HOVER: &str = "gui/sprites/widget/slider_handle_highlighted";
pub(super) const VU_BG: u32 = 0xFF1A_1A1A;
pub(super) const VU_FRAME: u32 = 0xFF10_1014;

// The 16x16 icon glyphs in the resource pack.
pub(super) const IC_MIC_ON: &str = "horizon_qol:gui/voice/icon_microphone";
pub(super) const IC_MIC_OFF: &str = "horizon_qol:gui/voice/icon_mute";
pub(super) const IC_SPK_ON: &str = "horizon_qol:gui/voice/icon_speaker_on";
pub(super) const IC_SPK_OFF: &str = "horizon_qol:gui/voice/icon_speaker_off";
pub(super) const IC_PTT: &str = "horizon_qol:gui/voice/icon_ptt";
pub(super) const IC_VOICE: &str = "horizon_qol:gui/voice/icon_voice";
pub(super) const IC_SETTINGS: &str = "horizon_qol:gui/voice/icon_settings";
pub(super) const IC_GROUPS: &str = "horizon_qol:gui/voice/icon_group";
pub(super) const IC_VOLUMES: &str = "horizon_qol:gui/voice/icon_adjust_volumes";
pub(super) const IC_PLAYER_VOLUME: &str = "horizon_qol:gui/voice/icon_player_volume";
pub(super) const IC_LOCK: &str = "horizon_qol:gui/voice/icon_lock";
pub(super) const IC_JOIN: &str = "horizon_qol:gui/voice/icon_join";
pub(super) const IC_LEAVE: &str = "horizon_qol:gui/voice/icon_leave";

// Readable ink on the designer's light-grey panel (no drop shadow).
pub(super) const INK: u32 = 0x3F_3F3F;
pub(super) const INK_SOFT: u32 = 0x55_5555;
pub(super) const INK_FAINT: u32 = 0x6F_6F6F;
pub(super) const TITLE_INK: u32 = 0x3F_3F3F;

pub(super) const CARD_OUT: u32 = 0xFFA0_A0A0;
pub(super) const CARD_BG: u32 = 0x10FF_FFFF;
pub(super) const CARD_HEAD: u32 = 0x1000_0000;
pub(super) const CARD_LINE: u32 = 0xFFA0_A0A0;
pub(super) const CARD_LIGHT: u32 = 0x80FF_FFFF;
pub(super) const KEY_BG: u32 = 0xFFD6_D6D6;
pub(super) const KEY_OUT: u32 = 0xFF55_5555;

// The design book's button palette: black outline, grey face, white/dark bevel.
pub(super) const BTN_OUT: u32 = 0xFF00_0000;
pub(super) const BTN_FACE: u32 = 0xFF8B_8B8B;
pub(super) const BTN_FACE_HOVER: u32 = 0xFF7D_8FC9;
pub(super) const BTN_TOP: u32 = 0xFFAA_AAAA;
pub(super) const BTN_BOT: u32 = 0xFF55_5555;
pub(super) const BTN_INK: u32 = 0xFFFF_FFFF;
pub(super) const BTN_ON: u32 = 0xFF4A_B64E;
pub(super) const BTN_OFF: u32 = 0xFFC2_4234;

// A sunken text field.
pub(super) const FIELD_OUT: u32 = 0xFF10_1014;
pub(super) const FIELD_BG: u32 = 0xFF22_2226;
pub(super) const FIELD_FOCUS: u32 = 0xFFE6_E6F0;
pub(super) const FIELD_INK: u32 = 0xFFFF_FFFF;
pub(super) const FIELD_HINT: u32 = 0xFF7A_7A82;

/// A bright HUD colour darkened to read on the light-grey panel.
pub(super) fn ink(rgb: u32) -> u32 {
    let ch = |sh: u32| (((rgb >> sh) & 0xFF) as f32 * 0.52) as u32;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

pub(super) struct Out<'a> {
    pub(super) draws: Vec<Draw>,
    pub(super) widgets: Vec<Widget>,
    pub(super) mouse: [f32; 2],
    pub(super) measure: &'a dyn Fn(&str) -> f32,
    /// A tooltip to draw on top, set when the pointer is over a widget.
    pub(super) tip: Option<String>,
}

pub(super) fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

pub(super) fn slider_fraction(rect: [f32; 4], at: [f32; 2]) -> f32 {
    ((at[0] - rect[0] - 4.0) / (rect[2] - 8.0)).clamp(0.0, 1.0)
}

impl Out<'_> {
    pub(super) fn fill(&mut self, rect: [f32; 4], argb: u32) {
        self.draws.push(Draw::Fill(rect, argb));
    }
    /// Dark panel text, no drop shadow.
    pub(super) fn text(&mut self, text: &str, at: [f32; 2], rgb: u32) {
        self.draws.push(Draw::Text(text.to_owned(), at, rgb, false));
    }
    /// Light text with a shadow, for buttons and fields.
    pub(super) fn text_sh(&mut self, text: &str, at: [f32; 2], rgb: u32) {
        self.draws.push(Draw::Text(text.to_owned(), at, rgb, true));
    }
    pub(super) fn width(&self, text: &str) -> f32 {
        (self.measure)(text)
    }
    pub(super) fn sprite(&mut self, name: &'static str, src: [f32; 4], dst: [f32; 4]) {
        self.draws.push(Draw::Sprite { name, src, dst });
    }
    /// Nine-slice `name` into `dst`: corners stay 1:1, edges and centre
    /// stretch, so a panel keeps its bevel and rivets at any size.
    pub(super) fn nine_slice(
        &mut self,
        name: &'static str,
        dst: [f32; 4],
        sprite: (f32, f32),
        b: f32,
    ) {
        let [x, y, w, h] = dst;
        let (sw, sh) = sprite;
        let dx = [x, x + b, x + w - b];
        let dw = [b, (w - 2.0 * b).max(0.0), b];
        let dy = [y, y + b, y + h - b];
        let dh = [b, (h - 2.0 * b).max(0.0), b];
        let sx = [0.0, b, sw - b];
        let sww = [b, sw - 2.0 * b, b];
        let sy = [0.0, b, sh - b];
        let shh = [b, sh - 2.0 * b, b];
        for r in 0..3 {
            for c in 0..3 {
                self.sprite(
                    name,
                    [sx[c], sy[r], sww[c], shh[r]],
                    [dx[c], dy[r], dw[c], dh[r]],
                );
            }
        }
    }
    /// The designer's main panel nine-sliced into `dst`; a grey backing shows
    /// first so the menu is never invisible if the pack is missing.
    pub(super) fn panel_main(&mut self, dst: [f32; 4]) {
        let (name, w, h) = PANEL_MAIN;
        self.fill(dst, 0xFFC2_C2C2);
        let [x, y, dw, dh] = dst;
        self.fill([x, y, dw, 1.0], 0xFF1C_1C1C);
        self.fill([x, y + dh - 1.0, dw, 1.0], 0xFF1C_1C1C);
        self.fill([x, y, 1.0, dh], 0xFF1C_1C1C);
        self.fill([x + dw - 1.0, y, 1.0, dh], 0xFF1C_1C1C);
        self.nine_slice(name, dst, (w, h), MAIN_BORDER);
    }
    /// A designer panel stretched (nine-sliced) into `dst`, with a grey backing.
    pub(super) fn panel_sub(&mut self, dst: [f32; 4]) {
        let (name, w, h) = PANEL_SUB;
        self.fill(dst, 0xFFC2_C2C2);
        let [x, y, dw, dh] = dst;
        self.fill([x, y, dw, 1.0], 0xFF1C_1C1C);
        self.fill([x, y + dh - 1.0, dw, 1.0], 0xFF1C_1C1C);
        self.fill([x, y, 1.0, dh], 0xFF1C_1C1C);
        self.fill([x + dw - 1.0, y, 1.0, dh], 0xFF1C_1C1C);
        self.nine_slice(name, dst, (w, h), SUB_BORDER);
    }

    /// A 24x24 icon button with a 16x16 glyph centred in its bevel.
    pub(super) fn icon_square(
        &mut self,
        x: f32,
        y: f32,
        icon: &'static str,
        action: Action,
        on: bool,
        tip: &str,
    ) {
        let rect = [x, y, 24.0, 24.0];
        let hover = inside(rect, self.mouse);
        self.fill(rect, BTN_OUT);
        self.fill(
            [x + 1.0, y + 1.0, 22.0, 22.0],
            if hover { BTN_FACE_HOVER } else { BTN_FACE },
        );
        self.fill([x + 1.0, y + 1.0, 22.0, 1.0], BTN_TOP);
        self.fill([x + 1.0, y + 1.0, 1.0, 22.0], BTN_TOP);
        self.fill([x + 1.0, y + 22.0, 22.0, 1.0], BTN_BOT);
        self.sprite(icon, [0.0, 0.0, 16.0, 16.0], [x + 4.0, y + 4.0, 16.0, 16.0]);
        if !on {
            self.fill([x + 1.0, y + 1.0, 22.0, 22.0], 0x7020_2024);
        }
        if hover {
            self.tip = Some(tip.to_owned());
        }
        self.widgets.push(Widget { rect, action });
    }
    /// `text` cut to `max` pixels with an ellipsis.
    pub(super) fn fit(&self, text: &str, max: f32) -> String {
        if self.width(text) <= max {
            return text.to_owned();
        }
        let ellipsis = self.width("...");
        let bounds: Vec<usize> = text
            .char_indices()
            .map(|(i, _)| i)
            .chain([text.len()])
            .collect();
        let (mut low, mut high) = (0, bounds.len() - 1);
        while low < high {
            let mid = (low + high).div_ceil(2);
            if self.width(&text[..bounds[mid]]) + ellipsis <= max {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        format!("{}...", &text[..bounds[low]])
    }
    /// A drawn Minecraft-style button: bevelled face, highlighted on hover,
    /// a thin accent for a toggle's state (green on, red off).
    pub(super) fn button(
        &mut self,
        rect: [f32; 4],
        label: &str,
        action: Action,
        state: Option<bool>,
    ) {
        let [x, y, w, h] = rect;
        let hover = inside(rect, self.mouse);
        self.fill(rect, BTN_OUT);
        self.fill(
            [x + 1.0, y + 1.0, w - 2.0, h - 2.0],
            if hover { BTN_FACE_HOVER } else { BTN_FACE },
        );
        self.fill([x + 1.0, y + 1.0, w - 2.0, 1.0], BTN_TOP);
        self.fill([x + 1.0, y + h - 2.0, w - 2.0, 1.0], BTN_BOT);
        if let Some(on) = state {
            self.fill(
                [x + 2.0, y + h - 3.0, w - 4.0, 1.0],
                if on { BTN_ON } else { BTN_OFF },
            );
        }
        let label = self.fit(label, w - 6.0);
        let tw = self.width(&label);
        self.text_sh(&label, [x + (w - tw) / 2.0, y + (h - 8.0) / 2.0], BTN_INK);
        self.widgets.push(Widget { rect, action });
    }
    /// A text field showing `value` (dots for a password), focused or not.
    pub(super) fn field(
        &mut self,
        rect: [f32; 4],
        value: &str,
        hint: &str,
        secret: bool,
        focus: bool,
        action: Action,
    ) {
        let [x, y, w, h] = rect;
        self.fill(rect, if focus { FIELD_FOCUS } else { FIELD_OUT });
        self.fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], FIELD_BG);
        let shown = if secret {
            "*".repeat(value.chars().count())
        } else {
            value.to_owned()
        };
        let is_hint = shown.is_empty() && !focus;
        let (mut text, rgb) = if is_hint {
            (hint.to_owned(), FIELD_HINT)
        } else {
            (shown, FIELD_INK)
        };
        if is_hint {
            // A placeholder clips on the right; typed text scrolls from the left
            // so the caret stays in view.
            text = self.fit(&text, w - 6.0);
        } else {
            while !text.is_empty() && self.width(&text) > w - 10.0 {
                text.remove(0);
            }
        }
        let caret = if focus { "_" } else { "" };
        self.text_sh(
            &format!("{text}{caret}"),
            [x + 3.0, y + (h - 8.0) / 2.0],
            rgb,
        );
        self.widgets.push(Widget { rect, action });
    }
    /// `label: [-] value [+]` across `w`, on the light panel.
    pub(super) fn stepper(
        &mut self,
        at: [f32; 2],
        w: f32,
        label: &str,
        value: &str,
        down: Action,
        up: Action,
    ) {
        let [x, y] = at;
        self.text(label, [x, y + 2.0], INK);
        let right = x + w;
        self.button([right - 13.0, y, 13.0, 12.0], "+", up, None);
        let vw = 46.0;
        let vx = right - 15.0 - vw;
        let tw = self.width(value);
        self.text(value, [vx + (vw - tw) / 2.0, y + 2.0], INK_SOFT);
        self.button([vx - 15.0, y, 13.0, 12.0], "-", down, None);
    }
    /// A section heading on the panel.
    pub(super) fn heading(&mut self, text: &str, x: f32, y: f32) {
        self.text(text, [x, y], TITLE_INK);
    }
    pub(super) fn settings_card(&mut self, rect: [f32; 4], title: &str) {
        let [x, y, w, h] = rect;
        self.fill(rect, CARD_OUT);
        self.fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], CARD_BG);
        self.fill([x + 1.0, y + 1.0, w - 2.0, 14.0], CARD_HEAD);
        self.fill([x + 1.0, y + 1.0, w - 2.0, 1.0], CARD_LIGHT);
        self.fill([x + 1.0, y + 15.0, w - 2.0, 1.0], CARD_LINE);
        self.fill([x + 1.0, y + 16.0, w - 2.0, 1.0], CARD_LIGHT);
        self.text(title, [x + 6.0, y + 5.0], TITLE_INK);
    }

    pub(super) fn key_cap(
        &mut self,
        right: f32,
        y: f32,
        max_w: f32,
        label: &str,
        bound: bool,
    ) -> f32 {
        let w = (self.width(label) + 10.0).clamp(26.0, max_w);
        let x = right - w;
        self.fill([x, y - 2.0, w, 12.0], KEY_OUT);
        self.fill(
            [x + 1.0, y - 1.0, w - 2.0, 10.0],
            if bound { KEY_BG } else { CARD_BG },
        );
        let label = self.fit(label, w - 6.0);
        let tw = self.width(&label);
        self.text(
            &label,
            [x + (w - tw) / 2.0, y],
            if bound { INK } else { INK_FAINT },
        );
        w
    }
    /// `label  [current device ▾]` – the face opens the device list below it.
    pub(super) fn device_picker(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        label: &str,
        name: &str,
        open: Action,
    ) {
        self.text(label, [x, y + 3.0], INK);
        let nx = x + 88.0;
        let nw = w - 88.0;
        let rect = [nx, y, nw, 14.0];
        let hover = inside(rect, self.mouse);
        self.fill(rect, BTN_OUT);
        self.fill(
            [nx + 1.0, y + 1.0, nw - 2.0, 12.0],
            if hover { BTN_FACE_HOVER } else { BTN_FACE },
        );
        self.fill([nx + 1.0, y + 1.0, nw - 2.0, 1.0], BTN_TOP);
        self.fill([nx + 1.0, y + 12.0, nw - 2.0, 1.0], BTN_BOT);
        self.text_sh(&self.fit(name, nw - 16.0), [nx + 3.0, y + 3.0], BTN_INK);
        self.text_sh("\u{25BC}", [nx + nw - 10.0, y + 3.0], BTN_INK);
        self.widgets.push(Widget { rect, action: open });
    }
    /// The full VU strip sits under a smooth dark veil driven by the live
    /// microphone level. The vanilla slider handle marks the threshold.
    pub(super) fn vu_meter(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        level_db: f32,
        threshold: i32,
        dragging: bool,
    ) {
        let (dx, dy, dw, dh) = (x + 1.0, y + 1.0, w - 2.0, 8.0);
        let rect = [x, y, w, 10.0];
        self.fill(rect, VU_FRAME);
        self.fill([dx, dy, dw, dh], VU_BG);
        self.sprite(TRACK, [0.0, 0.0, 226.0, 18.0], [dx, dy, dw, dh]);

        let level = ((level_db + 80.0) / 80.0).clamp(0.0, 1.0);
        let vx = dx + level * dw;
        if vx < dx + dw {
            self.fill([vx, dy, dx + dw - vx, dh], 0x8010_1014);
        }

        let thr = ((threshold + 80) as f32 / 80.0).clamp(0.0, 1.0);
        let kx = x + thr * (w - 8.0);
        let hover = dragging || inside(rect, self.mouse);
        let handle_y = y - 3.0;
        self.fill([kx, handle_y, 8.0, 16.0], BTN_OUT);
        self.fill(
            [kx + 1.0, handle_y + 1.0, 6.0, 14.0],
            if hover { BTN_FACE_HOVER } else { BTN_FACE },
        );
        self.sprite(
            if hover {
                SLIDER_HANDLE_HOVER
            } else {
                SLIDER_HANDLE
            },
            [0.0, 0.0, 8.0, 20.0],
            [kx, handle_y, 8.0, 16.0],
        );
        self.widgets.push(Widget {
            rect,
            action: Action::ThresholdAt(0.0),
        });
    }

    pub(super) fn volume_slider(&mut self, rect: [f32; 4], uuid: u128, value: u32, dragging: bool) {
        let [x, y, w, h] = rect;
        let fraction = value.min(200) as f32 / 200.0;
        let track_y = y + (h - 6.0) / 2.0;
        self.fill([x, track_y, w, 6.0], BTN_OUT);
        self.fill([x + 1.0, track_y + 1.0, w - 2.0, 4.0], FIELD_BG);
        let fill_w = ((w - 2.0) * fraction).round();
        if fill_w > 0.0 {
            self.fill([x + 1.0, track_y + 1.0, fill_w, 4.0], BTN_FACE);
        }
        let kx = x + fraction * (w - 8.0);
        let hover = dragging || inside(rect, self.mouse);
        self.fill([kx, y, 8.0, h], BTN_OUT);
        self.fill(
            [kx + 1.0, y + 1.0, 6.0, h - 2.0],
            if hover { BTN_FACE_HOVER } else { BTN_FACE },
        );
        self.fill([kx + 1.0, y + 1.0, 6.0, 1.0], BTN_TOP);
        self.fill([kx + 1.0, y + h - 2.0, 6.0, 1.0], BTN_BOT);
        self.widgets.push(Widget {
            rect,
            action: Action::PlayerVolume(uuid, value),
        });
    }

    /// A small ▲/▼ button at the side of an open device list.
    pub(super) fn drop_arrow(&mut self, x: f32, y: f32, glyph: &str, action: Action) {
        let rect = [x, y, 9.0, 9.0];
        let hover = inside(rect, self.mouse);
        self.fill(rect, BTN_OUT);
        self.fill(
            [x + 1.0, y + 1.0, 7.0, 7.0],
            if hover { BTN_FACE_HOVER } else { BTN_FACE },
        );
        self.text_sh(glyph, [x + 2.0, y + 1.0], BTN_INK);
        self.widgets.push(Widget { rect, action });
    }
}

/// A tooltip box drawn on top near the pointer, kept on screen.
pub(super) fn draw_tooltip(out: &mut Out<'_>, text: &str, size: [f32; 2]) {
    let tw = out.width(text);
    let w = tw + 8.0;
    let h = 14.0;
    let mut x = out.mouse[0] + 12.0;
    let mut y = out.mouse[1] - 6.0;
    if x + w > size[0] {
        x = (out.mouse[0] - w - 6.0).max(2.0);
    }
    y = y.clamp(2.0, (size[1] - h - 2.0).max(2.0));
    out.fill([x - 1.0, y - 1.0, w + 2.0, h + 2.0], 0xF02B_2B36);
    out.fill([x, y, w, h], 0xF01A_1A20);
    out.text_sh(text, [x + 4.0, y + 3.0], 0xFFFFFF);
}
