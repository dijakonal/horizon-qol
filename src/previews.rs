//! Shulker box and ender chest previews: a grid of the box's or chest's items in its tooltip,
//! drawn like the game's own inventory slots.
//!
//! A shulker box item carries its items (the `minecraft:container` component), so its preview
//! is always exact. The game never sends a client its ender chest until it is opened, so the
//! ender chest preview shows what it held the last time the player opened one, remembered per
//! world or server and kept in the mod's data folder across restarts.

use mod_api::{Compound, DrawContext, Host, ItemStack, Tag};
use std::sync::Mutex;

/// A container's 27 slots.
pub const SLOTS: usize = 27;
const COLUMNS: usize = 9;
const SLOT: f32 = 18.0;
const PAD: f32 = 4.0;

/// What the tooltip parts need, shared with the hooks that keep it current.
#[derive(Default)]
pub struct Shared {
    pub shulker: Show,
    pub shulker_tint: bool,
    /// Whether the game's own "64 x Oak Leaves" lines go while the grid shows.
    pub shulker_hide_list: bool,
    pub ender: Show,
    /// The world or server the player is in, as a file name; None outside a world.
    pub session: Option<String>,
    /// The ender chest as last seen in this session; None before it has been opened here.
    pub ender_items: Option<Vec<Option<ItemStack>>>,
    /// When it was last seen, in seconds since 1970.
    pub ender_seen: u64,
}

/// When a preview shows.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Show {
    #[default]
    Off,
    Always,
    /// While Shift is held.
    Shift,
}

impl Show {
    pub fn from(enabled: bool, mode: usize) -> Self {
        match (enabled, mode) {
            (false, _) => Show::Off,
            (true, 1) => Show::Shift,
            _ => Show::Always,
        }
    }

    pub fn now(self, shift: bool) -> bool {
        match self {
            Show::Off => false,
            Show::Always => true,
            Show::Shift => shift,
        }
    }
}

pub static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

/// Runs `f` on the shared state.
pub fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
    let mut guard = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Shared::default))
}

/// A field of compound `c` by name.
fn field<'a>(c: &'a Compound, key: &str) -> Option<&'a Tag> {
    c.iter()
        .find(|(k, _)| String::from_utf16_lossy(&k.0) == key)
        .map(|(_, v)| v)
}

fn number(tag: &Tag) -> Option<i64> {
    match tag {
        Tag::Byte(v) => Some(*v as i64),
        Tag::Short(v) => Some(*v as i64),
        Tag::Int(v) => Some(*v as i64),
        Tag::Long(v) => Some(*v),
        _ => None,
    }
}

/// The items a container item (a shulker box) holds, by slot: its `minecraft:container`
/// component, a list of `{slot, item}`. None when it holds nothing.
pub fn contents(host: &dyn Host, stack: &ItemStack) -> Option<Vec<Option<ItemStack>>> {
    let patch = stack.components()?.as_nbt();
    let Tag::List(entries) = field(patch, "minecraft:container")? else {
        return None;
    };
    let mut slots = vec![None; SLOTS];
    let mut any = false;
    for entry in entries {
        let Tag::Compound(entry) = entry else {
            continue;
        };
        let Some(slot) = field(entry, "slot").and_then(number) else {
            continue;
        };
        let Some(Tag::Compound(item)) = field(entry, "item") else {
            continue;
        };
        if let (Ok(slot), Some(stack)) = (usize::try_from(slot), ItemStack::from_nbt(host, item))
            && slot < SLOTS
        {
            slots[slot] = Some(stack);
            any = true;
        }
    }
    any.then_some(slots)
}

