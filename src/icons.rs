//! Pixel-art icons: small bitmaps drawn with fills, one GUI pixel (or more) per bitmap pixel, so
//! they look like the game's own sprites at every GUI scale and need no texture.
//!
//! In a bitmap, `#` is the icon's colour, `+` the same colour at half strength, anything else is
//! empty.

use crate::modules::Id;

/// The icons there are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Module(Id),
    Logo,
    Gear,
    Search,
    Back,
    PadCross,
    PadCircle,
    PadSquare,
    PadTriangle,
}

/// The bitmap of `icon`, rows top to bottom.
pub fn bitmap(icon: Icon) -> &'static [&'static str] {
    match icon {
        // A speedometer, its needle up and right.
        Icon::Module(Id::Fps) => &[
            "....####....",
            "..##....##..",
            ".#........#.",
            ".#......+.#.",
            "#......+...#",
            "#.....+....#",
            "#....+.....#",
            "#...##.....#",
            "#...##.....#",
            ".#........#.",
            "..########..",
        ],
        // A mouse, its left button pressed.
        Icon::Module(Id::Cps) => &[
            "...######...",
            "..#####..#..",
            "..#####..#..",
            "..#####..#..",
            "..########..",
            "..#......#..",
            "..#......#..",
            "..#......#..",
            "...#....#...",
            "....####....",
        ],
        // W over A S D, and the space bar.
        Icon::Module(Id::Keystrokes) => &[
            "....###.....",
            "....###.....",
            "....###.....",
            "............",
            "###.###.###.",
            "###.###.###.",
            "###.###.###.",
            "............",
            "###########.",
            "+++++++++++.",
        ],
        // A map pin.
        Icon::Module(Id::Coordinates) => &[
            "....####....",
            "..########..",
            "..###..###..",
            ".###....###.",
            ".###....###.",
            "..###..###..",
            "..########..",
            "...######...",
            "....####....",
            ".....##.....",
            ".....##.....",
        ],
        // A lightning bolt.
        Icon::Module(Id::Speed) => &[
            "......####..",
            ".....####...",
            "....####....",
            "...####.....",
            "..########..",
            "......###...",
            ".....###....",
            "....###.....",
            "...##.......",
            "..#.........",
        ],
        // A chestplate.
        Icon::Module(Id::Armor) => &[
            ".###....###.",
            "####+..+####",
            "############",
            ".##########.",
            "..########..",
            "..##++++##..",
            "..########..",
            "..########..",
            "..###..###..",
        ],
        // A potion.
        Icon::Module(Id::Effects) => &[
            "....####....",
            ".....##.....",
            ".....##.....",
            "....#..#....",
            "...#....#...",
            "..#......#..",
            "..#++++++#..",
            "..#++++++#..",
            "..#++++++#..",
            "...#++++#...",
            "....####....",
        ],
        // A clock face at three o'clock.
        Icon::Module(Id::Clock) => &[
            "....####....",
            "..##....##..",
            ".#....#...#.",
            ".#....#...#.",
            "#.....#....#",
            "#.....####.#",
            "#..........#",
            ".#........#.",
            ".#........#.",
            "..##....##..",
            "....####....",
        ],
        // Two chevrons, forward.
        Icon::Module(Id::ToggleSprint) => &[
            ".##....##...",
            "..##....##..",
            "...##....##.",
            "....##....##",
            "...##....##.",
            "..##....##..",
            ".##....##...",
        ],
        // Two chevrons, down.
        Icon::Module(Id::ToggleSneak) => &[
            "#.........#",
            ".#.......#.",
            "..#.....#..",
            "...#...#...",
            "....#.#....",
            ".....#.....",
            "#.........#",
            ".#.......#.",
            "..#.....#..",
            "...#...#...",
            "....#.#....",
            ".....#.....",
        ],
        // A magnifying glass.
        Icon::Module(Id::Zoom) => &[
            "..####......",
            ".#....#.....",
            "#..++..#....",
            "#.+....#....",
            "#......#....",
            ".#....#.....",
            "..#####.....",
            "......###...",
            ".......###..",
            "........###.",
        ],
        // The sun setting on the horizon.
        Icon::Logo => &[
            "....####....",
            "..########..",
            ".##########.",
            "############",
            "............",
            "############",
            "............",
            "..++++++++..",
        ],
        Icon::Gear => &[
            "..#.#..", //
            ".#####.", //
            "##...##", //
            ".#...#.", //
            "##...##", //
            ".#####.", //
            "..#.#..",
        ],
        Icon::Search => &[
            ".###...", //
            "#...#..", //
            "#...#..", //
            "#...#..", //
            ".####..", //
            "....##.", //
            ".....##",
        ],
        Icon::PadCross => &[
            "#...#", //
            ".#.#.", //
            "..#..", //
            ".#.#.", //
            "#...#",
        ],
        Icon::PadCircle => &[
            ".###.", //
            "#...#", //
            "#...#", //
            "#...#", //
            ".###.",
        ],
        Icon::PadSquare => &[
            "#####", //
            "#...#", //
            "#...#", //
            "#...#", //
            "#####",
        ],
        Icon::PadTriangle => &[
            "..#..", //
            ".#.#.", //
            ".#.#.", //
            "#...#", //
            "#####",
        ],
        Icon::Back => &[
            "...#...", //
            "..##...", //
            ".######", //
            "#######", //
            ".######", //
            "..##...", //
            "...#...",
        ],
    }
}

