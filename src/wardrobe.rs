//! The skin manager: your skin and cape, chosen from ones you import, or your account's own.
//!
//! - **Custom skins and capes** are PNG files you import (drag one onto the window while the
//!   skin manager is open, or paste a copied file's path), kept in `config/horizon_qol/skins/`
//!   and `config/horizon_qol/capes/`. The one you pick is drawn on your player right away, by
//!   the game-side code (`src/mixins/wardrobe.rs`). Only you see a custom cape: there's no cape
//!   service. A skin you also upload to your Microsoft account is everyone's.
//! - **Your account's capes** (Minecon, Migrator...) come from Mojang: picking one makes it the
//!   cape everyone sees.
//!
//! The account's side (reading it, uploading a skin, choosing a cape) needs the game's sign-in, so
//! the game-side code does it, on its own thread, through the jobs queued here.

use std::sync::{Arc, Mutex, MutexGuard};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A PNG, by a key that changes when its pixels do.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    pub key: String,
    pub png: Arc<Vec<u8>>,
}

impl Texture {
    pub fn new(png: Vec<u8>) -> Self {
        Self {
            key: format!("{:016x}", fnv(&png)),
            png: Arc::new(png),
        }
    }
}

/// FNV-1a, for textures' keys.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

/// What your player wears in this game, for the game-side code to draw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Look {
    /// A custom skin, and whether its arms are slim; None: the account's.
    pub skin: Option<Texture>,
    pub slim: bool,
    /// A custom cape; None: the account's (or none).
    pub cape: Option<Texture>,
}

static LOOK: Mutex<Option<Look>> = Mutex::new(None);

/// For the game: what to draw on the player.
pub fn look() -> Look {
    lock(&LOOK).clone().unwrap_or_default()
}

pub fn set_look(look: Look) {
    *lock(&LOOK) = Some(look);
}

/// Something to do on the account, for the game-side code.
#[derive(Clone, Debug, PartialEq)]
pub enum Job {
    /// Read the account: its name, skin and capes.
    Refresh,
    /// Make `png` the account's skin, slim-armed or not.
    UploadSkin { png: Arc<Vec<u8>>, slim: bool },
    /// Wear the account's cape `id`, or none.
    Cape(Option<String>),
}

static JOBS: Mutex<Vec<Job>> = Mutex::new(Vec::new());

pub fn queue(job: Job) {
    let mut jobs = lock(&JOBS);
    if !jobs.contains(&job) {
        jobs.push(job);
    }
}

/// For the game: the next job.
pub fn take_job() -> Option<Job> {
    let mut jobs = lock(&JOBS);
    (!jobs.is_empty()).then(|| jobs.remove(0))
}

/// One of the account's capes.
#[derive(Clone, Debug, PartialEq)]
pub struct OfficialCape {
    pub id: String,
    pub name: String,
    pub png: Option<Arc<Vec<u8>>>,
    pub active: bool,
}

/// Where reading the account stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum AccountState {
    #[default]
    Unknown,
    Loading,
    /// No Microsoft sign-in (an offline name): only custom skins and capes.
    SignedOut,
    Ready,
    Failed(String),
}

/// The account as last read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    pub state: AccountState,
    pub name: String,
    pub capes: Vec<OfficialCape>,
    /// The account's skin, and whether it is slim.
    pub skin: Option<Arc<Vec<u8>>>,
    pub slim: bool,
}

static ACCOUNT: Mutex<Option<Account>> = Mutex::new(None);

pub fn account() -> Account {
    lock(&ACCOUNT).clone().unwrap_or_default()
}

/// For the game: the account as read, or how reading it went.
pub fn set_account(account: Account) {
    *lock(&ACCOUNT) = Some(account);
}

/// For the game: the state alone, keeping what was read before.
pub fn set_account_state(state: AccountState) {
    lock(&ACCOUNT).get_or_insert_with(Account::default).state = state;
}

static NOTES: Mutex<Vec<(bool, String)>> = Mutex::new(Vec::new());

/// For the game: what became of a job, good or not, to show the player.
pub fn note(ok: bool, text: String) {
    lock(&NOTES).push((ok, text));
}

pub fn take_notes() -> Vec<(bool, String)> {
    std::mem::take(&mut *lock(&NOTES))
}