/// Whether item id `id` is a shulker box, and its dye colour when it has one.
pub fn shulker_colour(id: &str) -> Option<Option<u32>> {
    let name = id.strip_prefix("minecraft:")?;
    if name == "shulker_box" {
        return Some(None);
    }
    let dye = name.strip_suffix("_shulker_box")?;
    let colour = match dye {
        "white" => 0xFFF9FFFE,
        "orange" => 0xFFF9801D,
        "magenta" => 0xFFC74EBD,
        "light_blue" => 0xFF3AB3DA,
        "yellow" => 0xFFFED83D,
        "lime" => 0xFF80C71F,
        "pink" => 0xFFF38BAA,
        "gray" => 0xFF474F52,
        "light_gray" => 0xFF9D9D97,
        "cyan" => 0xFF169C9C,
        "purple" => 0xFF8932B8,
        "blue" => 0xFF3C44AA,
        "brown" => 0xFF835432,
        "green" => 0xFF5E7C16,
        "red" => 0xFFB02E26,
        "black" => 0xFF1D1D21,
        _ => return None,
    };
    Some(Some(colour))
}

/// The plain shulker box's own purple.
pub const SHULKER_PURPLE: u32 = 0xFF976997;
/// The ender chest's dark green-black.
pub const ENDER_GREEN: u32 = 0xFF1E3A36;

/// The size of a 27-slot grid in GUI pixels.
pub fn grid_size() -> [f32; 2] {
    [
        COLUMNS as f32 * SLOT + PAD * 2.0,
        (SLOTS / COLUMNS) as f32 * SLOT + PAD * 2.0,
    ]
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |s: u32| {
        let x = ((a >> s) & 0xFF) as f32;
        let y = ((b >> s) & 0xFF) as f32;
        ((x + (y - x) * t).round() as u32) << s
    };
    ch(24) | ch(16) | ch(8) | ch(0)
}

/// Draws `items` as the game's inventory slots, its top left at `at`, the panel tinted towards
/// `tint` when there is one.
pub fn draw_grid(
    draw: &mut dyn DrawContext,
    at: [f32; 2],
    items: &[Option<ItemStack>],
    tint: Option<u32>,
) {
    let [w, h] = grid_size();
    let panel = match tint {
        Some(t) => mix(0xFFC6C6C6, t, 0.6),
        None => 0xFFC6C6C6,
    };
    // The panel, with the game's light top-left and dark bottom-right edges.
    draw.fill([at[0], at[1], w, h], panel);
    draw.fill([at[0], at[1], w, 1.0], mix(panel, 0xFFFFFFFF, 0.6));
    draw.fill([at[0], at[1], 1.0, h], mix(panel, 0xFFFFFFFF, 0.6));
    draw.fill(
        [at[0], at[1] + h - 1.0, w, 1.0],
        mix(panel, 0xFF000000, 0.45),
    );
    draw.fill(
        [at[0] + w - 1.0, at[1], 1.0, h],
        mix(panel, 0xFF000000, 0.45),
    );
    for i in 0..SLOTS {
        let x = at[0] + PAD + (i % COLUMNS) as f32 * SLOT;
        let y = at[1] + PAD + (i / COLUMNS) as f32 * SLOT;
        // A slot: dark top-left, light bottom-right, grey inside.
        draw.fill([x, y, SLOT, SLOT], 0xFFFFFFFF);
        draw.fill([x, y, SLOT - 1.0, SLOT - 1.0], 0xFF373737);
        draw.fill([x + 1.0, y + 1.0, SLOT - 2.0, SLOT - 2.0], 0xFF8B8B8B);
    }
    for (i, item) in items.iter().enumerate().take(SLOTS) {
        let Some(stack) = item else { continue };
        let x = at[0] + PAD + (i % COLUMNS) as f32 * SLOT + 1.0;
        let y = at[1] + PAD + (i / COLUMNS) as f32 * SLOT + 1.0;
        let count = (stack.count() > 1).then(|| stack.count().to_string());
        draw.gui_draw_item_with(stack, [x, y], count.as_deref());
    }
}

/// A tooltip line as plain text: its literal characters, a translation's fallback or key, and
/// its arguments and children in order.
pub fn plain(text: &mod_api::Text) -> String {
    use mod_api::text::TextContent;
    let mut out = String::new();
    match &text.content {
        TextContent::Literal(s) => out.push_str(s),
        TextContent::Translatable {
            key,
            fallback,
            arguments,
        } => {
            out.push_str(fallback.as_deref().unwrap_or(key));
            for a in arguments {
                out.push(' ');
                out.push_str(&plain(a));
            }
        }
        _ => {}
    }
    for c in &text.children {
        out.push_str(&plain(c));
    }
    out
}

