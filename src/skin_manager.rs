//! The skin manager, mod side: the imported skins and capes (files in the mod's config folder),
//! which are picked, importing a dropped or pasted file, and what the page shows. The account's
//! side and the drawing are the game's: see `wardrobe.rs`.

use crate::State;
use crate::ui::{WardrobeAction, WardrobeView};
use crate::wardrobe::{self as w, AccountState, Image, Job, Kind, Texture};
use mod_api::{ClientHost, ConfigEntryKind, ConfigPath};
use std::path::Path;

const SKINS: &str = "horizon_qol/skins";
const CAPES: &str = "horizon_qol/capes";
/// The biggest image the skin manager imports.
const MAX_FILE: u64 = 1 << 20;

/// An imported skin or cape.
pub(crate) struct Entry {
    pub file: String,
    pub texture: Texture,
    pub image: Option<Image>,
}

/// What the skin manager keeps.
#[derive(Default)]
pub(crate) struct Wardrobe {
    pub skins: Vec<Entry>,
    pub capes: Vec<Entry>,
    /// The picked imported skin and cape, by file; None: the account's.
    pub skin: Option<String>,
    pub cape: Option<String>,
    pub slim: bool,
    /// The folders have been read.
    listed: bool,
    /// The account's pictures, decoded once: by the address of their bytes.
    account_images: Vec<(usize, Option<Image>)>,
}

fn path(folder: &str, file: &str) -> Option<ConfigPath> {
    ConfigPath::new(&format!("{folder}/{file}")).ok()
}

/// The PNGs in `folder`, by name; an emptied file is a deleted one (the mod can't delete files).
fn list(host: &dyn ClientHost, folder: &str) -> Vec<Entry> {
    let files = host.config_files();
    let Ok(dir) = ConfigPath::new(folder) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut after: Option<String> = None;
    while let Ok(page) = files.list(&dir, after.as_deref(), 256) {
        let Some(last) = page.last() else { break };
        after = Some(last.name.clone());
        for entry in page {
            if entry.stat.kind != ConfigEntryKind::File
                || entry.stat.len == 0
                || !entry.name.to_lowercase().ends_with(".png")
            {
                continue;
            }
            let Some(bytes) = path(folder, &entry.name).and_then(|p| files.read(&p).ok()) else {
                continue;
            };
            out.push(Entry {
                file: entry.name,
                image: w::decode(&bytes),
                texture: Texture::new(bytes),
            });
        }
    }
    out.sort_by_key(|e| e.file.to_lowercase());
    out
}

impl Wardrobe {
    fn reread(&mut self, host: &dyn ClientHost) {
        self.skins = list(host, SKINS);
        self.capes = list(host, CAPES);
        self.listed = true;
        // A picked file that is gone: back to the account's.
        if self
            .skin
            .as_ref()
            .is_some_and(|f| !self.skins.iter().any(|e| &e.file == f))
        {
            self.skin = None;
        }
        if self
            .cape
            .as_ref()
            .is_some_and(|f| !self.capes.iter().any(|e| &e.file == f))
        {
            self.cape = None;
        }
    }

    fn look(&self) -> w::Look {
        let find = |list: &[Entry], file: &Option<String>| {
            file.as_ref()
                .and_then(|f| list.iter().find(|e| &e.file == f))
                .map(|e| e.texture.clone())
        };
        w::Look {
            skin: find(&self.skins, &self.skin),
            slim: self.slim,
            cape: find(&self.capes, &self.cape),
        }
    }

    /// The account's picture `png`, decoded once.
    fn account_image(&mut self, png: &std::sync::Arc<Vec<u8>>) -> Option<Image> {
        let key = std::sync::Arc::as_ptr(png) as usize;
        if let Some((_, img)) = self.account_images.iter().find(|(k, _)| *k == key) {
            return img.clone();
        }
        let img = w::decode(png);
        // Only images the account holds now: an old read's are dropped.
        if self.account_images.len() > 32 {
            self.account_images.clear();
        }
        self.account_images.push((key, img.clone()));
        img
    }

