//! The cosmetics page: the catalog by category, everything free to wear. A tile's picture is
//! the item's own art (`cosmetics/art.rs`); what is worn comes from the catalog's state.

use super::{Gui, LINE, Paint, Target, theme};
use crate::cosmetics::{self, Category, art};

const TILE: f32 = 24.0;
const GAP: f32 = 4.0;

/// Draws one tile's art centred in 18 GUI pixels, as large as it fits.
fn draw_art(p: &mut Paint<'_>, at: [f32; 2], a: &art::Art) {
    let h = a.rows.len();
    let w = a.rows.first().map_or(0, |r| r.chars().count());
    if w == 0 || h == 0 {
        return;
    }
    let s = (18.0_f32 / w.max(h) as f32).min(2.0);
    let x0 = at[0] + (18.0 - w as f32 * s) / 2.0;
    let y0 = at[1] + (18.0 - h as f32 * s) / 2.0;
    for (y, row) in a.rows.iter().enumerate() {
        let bytes = row.as_bytes();
        let mut x = 0;
        while x < bytes.len() {
            let kind = bytes[x];
            let mut end = x + 1;
            while end < bytes.len() && bytes[end] == kind {
                end += 1;
            }
            if let Some((_, c)) = a.colours.iter().find(|(k, _)| *k as u8 == kind) {
                let [r, g, b] = *c;
                p.fill(
                    [
                        x0 + x as f32 * s,
                        y0 + y as f32 * s,
                        (end - x) as f32 * s,
                        s,
                    ],
                    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32,
                );
            }
            x = end;
        }
    }
}

impl Gui {
    pub(super) fn draw_cosmetics(
        &mut self,
        p: &mut Paint<'_>,
        area: [f32; 4],
        hover: &dyn Fn(Target) -> bool,
    ) {
        let [ax, ay, aw, ah] = area;
        let top = ay - self.scroll.round();
        p.text("Cosmetics", [ax, top + 2.0], theme::TEXT);
        let about = p.wrap(
            "Everything here is free: click a tile to wear it, click it again to take it \
             off. One item per slot, so ears take a hat's place, and a helmet or a cape \
             covers either while you wear one.",
            aw,
            3,
        );
        for (n, line) in about.iter().enumerate() {
            p.text(line, [ax, top + 14.0 + n as f32 * 10.0], theme::TEXT_DIM);
        }
        let mut y = top + 18.0 + about.len() as f32 * 10.0;

        // The module's switch: with it off, nothing is worn and the tiles don't answer.
        if !self.cosmetics_on {
            p.text(
                "The Cosmetics module is off: switch it on, on its card, to wear these.",
                [ax, y + 2.0],
                theme::WARN,
            );
        }

        // The category tabs, as wide as they come, wrapped.
        let mut tx = ax;
        let mut ty = y + if self.cosmetics_on { 14.0 } else { 24.0 };
        for category in Category::ALL {
            let w = p.width(category.label()) + 14.0;
            if tx > ax && tx + w > ax + aw {
                tx = ax;
                ty += 20.0;
            }
            let r = [tx, ty, w, 16.0];
            let t = Target::CosmeticTab(category);
            p.button(
                r,
                category.label(),
                hover(t),
                self.cosmetics_tab == category,
            );
            self.hit(r, t);
            tx += w + GAP;
        }
        y = ty + 20.0;

        // The tiles: nothing on the slot first, then the category's items.
        let slot = self.cosmetics_tab.slot();
        let worn = cosmetics::worn();
        let worn_here = worn.iter().find(|(s, _)| *s == slot).map(|(_, c)| c.id);
        let items: Vec<(usize, &cosmetics::Cosmetic)> = cosmetics::CATALOG
            .iter()
            .enumerate()
            .filter(|(_, c)| c.category == self.cosmetics_tab)
            .collect();
        let per_row = (((aw + GAP) / (TILE + GAP)).floor() as usize).max(1);
        let mut hovered: Option<String> = None;
        for row_start in (0..=items.len()).step_by(per_row) {
            for k in 0..per_row {
                let i = row_start + k; // 0 is the none tile, 1.. the category's items.
                if i > items.len() {
                    break;
                }
                let r = [
                    ax + k as f32 * (TILE + GAP),
                    y + (i / per_row) as f32 * (TILE + GAP),
                    TILE,
                    TILE,
                ];
                let which = i.checked_sub(1).map(|n| items[n].0);
                let (worn_tile, name) = match which {
                    None => (worn_here.is_none(), "Nothing on this slot".to_owned()),
                    Some(index) => {
                        let c = &cosmetics::CATALOG[index];
                        (worn_here == Some(c.id), c.name.to_owned())
                    }
                };
                let t = Target::Cosmetic(which);
                p.rounded(r, theme::CARD);
                let at = [r[0] + (TILE - 18.0) / 2.0, r[1] + (TILE - 18.0) / 2.0];
                match which {
                    None => {
                        // Nothing on the slot: a crossed-out box, as the capes' none tile.
                        let c = theme::TEXT_FAINT;
                        p.outline([at[0] + 3.0, at[1] + 1.0, 12.0, 16.0], c);
                        for k in 0..12 {
                            p.fill(
                                [
                                    at[0] + 3.0 + k as f32,
                                    at[1] + 1.0 + k as f32 * 1.33,
                                    1.0,
                                    1.0,
                                ],
                                c,
                            );
                        }
                    }
                    Some(index) => draw_art(p, at, art_of(cosmetics::CATALOG[index].id)),
                }
                if hover(t) {
                    p.outline(r, theme::TEXT_DIM);
                    hovered = Some(name);
                }
                if worn_tile && self.cosmetics_on {
                    p.outline(r, theme::ACCENT);
                    p.outline(
                        [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0],
                        theme::ACCENT,
                    );
                }
                self.hit(r, t);
            }
        }
        let rows = (items.len() + 1).div_ceil(per_row);
        let y = y + rows as f32 * (TILE + GAP) + 4.0;
        if let Some(name) = hovered {
            p.text(&name, [ax, y], theme::TEXT);
        }
        let total = y + LINE - ay + self.scroll.round();
        self.max_scroll = (total - ah).max(0.0);
        self.scroll = self.scroll.clamp(0.0, self.max_scroll);
    }
}

/// The tile art of the cosmetic whose id this is. The catalog and the art file are
/// generated together (`tools/cosmetics.py`); the ids line up with the consts, whose names
/// are the ids' tails.
fn art_of(id: &str) -> &'static art::Art {
    match id.strip_prefix("qol_").unwrap_or(id) {
        "straw_hat" => &art::STRAW_HAT,
        "top_hat" => &art::TOP_HAT,
        "crown" => &art::CROWN,
        "halo" => &art::HALO,
        "cat_ears" => &art::CAT_EARS,
        "bunny_ears" => &art::BUNNY_EARS,
        "horns" => &art::HORNS,
        "angel_wings" => &art::ANGEL_WINGS,
        "dragon_wings" => &art::DRAGON_WINGS,
        "backpack" => &art::BACKPACK,
        "ember_ring" => &art::EMBER_RING,
        "stardust" => &art::STARDUST,
        "sunset_badge" => &art::SUNSET_BADGE,
        "little_sun" => &art::LITTLE_SUN,
        _ => &art::HALO,
    }
}