/// Whether `line` has the number `n` in it as a whole number.
fn has_number(line: &str, n: usize) -> bool {
    line.split(|c: char| !c.is_ascii_digit())
        .any(|w| w.parse::<usize>().ok() == Some(n))
}

/// Where the game's own list of a shulker box's items is in its tooltip, as (first line, line
/// count): a line per stack for the first five (`counts`, by slot), each with its count, then
/// "and N more..." when there are more. The list comes after the name, so line 0 is never in it.
pub fn list_lines(lines: &[String], counts: &[usize]) -> Option<(usize, usize)> {
    let shown = counts.len().min(5);
    let more = counts.len().saturating_sub(5);
    let len = shown + usize::from(more > 0);
    if len == 0 || lines.len() < len + 1 {
        return None;
    }
    (1..=lines.len() - len)
        .find(|&at| {
            counts[..shown]
                .iter()
                .enumerate()
                .all(|(i, &c)| has_number(&lines[at + i], c))
                && (more == 0 || has_number(&lines[at + shown], more))
        })
        .map(|at| (at, len))
}

/// The tooltip box's colours for a shulker box of colour `c`, as the game's tooltip takes them:
/// (background, top border, bottom border), ARGB.
pub fn tooltip_colours(c: u32) -> (u32, u32, u32) {
    let rgb = |v: u32| v & 0x00FF_FFFF;
    let background = rgb(mix(c, 0xFF000000, 0.78)) | 0xF000_0000;
    let top = rgb(mix(c, 0xFFFFFFFF, 0.1)) | 0xFF00_0000;
    let bottom = rgb(mix(c, 0xFF000000, 0.45)) | 0xFF00_0000;
    (background, top, bottom)
}

/// The message an ender chest shows before it has been seen here.
pub const ENDER_UNSEEN: &str = "Open your ender chest once to preview it.";

/// A file name for a world or server: its letters and digits, the rest as `_`.
pub fn file_name(prefix: &str, name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    format!("{prefix}_{clean}.nbt")
}

// A small NBT codec for the remembered ender chests: Java's binary NBT, uncompressed.

fn write_string(out: &mut Vec<u8>, units: &[u16]) {
    let s = String::from_utf16_lossy(units);
    let bytes = s.as_bytes();
    let len = bytes.len().min(u16::MAX as usize);
    out.extend_from_slice(&(len as u16).to_be_bytes());
    out.extend_from_slice(&bytes[..len]);
}

fn write_payload(out: &mut Vec<u8>, tag: &Tag) {
    match tag {
        Tag::End => {}
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
        Tag::ByteArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            out.extend_from_slice(v);
        }
        Tag::String(s) => write_string(out, &s.0),
        Tag::List(items) => {
            out.push(items.first().map_or(0, Tag::id));
            out.extend_from_slice(&(items.len() as i32).to_be_bytes());
            for item in items {
                write_payload(out, item);
            }
        }
        Tag::Compound(map) => {
            for (k, v) in map {
                out.push(v.id());
                write_string(out, &k.0);
                write_payload(out, v);
            }
            out.push(0);
        }
        Tag::IntArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
        Tag::LongArray(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for x in v {
                out.extend_from_slice(&x.to_be_bytes());
            }
        }
    }
}

