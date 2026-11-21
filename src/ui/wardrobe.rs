//! The skin manager's page: a preview of your player, then your skins and capes as tiles.

use super::{Gui, LINE, Paint, Target, theme};
use crate::wardrobe::{self as w, Image};

/// What the skin manager shows, filled in by the mod each frame it is open.
#[derive(Clone, Debug, Default)]
pub struct WardrobeView {
    /// Imported skins and capes: their names and pixels.
    pub skins: Vec<(String, Option<Image>)>,
    pub capes: Vec<(String, Option<Image>)>,
    /// The account's capes: name, pixels, and whether it wears it.
    pub official: Vec<(String, Option<Image>, bool)>,
    /// The account's skin, and whether it is slim.
    pub account_skin: Option<Image>,
    pub account_slim: bool,
    /// The skin picked: an imported one, or the account's (None).
    pub skin: Option<usize>,
    /// The cape picked: an imported one, or the account's (None).
    pub cape: Option<usize>,
    /// The picked imported skin's arms are slim.
    pub slim: bool,
    /// The account, as a line: signed in as, offline, loading, or what went wrong.
    pub account: (String, u32),
    /// The account can be changed (a Microsoft sign-in).
    pub online: bool,
}

/// What the page asks the mod to do.
#[derive(Clone, Debug, PartialEq)]
pub enum WardrobeAction {
    /// Wear imported skin `i`, or the account's.
    Skin(Option<usize>),
    /// Wear imported cape `i`.
    CustomCape(usize),
    /// Wear the account's cape `i`, or none at all.
    OfficialCape(Option<usize>),
    Slim(bool),
    /// Make the picked imported skin the account's.
    Upload,
    /// Import the file whose path is on the clipboard.
    Paste,
    /// Read the account again.
    Refresh,
    /// Delete an imported skin or cape.
    Delete {
        cape: bool,
        index: usize,
    },
}

/// Draws tile `i` in its rectangle: what it is, whether it is picked, and its name.
type DrawTile<'a> = dyn FnMut(&mut Paint<'_>, usize, [f32; 4]) -> (Target, bool, String) + 'a;

const TILE: f32 = 24.0;
const GAP: f32 = 4.0;

/// Draws `px` (pixel positions in skin pixels, and colours) at `at`, `scale` GUI pixels each.
fn draw_pixels(p: &mut Paint<'_>, at: [f32; 2], scale: f32, px: &[([u32; 2], u32)]) {
    for ([x, y], c) in px {
        p.fill(
            [
                at[0] + *x as f32 * scale,
                at[1] + *y as f32 * scale,
                scale,
                scale,
            ],
            *c,
        );
    }
}

/// A skin's front view: 16 by 32 skin pixels, `scale` each.
fn draw_skin(p: &mut Paint<'_>, at: [f32; 2], scale: f32, img: Option<&Image>, slim: bool) {
    match img {
        Some(img) => {
            let legacy = img.h * 2 == img.w;
            for (part, to) in w::front(slim, legacy) {
                draw_pixels(p, at, scale, &w::pixels(img, part, to));
            }
        }
        None => {
            // A grey stand-in: head, body, arms and legs.
            let grey = 0xFF5C5C68;
            for [x, y, ww, hh] in [
                [4.0, 0.0, 8.0, 8.0],
                [4.0, 8.0, 8.0, 12.0],
                [0.0, 8.0, 4.0, 12.0],
                [12.0, 8.0, 4.0, 12.0],
                [4.0, 20.0, 8.0, 12.0],
            ] {
                p.fill(
                    [at[0] + x * scale, at[1] + y * scale, ww * scale, hh * scale],
                    grey,
                );
            }
        }
    }
}

/// A skin's head, 8 by 8 skin pixels with its hat.
fn draw_head(p: &mut Paint<'_>, at: [f32; 2], scale: f32, img: Option<&Image>) {
    match img {
        Some(img) => {
            draw_pixels(p, at, scale, &w::pixels(img, [8, 8, 8, 8], [0, 0]));
            draw_pixels(p, at, scale, &w::pixels(img, [40, 8, 8, 8], [0, 0]));
        }
        None => p.fill([at[0], at[1], 8.0 * scale, 8.0 * scale], 0xFF5C5C68),
    }
}

fn draw_cape(p: &mut Paint<'_>, at: [f32; 2], scale: f32, img: &Image) {
    draw_pixels(p, at, scale, &w::cape_pixels(img));
}