/// The size of `icon` in bitmap pixels, width then height.
pub fn size(icon: Icon) -> [f32; 2] {
    let rows = bitmap(icon);
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    [w as f32, rows.len() as f32]
}

/// Draws `icon` with its top left at `at`, `px` GUI pixels a bitmap pixel, in `argb`, through
/// `fill`. Runs of one kind in a row are one fill.
pub fn draw(icon: Icon, at: [f32; 2], px: f32, argb: u32, fill: &mut dyn FnMut([f32; 4], u32)) {
    let half = ((argb >> 24) / 2) << 24 | (argb & 0x00FF_FFFF);
    for (y, row) in bitmap(icon).iter().enumerate() {
        let bytes = row.as_bytes();
        let mut x = 0;
        while x < bytes.len() {
            let kind = bytes[x];
            let mut end = x + 1;
            while end < bytes.len() && bytes[end] == kind {
                end += 1;
            }
            let colour = match kind {
                b'#' => Some(argb),
                b'+' => Some(half),
                _ => None,
            };
            if let Some(c) = colour {
                fill(
                    [
                        at[0] + x as f32 * px,
                        at[1] + y as f32 * px,
                        (end - x) as f32 * px,
                        px,
                    ],
                    c,
                );
            }
            x = end;
        }
    }
}

/// Draws `icon` centred in `r`, as large as fits in whole GUI pixels with a margin, at least 1.
pub fn draw_in(icon: Icon, r: [f32; 4], argb: u32, fill: &mut dyn FnMut([f32; 4], u32)) {
    let [w, h] = size(icon);
    let px = ((r[2] - 4.0) / w).min((r[3] - 4.0) / h).floor().max(1.0);
    let at = [
        r[0] + ((r[2] - w * px) / 2.0).floor(),
        r[1] + ((r[3] - h * px) / 2.0).floor(),
    ];
    draw(icon, at, px, argb, fill);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::defaults;

    #[test]
    fn every_module_has_an_icon_with_even_rows() {
        let mut icons: Vec<Icon> = defaults().iter().map(|m| Icon::Module(m.id)).collect();
        icons.extend([
            Icon::Logo,
            Icon::Gear,
            Icon::Search,
            Icon::Back,
            Icon::PadCross,
            Icon::PadCircle,
            Icon::PadSquare,
            Icon::PadTriangle,
        ]);
        for icon in icons {
            let rows = bitmap(icon);
            assert!(!rows.is_empty());
            assert!(
                rows.iter().all(|r| r.len() == rows[0].len()),
                "{icon:?} has rows of different widths"
            );
            assert!(rows.iter().any(|r| r.contains('#')));
        }
    }

    #[test]
    fn runs_become_single_fills() {
        let mut fills = Vec::new();
        draw(Icon::Gear, [10.0, 20.0], 2.0, 0xFFFFFFFF, &mut |r, _| {
            fills.push(r)
        });
        // The gear's second row, ".#####.", is one fill five pixels wide.
        assert!(fills.contains(&[12.0, 22.0, 10.0, 2.0]));
        let mut fills = Vec::new();
        draw_in(
            Icon::Gear,
            [0.0, 0.0, 14.0, 14.0],
            0xFFFFFFFF,
            &mut |r, _| fills.push(r),
        );
        // 7 pixels in 14 less the margin: 1 GUI pixel each, centred.
        assert!(fills.iter().all(|r| r[0] >= 3.0 && r[0] + r[2] <= 11.0));
    }
}