/// The skin manager is open: files dropped on the window come here.
static TAKES_DROPS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static DROPS: Mutex<Vec<std::path::PathBuf>> = Mutex::new(Vec::new());

pub fn set_takes_drops(on: bool) {
    TAKES_DROPS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// For the game: a file was dropped on the window. True when the skin manager took it.
pub fn dropped(path: &std::path::Path) -> bool {
    if !TAKES_DROPS.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    lock(&DROPS).push(path.to_path_buf());
    true
}

pub fn take_drops() -> Vec<std::path::PathBuf> {
    std::mem::take(&mut *lock(&DROPS))
}

/// Decodes a PNG to its width, height and RGBA pixels: the game's own decoder, which the
/// game-side code lends the mod (so the mod needs no image library of its own).
pub type Decoder = fn(&[u8]) -> Option<(u32, u32, Vec<u8>)>;

static DECODER: Mutex<Option<Decoder>> = Mutex::new(None);

pub fn set_decoder(decoder: Decoder) {
    *lock(&DECODER) = Some(decoder);
}

/// An image's pixels, RGBA, row by row.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    /// Pixel `(x, y)` as ARGB; transparent outside the image.
    pub fn argb(&self, x: u32, y: u32) -> u32 {
        if x >= self.w || y >= self.h {
            return 0;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let p = &self.rgba[i..i + 4];
        (p[3] as u32) << 24 | (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32
    }
}

/// Decodes `png`, when the game has lent its decoder.
pub fn decode(png: &[u8]) -> Option<Image> {
    let decoder = (*lock(&DECODER))?;
    let (w, h, rgba) = decoder(png)?;
    (rgba.len() == (w * h * 4) as usize).then_some(Image { w, h, rgba })
}

/// What an image is, by its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Skin,
    Cape,
    /// 64x32 (or its HD sizes): an old skin or a cape, as the player says.
    Either,
    Neither,
}

pub fn kind(w: u32, h: u32) -> Kind {
    let unit = w / 64;
    if w == 0 || !w.is_multiple_of(64) && w != 22 && w != 46 {
        return Kind::Neither;
    }
    match (w, h) {
        // Capes drawn for the old 22x17 and 46x22 layouts.
        (22, 17) | (46, 22) => Kind::Cape,
        _ if h == w => Kind::Skin,
        _ if h * 2 == w && unit >= 1 => Kind::Either,
        _ => Kind::Neither,
    }
}

/// A file name for an imported image: its letters and digits, the rest `_`, then `.png`.
pub fn file_name(stem: &str) -> String {
    let clean: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(32)
        .collect();
    let clean = clean.trim_matches('_');
    format!("{}.png", if clean.is_empty() { "imported" } else { clean })
}

/// The name a file shows as: its stem, `_` as spaces.
pub fn shown_name(file: &str) -> String {
    file.trim_end_matches(".png").replace('_', " ")
}

/// The path a pasted text names: a path, or a `file://` URI (as a file manager copies a file).
pub fn pasted_path(text: &str) -> Option<std::path::PathBuf> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let path = match line.strip_prefix("file://") {
        Some(rest) => percent_decode(rest),
        None => line.trim_matches('"').to_owned(),
    };
    let path = std::path::PathBuf::from(path);
    path.is_absolute().then_some(path)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A rectangle of a skin's texture (64x64 units: x, y, w, h), drawn at (dx, dy) of the front
/// view, which is 16 by 32 skin pixels.
type Part = ([u32; 4], [u32; 2]);

/// The front of a player, as parts of a 64x64 skin: the base layer, then the outer layer over
/// it. `slim` arms are 3 pixels wide. An old 64x32 skin has no left arm or leg of its own (they
/// mirror the right ones) and no outer layer but the hat.
pub fn front(slim: bool, legacy: bool) -> Vec<Part> {
    let a = if slim { 3 } else { 4 };
    let mut parts = vec![
        ([8, 8, 8, 8], [4, 0]),        // head
        ([20, 20, 8, 12], [4, 8]),     // body
        ([44, 20, a, 12], [4 - a, 8]), // right arm (the viewer's left)
        ([4, 20, 4, 12], [4, 20]),     // right leg
    ];
    if legacy {
        parts.push(([44, 20, a, 12], [12, 8]));
        parts.push(([4, 20, 4, 12], [8, 20]));
        parts.push(([40, 8, 8, 8], [4, 0]));
    } else {
        parts.extend([
            ([36, 52, a, 12], [12, 8]), // left arm
            ([20, 52, 4, 12], [8, 20]), // left leg
            ([40, 8, 8, 8], [4, 0]),    // hat
            ([20, 36, 8, 12], [4, 8]),  // jacket
            ([44, 36, a, 12], [4 - a, 8]),
            ([4, 36, 4, 12], [4, 20]),
            ([52, 52, a, 12], [12, 8]),
            ([4, 52, 4, 12], [8, 20]),
        ]);
    }
    parts
}

/// A cape's front, as a part of a 64x32 cape texture: 10 by 16 pixels.
pub const CAPE_FRONT: [u32; 4] = [1, 1, 10, 16];

/// The pixels of `part` of `img`, scaled from 64-pixel-wide units to the image's size: each
/// with where it goes in the front view and its colour.
pub fn pixels(img: &Image, [sx, sy, w, h]: [u32; 4], [dx, dy]: [u32; 2]) -> Vec<([u32; 2], u32)> {
    let unit = (img.w / 64).max(1);
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let c = img.argb((sx + x) * unit, (sy + y) * unit);
            if c >> 24 != 0 {
                out.push(([dx + x, dy + y], c));
            }
        }
    }
    out
}

