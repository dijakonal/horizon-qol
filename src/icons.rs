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
    Gear,
    Search,
    Back,
    PadCross,
    PadCircle,
    PadSquare,
    PadTriangle,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

/// The mod's logo, a sunset on the sea: 16 by 16, one letter a colour ([`LOGO_COLOURS`]), `.`
/// empty. `icon.png` is drawn from these rows (`tools/logo.py`), so the two stay the same.
pub const LOGO_ART: [&str; 16] = [
    ".aaaaaaaaaaaaaa.",
    "aaaaaaaaaaaaaaaa",
    "bbbbbbbbbbbbbbbb",
    "bbbbbbyyyybbbbbb",
    "ccccyyyyyyyycccc",
    "cccyyyyyyyyyyccc",
    "ccyyyyyyyyyyyycc",
    "dddddddddddddddd",
    "dooooooooooooood",
    "dddddddddddddddd",
    "drrrrrrrrrrrrrrd",
    "dddddddddddddddd",
    "wwwwwwwwwwwwwwww",
    "wwwllllllllllwww",
    "wwwwwllllllwwwww",
    ".wwwwwwllwwwwww.",
];

/// The logo's colours, by letter.
pub const LOGO_COLOURS: [(u8, u32); 9] = [
    (b'a', 0xFF2B1055),
    (b'b', 0xFF6A2C70),
    (b'c', 0xFFD25D6E),
    (b'd', 0xFFF08A5D),
    (b'y', 0xFFFFD95A),
    (b'o', 0xFFFF9A3C),
    (b'r', 0xFFF25F5C),
    (b'w', 0xFF1B2A4A),
    (b'l', 0xFFFF9A3C),
];

/// Draws the logo with its top left at `at`, `px` GUI pixels a logo pixel, through `fill`, its
/// colours' alpha times `alpha`.
pub fn draw_logo(at: [f32; 2], px: f32, alpha: f32, fill: &mut dyn FnMut([f32; 4], u32)) {
    for (y, row) in LOGO_ART.iter().enumerate() {
        let bytes = row.as_bytes();
        let mut x = 0;
        while x < bytes.len() {
            let kind = bytes[x];
            let mut end = x + 1;
            while end < bytes.len() && bytes[end] == kind {
                end += 1;
            }
            if let Some(&(_, c)) = LOGO_COLOURS.iter().find(|(k, _)| *k == kind) {
                let a = (((c >> 24) as f32) * alpha.clamp(0.0, 1.0)).round() as u32;
                fill(
                    [
                        at[0] + x as f32 * px,
                        at[1] + y as f32 * px,
                        (end - x) as f32 * px,
                        px,
                    ],
                    (a << 24) | (c & 0x00FF_FFFF),
                );
            }
            x = end;
        }
    }
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
        // A sidebar: the title band, then lines with their scores.
        Icon::Module(Id::Scoreboard) => &[
            "############",
            "############",
            "............",
            "#####....##.",
            "............",
            "######...##.",
            "............",
            "####.....##.",
            "............",
            "#######..##.",
            "............",
        ],
        // A camera.
        Icon::Module(Id::Freecam) => &[
            "............",
            "...###......",
            ".##########.",
            "#..........#",
            "#...####...#",
            "#..#....#..#",
            "#..#....#..#",
            "#...####...#",
            "#..........#",
            ".##########.",
            "............",
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
        // A shulker box: a lid over a body, the shell's gap between.
        Icon::Module(Id::ShulkerPreview) => &[
            ".##########.",
            "############",
            "############",
            "#####..#####",
            "++++++++++++",
            "############",
            "#####..#####",
            "############",
            "############",
            ".##########.",
        ],
        // An ender chest: a chest with its eye on the lock.
        Icon::Module(Id::EnderPreview) => &[
            "############",
            "#++++++++++#",
            "#++++++++++#",
            "############",
            "#####..#####",
            "#####..#####",
            "#++++++++++#",
            "#++++++++++#",
            "#++++++++++#",
            "############",
        ],
        // A microphone on its stand.
        Icon::Module(Id::VoiceChat) => &[
            "....####....",
            "...######...",
            "...######...",
            "...######...",
            ".#.######.#.",
            ".#.######.#.",
            ".#..####..#.",
            "..#......#..",
            "...######...",
            ".....##.....",
            "...######...",
        ],
        // A screen on its stand: the HUD.
        Icon::Module(Id::Hud) => &[
            "############",
            "#++++++++++#",
            "#+##+++++#+#",
            "#++++++++++#",
            "#++++++++++#",
            "#+###++++++#",
            "############",
            ".....##.....",
            "...######...",
        ],
        // A microphone with waves either side: your voice.
        Icon::Module(Id::VoiceStatus) => &[
            ".....##.....",
            "....####....",
            "#...####...#",
            "#.#.####.#.#",
            "#.#.####.#.#",
            "#...####...#",
            "....####....",
            "..#..##..#..",
            "...######...",
            ".....##.....",
            "....####....",
        ],
        // Two people side by side: a group.
        Icon::Module(Id::VoiceGroup) => &[
            "..##....##..",
            ".####..####.",
            ".####..####.",
            "..##....##..",
            "............",
            ".####..####.",
            "######.#####",
            "############",
            "############",
        ],
        // A speaker with its sound: the players talking.
        Icon::Module(Id::VoiceSpeakers) => &[
            "....#.......",
            "...##...#...",
            "..###....#..",
            "#####..#..#.",
            "#####...#.#.",
            "#####...#.#.",
            "#####..#..#.",
            "..###....#..",
            "...##...#...",
            "....#.......",
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
        Icon::ArrowUp => &[
            "...#...", //
            "..###..", //
            ".#####.", //
            "#######",
        ],
        Icon::ArrowDown => &[
            "#######", //
            ".#####.", //
            "..###..", //
            "...#...",
        ],
        Icon::ArrowLeft => &[
            "...#", //
            "..##", //
            ".###", //
            "####", //
            ".###", //
            "..##", //
            "...#",
        ],
        Icon::ArrowRight => &[
            "#...", //
            "##..", //
            "###.", //
            "####", //
            "###.", //
            "##..", //
            "#...",
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
            Icon::Gear,
            Icon::Search,
            Icon::Back,
            Icon::PadCross,
            Icon::PadCircle,
            Icon::PadSquare,
            Icon::PadTriangle,
            Icon::ArrowUp,
            Icon::ArrowDown,
            Icon::ArrowLeft,
            Icon::ArrowRight,
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
    fn the_logo_is_square_and_fully_coloured() {
        assert!(LOGO_ART.iter().all(|r| r.len() == 16));
        for row in LOGO_ART {
            for b in row.bytes() {
                assert!(
                    b == b'.' || LOGO_COLOURS.iter().any(|(k, _)| *k == b),
                    "{}",
                    b as char
                );
            }
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