    /// Imports the PNG at `from`: a skin, or a cape (an image that could be either is a cape when
    /// dropped over the capes). Picks it, and says what became of it.
    fn import(
        &mut self,
        host: &dyn ClientHost,
        from: &Path,
        as_cape: bool,
    ) -> Result<String, String> {
        let size = std::fs::metadata(from)
            .map_err(|e| format!("Couldn't read {}: {e}", from.display()))?
            .len();
        if size > MAX_FILE {
            return Err("That file is too big for a skin or a cape.".into());
        }
        let bytes =
            std::fs::read(from).map_err(|e| format!("Couldn't read {}: {e}", from.display()))?;
        let img = w::decode(&bytes).ok_or("That isn't a PNG image.")?;
        let cape = match w::kind(img.w, img.h) {
            Kind::Skin => false,
            Kind::Cape => true,
            Kind::Either => as_cape,
            Kind::Neither => {
                return Err(format!(
                    "A {}x{} image is neither a skin (64x64) nor a cape (64x32).",
                    img.w, img.h
                ));
            }
        };
        let stem = from
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported");
        let (folder, list) = if cape {
            (CAPES, &self.capes)
        } else {
            (SKINS, &self.skins)
        };
        let wanted = w::file_name(stem);
        let taken = |f: &str| list.iter().any(|e| e.file.eq_ignore_ascii_case(f));
        let file = std::iter::once(wanted.clone())
            .chain((2..).map(|i| wanted.replace(".png", &format!("_{i}.png"))))
            .find(|f| !taken(f))
            .unwrap_or(wanted);
        let to = path(folder, &file).ok_or("That name can't be saved.")?;
        host.config_files()
            .write(&to, &bytes)
            .map_err(|e| format!("Couldn't save it: {e:?}"))?;
        self.reread(host);
        let name = w::shown_name(&file);
        if cape {
            self.cape = Some(file);
            Ok(format!("Imported the cape {name}: only you see it."))
        } else {
            self.slim = slim_arms(&img);
            self.skin = Some(file);
            Ok(format!(
                "Imported the skin {name}. Upload it to make it your account's."
            ))
        }
    }

    /// Deletes an imported skin or cape.
    fn delete(&mut self, host: &dyn ClientHost, cape: bool, index: usize) -> Option<String> {
        let (folder, list) = if cape {
            (CAPES, &self.capes)
        } else {
            (SKINS, &self.skins)
        };
        let file = list.get(index)?.file.clone();
        host.config_files().write(&path(folder, &file)?, &[]).ok()?;
        self.reread(host);
        Some(format!("Deleted {}.", w::shown_name(&file)))
    }
}

/// Whether a 64x64 skin has slim arms: the fourth column of its right arm is see-through.
fn slim_arms(img: &Image) -> bool {
    let unit = (img.w / 64).max(1);
    img.h == img.w
        && img.argb(54 * unit, 20 * unit) >> 24 == 0
        && img.argb(55 * unit, 20 * unit) >> 24 == 0
}

/// Reads the picks from the config file's `wardrobe.` lines.
pub(crate) fn load(state: &mut State, lines: &[(String, String)]) {
    let get = |k: &str| {
        lines
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone())
    };
    let wd = &mut state.wardrobe;
    wd.skin = get("wardrobe.skin").filter(|v| !v.is_empty());
    wd.cape = get("wardrobe.cape").filter(|v| !v.is_empty());
    wd.slim = get("wardrobe.slim").as_deref() == Some("true");
}

/// The config file's `wardrobe.` lines.
pub(crate) fn save(state: &State) -> String {
    let wd = &state.wardrobe;
    format!(
        "wardrobe.skin={}\nwardrobe.cape={}\nwardrobe.slim={}\n",
        wd.skin.as_deref().unwrap_or(""),
        wd.cape.as_deref().unwrap_or(""),
        wd.slim
    )
}

/// Each client tick: the folders once, the picks for the game to draw, and dropped files.
pub(crate) fn tick(host: &dyn ClientHost, state: &mut State) {
    let open = state.gui.as_ref().is_some_and(|g| g.wardrobe_open())
        && host.mod_screen() == Some(crate::ID);
    w::set_takes_drops(open);
    let wd = &mut state.wardrobe;
    if !wd.listed {
        wd.reread(host);
    }
    // Pictures read before the game lent its decoder.
    for e in wd.skins.iter_mut().chain(wd.capes.iter_mut()) {
        if e.image.is_none() {
            e.image = w::decode(&e.texture.png);
        }
    }
    let wanted_look = wd.look();
    if state.module(crate::Id::Wardrobe).enabled {
        w::set_look(wanted_look);
    } else {
        w::set_look(w::Look::default());
    }
    let cape_zone = state.gui.as_ref().is_some_and(|g| g.wardrobe_cape_zone);
    let mut notes: Vec<(String, u32)> = Vec::new();
    for path in w::take_drops() {
        notes.push(match state.wardrobe.import(host, &path, cape_zone) {
            Ok(said) => (said, GOOD),
            Err(said) => (said, BAD),
        });
    }
    for (ok, said) in w::take_notes() {
        notes.push((said, if ok { GOOD } else { BAD }));
    }
    let picked = !notes.is_empty();
    if let (Some(note), Some(gui)) = (notes.pop(), state.gui.as_mut()) {
        gui.note = Some(note);
    }
    if picked {
        crate::save_config(host, state);
    }
}

const GOOD: u32 = 0xFF22C55E;
const BAD: u32 = 0xFFEF4444;