/// A cape's pixels: its front, from a 64x32 texture or its HD sizes, or the old 22x17 layout.
pub fn cape_pixels(img: &Image) -> Vec<([u32; 2], u32)> {
    let unit = if img.w == 22 || img.w == 46 {
        1
    } else {
        (img.w / 64).max(1)
    };
    let [sx, sy, w, h] = CAPE_FRONT;
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let c = img.argb((sx + x) * unit, (sy + y) * unit);
            if c >> 24 != 0 {
                out.push(([x, y], c));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, argb: u32) -> Image {
        let px = [
            (argb >> 16) as u8,
            (argb >> 8) as u8,
            argb as u8,
            (argb >> 24) as u8,
        ];
        Image {
            w,
            h,
            rgba: px.repeat((w * h) as usize),
        }
    }

    #[test]
    fn kinds_names_and_paths() {
        assert_eq!(kind(64, 64), Kind::Skin);
        assert_eq!(kind(128, 128), Kind::Skin);
        assert_eq!(kind(64, 32), Kind::Either);
        assert_eq!(kind(512, 256), Kind::Either);
        assert_eq!(kind(22, 17), Kind::Cape);
        assert_eq!(kind(100, 50), Kind::Neither);
        assert_eq!(file_name("My Cape (1)"), "My_Cape__1.png");
        assert_eq!(file_name("***"), "imported.png");
        assert_eq!(shown_name("My_Cape.png"), "My Cape");
        assert_eq!(
            pasted_path("file:///home/me/My%20Cape.png\n"),
            Some(std::path::PathBuf::from("/home/me/My Cape.png"))
        );
        assert_eq!(
            pasted_path("\"/home/me/cape.png\""),
            Some(std::path::PathBuf::from("/home/me/cape.png"))
        );
        assert_eq!(pasted_path("not a path"), None);
        assert_ne!(Texture::new(vec![1]).key, Texture::new(vec![2]).key);
    }

    #[test]
    fn the_front_view_and_cape_read_the_right_pixels() {
        let img = solid(64, 64, 0xFF336699);
        // Head, body, both arms and legs: 64 + 96 + 2 * 48 + 2 * 48 base pixels, then the
        // outer layer over them.
        let base: usize = front(false, false)
            .iter()
            .take(6)
            .map(|(r, at)| pixels(&img, *r, *at).len())
            .sum();
        assert_eq!(base, 64 + 96 + 48 * 4);
        // Slim arms are 3 wide and sit next to the body.
        let (r, at) = front(true, false)[2];
        assert_eq!((r[2], at[0]), (3, 1));
        // An HD cape reads every other pixel; transparent ones are left out.
        let mut cape = solid(128, 64, 0xFFFF0000);
        cape.rgba[(2 * 128 + 2) * 4 + 3] = 0;
        let px = cape_pixels(&cape);
        assert_eq!(px.len(), 10 * 16 - 1);
        assert_eq!(px[0].1, 0xFFFF0000);
    }
}