impl Gui {
    pub(super) fn draw_wardrobe(
        &mut self,
        p: &mut Paint<'_>,
        area: [f32; 4],
        mouse: [f32; 2],
        hover: &dyn Fn(Target) -> bool,
    ) {
        let [ax, ay, aw, ah] = area;
        let top = ay - self.scroll.round();
        let Some(v) = self.wardrobe.clone() else {
            p.text("Loading...", [ax, top + 2.0], theme::TEXT_DIM);
            return;
        };

        // The preview: the skin worn, and the cape beside it.
        let panel = [ax, top, 88.0, 162.0];
        p.rounded(panel, theme::ROW);
        let (skin, slim) = match v.skin.and_then(|i| v.skins.get(i)) {
            Some((_, img)) => (img.as_ref(), v.slim),
            None => (v.account_skin.as_ref(), v.account_slim),
        };
        draw_skin(p, [ax + 6.0, top + 6.0], 3.0, skin, slim);
        let cape = match v.cape {
            Some(i) => v.capes.get(i).and_then(|(_, img)| img.as_ref()),
            None => v
                .official
                .iter()
                .find(|(_, _, worn)| *worn)
                .and_then(|(_, img, _)| img.as_ref()),
        };
        if let Some(img) = cape {
            draw_cape(p, [ax + 56.0, top + 30.0], 3.0, img);
        }
        // Arms: only an imported skin's can be chosen (the account's are the account's).
        let custom = v.skin.is_some();
        let half = 39.0;
        for (i, (label, on)) in [("Wide", !v.slim), ("Slim", v.slim)].iter().enumerate() {
            let r = [ax + 4.0 + i as f32 * (half + 2.0), top + 108.0, half, 14.0];
            let t = Target::WardrobeSlim(i == 1);
            p.button(r, label, hover(t), custom && *on);
            if custom {
                self.hit(r, t);
            }
        }
        let upload = [ax + 4.0, top + 126.0, 80.0, 14.0];
        let can_upload = custom && v.online;
        p.button(
            upload,
            "Upload",
            can_upload && hover(Target::WardrobeUpload),
            can_upload,
        );
        if can_upload {
            self.hit(upload, Target::WardrobeUpload);
        }
        p.text_centred(
            if can_upload {
                "for everyone"
            } else {
                "only you see it"
            },
            ax,
            88.0,
            top + 146.0,
            theme::TEXT_FAINT,
        );

        // The tiles.
        let lx = ax + 96.0;
        let lw = aw - 96.0;
        let per_row = (((lw + GAP) / (TILE + GAP)).floor() as usize).max(1);
        let mut y = top;
        let mut hovered_name: Option<String> = None;
        let mut tiles =
            |p: &mut Paint<'_>, gui: &mut Gui, y: &mut f32, n: usize, draw: &mut DrawTile<'_>| {
                for i in 0..n {
                    let r = [
                        lx + (i % per_row) as f32 * (TILE + GAP),
                        *y + (i / per_row) as f32 * (TILE + GAP),
                        TILE,
                        TILE,
                    ];
                    let (t, picked, name) = {
                        // Draw over the tile's background: lay it first.
                        p.rounded(r, theme::CARD);
                        draw(p, i, r)
                    };
                    if hover(t) {
                        p.outline(r, theme::TEXT_DIM);
                        hovered_name = Some(name);
                    }
                    if picked {
                        p.outline(r, theme::ACCENT);
                        p.outline(
                            [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
                            theme::ACCENT,
                        );
                    }
                    gui.hit(r, t);
                }
                *y += n.div_ceil(per_row) as f32 * (TILE + GAP);
            };

        p.text("SKINS", [lx + 2.0, y + 2.0], theme::ACCENT);
        y += 13.0;
        let skins = v.skins.clone();
        tiles(p, self, &mut y, skins.len() + 1, &mut |p, i, r| {
            let at = [r[0] + 4.0, r[1] + 4.0];
            if i == 0 {
                draw_head(p, at, 2.0, v.account_skin.as_ref());
                p.text("A", [r[0] + r[2] - 6.0, r[1] + r[3] - 9.0], theme::TEXT);
                (
                    Target::WardrobeSkin(0),
                    v.skin.is_none(),
                    "Your account's skin".into(),
                )
            } else {
                let (name, img) = &skins[i - 1];
                draw_head(p, at, 2.0, img.as_ref());
                (
                    Target::WardrobeSkin(i),
                    v.skin == Some(i - 1),
                    w::shown_name(name),
                )
            }
        });

        y += 4.0;
        let capes_top = y;
        p.text("CAPES", [lx + 2.0, y + 2.0], theme::ACCENT);
        y += 13.0;
        let official = v.official.clone();
        let capes = v.capes.clone();
        let worn_none = v.cape.is_none() && !official.iter().any(|(_, _, worn)| *worn);
        tiles(
            p,
            self,
            &mut y,
            1 + official.len() + capes.len(),
            &mut |p, i, r| {
                if i == 0 {
                    // No cape: a crossed-out box.
                    let c = theme::TEXT_FAINT;
                    p.outline([r[0] + 6.0, r[1] + 4.0, 12.0, 16.0], c);
                    for k in 0..12 {
                        p.fill(
                            [
                                r[0] + 6.0 + k as f32,
                                r[1] + 4.0 + k as f32 * 1.33,
                                1.0,
                                1.0,
                            ],
                            c,
                        );
                    }
                    return (Target::WardrobeCape(0), worn_none, "No cape".into());
                }
                let at = [r[0] + 7.0, r[1] + 4.0];
                if i <= official.len() {
                    let (name, img, worn) = &official[i - 1];
                    if let Some(img) = img {
                        draw_cape(p, at, 1.0, img);
                    }
                    p.text("M", [r[0] + r[2] - 6.0, r[1] + r[3] - 9.0], theme::TEXT);
                    (
                        Target::WardrobeCape(i),
                        v.cape.is_none() && *worn,
                        format!("{name} (your account's: everyone sees it)"),
                    )
                } else {
                    let k = i - 1 - official.len();
                    let (name, img) = &capes[k];
                    if let Some(img) = img {
                        draw_cape(p, at, 1.0, img);
                    }
                    (
                        Target::WardrobeCape(i),
                        v.cape == Some(k),
                        format!("{} (only you see it)", w::shown_name(name)),
                    )
                }
            },
        );
        // A dropped file of either kind goes where it is dropped: over the capes, a cape.
        self.wardrobe_cape_zone = mouse[1] >= capes_top && mouse[0] >= lx;

        // The account, what was last done, and how to import.
        y += 6.0;
        let (account, colour) = &v.account;
        let line = p.fit(account, lw - 40.0);
        p.text(&line, [lx, y], *colour);
        let refresh = [lx + lw - 36.0, y - 2.0, 36.0, 12.0];
        p.button(refresh, "Reload", hover(Target::WardrobeRefresh), false);
        self.hit(refresh, Target::WardrobeRefresh);
        y += 13.0;
        let (status, colour) = match (&hovered_name, &self.note) {
            (Some(name), _) => (name.clone(), theme::TEXT),
            (None, Some((note, colour))) => (note.clone(), *colour),
            (None, None) => (
                "Right-click an imported one twice to delete it.".to_owned(),
                theme::TEXT_FAINT,
            ),
        };
        for line in p.wrap(&status, lw, 2) {
            p.text(&line, [lx, y], colour);
            y += LINE + 1.0;
        }
        y += 3.0;
        let hint = "Import: drop a PNG on the window, or copy the file and press Paste.";
        for line in p.wrap(hint, lw - 44.0, 2) {
            p.text(&line, [lx, y], theme::TEXT_DIM);
            y += LINE + 1.0;
        }
        let paste = [lx + lw - 40.0, y - 2.0 * (LINE + 1.0) - 1.0, 40.0, 14.0];
        p.button(paste, "Paste", hover(Target::WardrobePaste), true);
        self.hit(paste, Target::WardrobePaste);

        let total = (y.max(top + 162.0) + 4.0) + self.scroll.round() - ay;
        self.max_scroll = (total - ah).max(0.0);
        self.scroll = self.scroll.clamp(0.0, self.max_scroll);
    }

    /// A press on the skin manager's page: what it asks for, when it does.
    pub(super) fn wardrobe_press(&mut self, target: Target, button: i32) -> bool {
        let Some(v) = self.wardrobe.as_ref() else {
            return false;
        };
        let official = v.official.len();
        // Right-click twice on an imported one deletes it.
        if button == 1 {
            let which = match target {
                Target::WardrobeSkin(i) if i > 0 => Some((false, i - 1)),
                Target::WardrobeCape(i) if i > official => Some((true, i - 1 - official)),
                _ => None,
            };
            let Some((cape, index)) = which else {
                return false;
            };
            if self.wardrobe_confirm == Some((cape, index)) {
                self.wardrobe_action = Some(WardrobeAction::Delete { cape, index });
                self.wardrobe_confirm = None;
            } else {
                self.wardrobe_confirm = Some((cape, index));
                let name = if cape {
                    &v.capes[index].0
                } else {
                    &v.skins[index].0
                };
                self.note = Some((
                    format!("Right-click {} again to delete it.", w::shown_name(name)),
                    theme::WARN,
                ));
            }
            return true;
        }
        if button != 0 {
            return false;
        }
        self.wardrobe_confirm = None;
        self.wardrobe_action = Some(match target {
            Target::WardrobeSkin(0) => WardrobeAction::Skin(None),
            Target::WardrobeSkin(i) => WardrobeAction::Skin(Some(i - 1)),
            Target::WardrobeCape(0) => WardrobeAction::OfficialCape(None),
            Target::WardrobeCape(i) if i <= official => WardrobeAction::OfficialCape(Some(i - 1)),
            Target::WardrobeCape(i) => WardrobeAction::CustomCape(i - 1 - official),
            Target::WardrobeSlim(slim) => WardrobeAction::Slim(slim),
            Target::WardrobeUpload => WardrobeAction::Upload,
            Target::WardrobePaste => WardrobeAction::Paste,
            Target::WardrobeRefresh => WardrobeAction::Refresh,
            _ => return false,
        });
        true
    }
}