/// The page opened: the folders and the account read again.
pub(crate) fn opened(host: &dyn ClientHost, state: &mut State) {
    state.wardrobe.reread(host);
    w::queue(Job::Refresh);
}

/// Does what the page asked.
pub(crate) fn act(host: &dyn ClientHost, state: &mut State, action: WardrobeAction) {
    let account = w::account();
    let wd = &mut state.wardrobe;
    let note: Option<(String, u32)> = match action {
        WardrobeAction::Skin(i) => {
            wd.skin = i.and_then(|i| wd.skins.get(i)).map(|e| e.file.clone());
            if let Some(img) = i
                .and_then(|i| wd.skins.get(i))
                .and_then(|e| e.image.as_ref())
            {
                wd.slim = slim_arms(img);
            }
            None
        }
        WardrobeAction::CustomCape(i) => {
            wd.cape = wd.capes.get(i).map(|e| e.file.clone());
            Some(("Only you see an imported cape.".into(), 0xFFB4B4BE))
        }
        WardrobeAction::OfficialCape(i) => {
            wd.cape = None;
            match i.and_then(|i| account.capes.get(i)) {
                Some(cape) => {
                    w::queue(Job::Cape(Some(cape.id.clone())));
                    Some((format!("Putting on {}...", cape.name), 0xFFB4B4BE))
                }
                None if account.capes.iter().any(|c| c.active) => {
                    w::queue(Job::Cape(None));
                    Some(("Taking off your cape...".into(), 0xFFB4B4BE))
                }
                None => None,
            }
        }
        WardrobeAction::Slim(slim) => {
            wd.slim = slim;
            None
        }
        WardrobeAction::Upload => {
            let png = wd
                .skin
                .as_ref()
                .and_then(|f| wd.skins.iter().find(|e| &e.file == f))
                .map(|e| e.texture.png.clone());
            png.map(|png| {
                w::queue(Job::UploadSkin { png, slim: wd.slim });
                ("Uploading your skin...".to_owned(), 0xFFB4B4BE)
            })
        }
        WardrobeAction::Paste => Some(
            match host.clipboard_text().as_deref().and_then(w::pasted_path) {
                Some(path) => match wd.import(host, &path, true) {
                    Ok(said) => (said, GOOD),
                    Err(said) => (said, BAD),
                },
                None => (
                    "Copy a PNG file (or its path) first, then press Paste.".into(),
                    BAD,
                ),
            },
        ),
        WardrobeAction::Refresh => {
            wd.reread(host);
            w::queue(Job::Refresh);
            None
        }
        WardrobeAction::Delete { cape, index } => wd.delete(host, cape, index).map(|s| (s, GOOD)),
    };
    if let (Some(note), Some(gui)) = (note, state.gui.as_mut()) {
        gui.note = Some(note);
    }
    crate::save_config(host, state);
}

/// What the page shows now.
pub(crate) fn view(state: &mut State) -> WardrobeView {
    let account = w::account();
    let wd = &mut state.wardrobe;
    let index = |list: &[Entry], file: &Option<String>| {
        file.as_ref()
            .and_then(|f| list.iter().position(|e| &e.file == f))
    };
    let official = account
        .capes
        .iter()
        .map(|c| {
            let img = c.png.as_ref().and_then(|png| wd.account_image(png));
            (c.name.clone(), img, c.active)
        })
        .collect();
    let account_skin = account.skin.as_ref().and_then(|png| wd.account_image(png));
    let (line, colour) = match &account.state {
        AccountState::Unknown | AccountState::Loading => {
            ("Reading your account...".to_owned(), 0xFF9090A0)
        }
        AccountState::SignedOut => (
            "Offline name: imported skins and capes, which only you see.".to_owned(),
            0xFF9090A0,
        ),
        AccountState::Ready => (format!("Signed in as {}", account.name), 0xFF55FF55),
        AccountState::Failed(e) => (format!("Couldn't read your account: {e}"), BAD),
    };
    WardrobeView {
        skins: wd
            .skins
            .iter()
            .map(|e| (e.file.clone(), e.image.clone()))
            .collect(),
        capes: wd
            .capes
            .iter()
            .map(|e| (e.file.clone(), e.image.clone()))
            .collect(),
        official,
        account_skin,
        account_slim: account.slim,
        skin: index(&wd.skins, &wd.skin),
        cape: index(&wd.capes, &wd.cape),
        slim: wd.slim,
        account: (line, colour),
        online: matches!(account.state, AccountState::Ready | AccountState::Loading),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slim_arms_are_told_apart() {
        let mut img = Image {
            w: 64,
            h: 64,
            rgba: vec![255; 64 * 64 * 4],
        };
        assert!(!slim_arms(&img));
        for x in [54, 55] {
            img.rgba[(20 * 64 + x) * 4 + 3] = 0;
        }
        assert!(slim_arms(&img));
    }
}