/// `tag` as NBT bytes: its type, an empty name, its payload.
pub fn encode(tag: &Tag) -> Vec<u8> {
    let mut out = vec![tag.id(), 0, 0];
    write_payload(&mut out, tag);
    out
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    depth: u32,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let end = self.at.checked_add(n)?;
        let slice = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    fn len(&mut self) -> Option<usize> {
        let n = i32::from_be_bytes(self.array()?);
        // Every element is at least a byte: a length past the end is corrupt.
        usize::try_from(n).ok().filter(|n| *n <= self.bytes.len())
    }

    fn string(&mut self) -> Option<Tag> {
        let len = u16::from_be_bytes(self.array()?) as usize;
        let s = String::from_utf8_lossy(self.take(len)?).into_owned();
        Some(Tag::String(s.as_str().into()))
    }

    fn payload(&mut self, id: u8) -> Option<Tag> {
        self.depth += 1;
        if self.depth > 64 {
            return None;
        }
        let tag = match id {
            1 => Tag::Byte(self.array::<1>()?[0] as i8),
            2 => Tag::Short(i16::from_be_bytes(self.array()?)),
            3 => Tag::Int(i32::from_be_bytes(self.array()?)),
            4 => Tag::Long(i64::from_be_bytes(self.array()?)),
            5 => Tag::Float(u32::from_be_bytes(self.array()?)),
            6 => Tag::Double(u64::from_be_bytes(self.array()?)),
            7 => {
                let n = self.len()?;
                Tag::ByteArray(self.take(n)?.to_vec())
            }
            8 => self.string()?,
            9 => {
                let item = self.array::<1>()?[0];
                let n = self.len()?;
                let mut items = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    items.push(self.payload(item)?);
                }
                Tag::List(items)
            }
            10 => {
                let mut map = Compound::new();
                loop {
                    let child = self.array::<1>()?[0];
                    if child == 0 {
                        break;
                    }
                    let Tag::String(name) = self.string()? else {
                        return None;
                    };
                    let value = self.payload(child)?;
                    map.insert(name, value);
                }
                Tag::Compound(map)
            }
            11 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    v.push(i32::from_be_bytes(self.array()?));
                }
                Tag::IntArray(v)
            }
            12 => {
                let n = self.len()?;
                let mut v = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    v.push(i64::from_be_bytes(self.array()?));
                }
                Tag::LongArray(v)
            }
            _ => return None,
        };
        self.depth -= 1;
        Some(tag)
    }
}

/// The tag `encode` wrote; None for bytes that aren't one.
pub fn decode(bytes: &[u8]) -> Option<Tag> {
    let mut r = Reader {
        bytes,
        at: 0,
        depth: 0,
    };
    let id = r.array::<1>()?[0];
    r.string()?;
    r.payload(id)
}

/// The ender chest file: `{seen: seconds, items: [{slot, item}]}`.
pub fn ender_to_nbt(host: &dyn Host, items: &[Option<ItemStack>], seen: u64) -> Tag {
    let entries: Vec<Tag> = items
        .iter()
        .enumerate()
        .filter_map(|(slot, item)| {
            let nbt = item.as_ref()?.to_nbt(host)?;
            let mut entry = Compound::new();
            entry.insert("slot".into(), Tag::Int(slot as i32));
            entry.insert("item".into(), Tag::Compound(nbt));
            Some(Tag::Compound(entry))
        })
        .collect();
    let mut root = Compound::new();
    root.insert("seen".into(), Tag::Long(seen as i64));
    root.insert("items".into(), Tag::List(entries));
    Tag::Compound(root)
}

/// The ender chest an `ender_to_nbt` file holds, and when it was seen.
pub fn ender_from_nbt(host: &dyn Host, tag: &Tag) -> Option<(Vec<Option<ItemStack>>, u64)> {
    let Tag::Compound(root) = tag else {
        return None;
    };
    let seen = field(root, "seen").and_then(number).unwrap_or(0).max(0) as u64;
    let Some(Tag::List(entries)) = field(root, "items") else {
        return None;
    };
    let mut slots = vec![None; SLOTS];
    for entry in entries {
        let Tag::Compound(entry) = entry else {
            continue;
        };
        let (Some(slot), Some(Tag::Compound(item))) =
            (field(entry, "slot").and_then(number), field(entry, "item"))
        else {
            continue;
        };
        if let Ok(slot) = usize::try_from(slot)
            && slot < SLOTS
        {
            slots[slot] = ItemStack::from_nbt(host, item);
        }
    }
    Some((slots, seen))
}

/// How long ago `seen` was, as a tooltip says it.
pub fn ago(now: u64, seen: u64) -> String {
    let s = now.saturating_sub(seen);
    match s {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", s / 60),
        3600..86_400 => format!("{} h ago", s / 3600),
        _ => format!("{} days ago", s / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nbt_round_trips_every_kind() {
        let mut inner = Compound::new();
        inner.insert("id".into(), Tag::String("minecraft:diamond".into()));
        inner.insert("count".into(), Tag::Int(64));
        let mut root = Compound::new();
        root.insert("b".into(), Tag::Byte(-3));
        root.insert("s".into(), Tag::Short(1234));
        root.insert("l".into(), Tag::Long(-1 << 40));
        root.insert("f".into(), Tag::Float(1.5f32.to_bits()));
        root.insert("d".into(), Tag::Double(2.25f64.to_bits()));
        root.insert("ba".into(), Tag::ByteArray(vec![1, 2, 3]));
        root.insert("ia".into(), Tag::IntArray(vec![-1, 7]));
        root.insert("la".into(), Tag::LongArray(vec![9]));
        root.insert(
            "list".into(),
            Tag::List(vec![Tag::Compound(inner.clone()), Tag::Compound(inner)]),
        );
        root.insert("empty".into(), Tag::List(vec![]));
        let tag = Tag::Compound(root);
        assert_eq!(decode(&encode(&tag)), Some(tag));
    }

    #[test]
    fn corrupt_bytes_decode_to_nothing() {
        assert_eq!(decode(&[]), None);
        assert_eq!(
            decode(&[10, 0, 0, 9, 0, 1, b'x', 1, 0x7F, 0xFF, 0xFF, 0xFF]),
            None
        );
        assert_eq!(decode(&[42, 0, 0]), None);
        // Deep nesting is refused rather than followed.
        let mut deep = vec![9, 0, 0];
        for _ in 0..100 {
            deep.extend_from_slice(&[9, 0, 0, 0, 1]);
        }
        assert_eq!(decode(&deep), None);
    }

    #[test]
    fn shulker_colours_and_file_names() {
        assert_eq!(shulker_colour("minecraft:shulker_box"), Some(None));
        assert_eq!(
            shulker_colour("minecraft:red_shulker_box"),
            Some(Some(0xFFB02E26))
        );
        assert_eq!(
            shulker_colour("minecraft:light_gray_shulker_box"),
            Some(Some(0xFF9D9D97))
        );
        assert_eq!(shulker_colour("minecraft:ender_chest"), None);
        assert_eq!(shulker_colour("mymod:red_shulker_box"), None);
        assert_eq!(
            file_name("mp", "Play.Example.com:25565"),
            "mp_play.example.com_25565.nbt"
        );
        assert_eq!(file_name("sp", "My World!"), "sp_my_world_.nbt");
    }

    #[test]
    fn show_modes_and_times() {
        assert!(!Show::from(false, 0).now(true));
        assert!(Show::from(true, 0).now(false));
        assert!(!Show::from(true, 1).now(false));
        assert!(Show::from(true, 1).now(true));
        assert_eq!(ago(100, 90), "just now");
        assert_eq!(ago(10_000, 10_000 - 300), "5 min ago");
        assert_eq!(ago(200_000, 0), "2 days ago");
    }

    #[test]
    fn the_games_item_list_is_found() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // 27 stacks of 64: five lines, then "and 22 more...", then the F3+H lines.
        let lines = l(&[
            "Yellow Shulker Box",
            "64 x Oak Leaves",
            "64 x Oak Leaves",
            "64 x Oak Leaves",
            "64 x Oak Leaves",
            "64 x Oak Leaves",
            "and 22 more...",
            "minecraft:yellow_shulker_box",
            "14 component(s)",
        ]);
        assert_eq!(list_lines(&lines, &[64; 27]), Some((1, 6)));
        // Lore first, and a German count format; two stacks so no "more" line.
        let lines = l(&["Box", "My stuff", "stone\u{d7} 3", "dirt\u{d7} 12"]);
        assert_eq!(list_lines(&lines, &[3, 12]), Some((2, 2)));
        assert_eq!(list_lines(&lines, &[4, 12]), None);
        assert_eq!(list_lines(&lines, &[]), None);
        let (bg, top, bottom) = tooltip_colours(0xFFFED83D);
        assert_eq!(bg >> 24, 0xF0);
        assert!(top & 0xFF0000 > bottom & 0xFF0000);
    }
}
