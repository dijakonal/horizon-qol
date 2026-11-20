//! The screen drawn on a fake `DrawContext` that keeps the calls, so layout, clicks and drags are
//! checked without the game. `HORIZON_QOL_PREVIEW=<dir>` also writes each frame as an SVG.
use super::*;
use crate::hud::{Effect, HudData, Keys};
use crate::modules::{self, Id};
use mod_api::client::hud_layout::PoseOp;
use mod_api::text::TextContent;
use mod_api::{GuiClipRefusal, ItemStack, TextFormError};
use std::fmt::Write as _;
use std::time::Duration;

/// The game's font is about 6 GUI pixels a character.
const CHAR_W: f32 = 6.0;

#[derive(Default)]
pub(crate) struct Recorder {
    size: [f32; 2],
    svg: String,
    pub texts: Vec<(String, [f32; 2])>,
    clips: Vec<[f32; 4]>,
    /// The `gui_paint` pose stack: translate and scale.
    poses: Vec<([f32; 2], f32)>,
}

fn colour(argb: u32) -> String {
    let a = (argb >> 24) as f32 / 255.0;
    format!(
        "rgba({},{},{},{a:.3})",
        (argb >> 16) & 0xFF,
        (argb >> 8) & 0xFF,
        argb & 0xFF
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl Recorder {
    pub(crate) fn new(size: [f32; 2]) -> Self {
        Self {
            size,
            ..Self::default()
        }
    }

    fn clip_attr(&self) -> String {
        match self.clips.len() {
            0 => String::new(),
            n => format!(" clip-path=\"url(#c{})\"", n - 1),
        }
    }

    fn pose(&self) -> ([f32; 2], f32) {
        self.poses.last().copied().unwrap_or(([0.0, 0.0], 1.0))
    }

    fn rect(&mut self, [x, y, w, h]: [f32; 4], argb: u32) {
        let clip = self.clip_attr();
        let _ = writeln!(
            self.svg,
            "<rect x='{x}' y='{y}' width='{w}' height='{h}' fill='{}'{clip}/>",
            colour(argb)
        );
    }

    fn label(&mut self, text: &str, [x, y]: [f32; 2], scale: f32, argb: u32) {
        let clip = self.clip_attr();
        let _ = writeln!(
            self.svg,
            "<text x='{x}' y='{}' fill='{}' font-size='{}' font-family='monospace' textLength='{}' lengthAdjust='spacingAndGlyphs'{clip}>{}</text>",
            y + 7.0 * scale,
            colour(argb),
            8.0 * scale,
            self.text_width(text) * scale,
            escape(text)
        );
        self.texts.push((text.to_owned(), [x, y]));
    }

    pub(crate) fn write(&self, name: &str) {
        if let Ok(dir) = std::env::var("HORIZON_QOL_PREVIEW") {
            let [w, h] = self.size;
            let svg = format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}' viewBox='0 0 {w} {h}'>\
                 <defs><linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='#7aa7e0'/><stop offset='0.55' stop-color='#b9d3ee'/><stop offset='0.56' stop-color='#5d8b3a'/><stop offset='1' stop-color='#3f6a2a'/></linearGradient></defs>\
                 <rect width='{w}' height='{h}' fill='url(#sky)'/>\n{}</svg>",
                w * 3.0,
                h * 3.0,
                self.svg
            );
            std::fs::write(format!("{dir}/{name}.svg"), svg).unwrap();
        }
    }
}

impl DrawContext for Recorder {
    fn size(&self) -> [f32; 2] {
        self.size
    }
    fn fill(&mut self, r: [f32; 4], argb: u32) {
        self.rect(r, argb);
    }
    fn text(&mut self, text: &str, at: [f32; 2], argb: u32, _: bool) {
        self.label(text, at, 1.0, argb);
    }
    fn text_width(&self, text: &str) -> f32 {
        text.chars().count() as f32 * CHAR_W
    }
    fn push_clip(&mut self, [x, y, w, h]: [f32; 4]) -> Result<(), GuiClipRefusal> {
        let _ = writeln!(
            self.svg,
            "<clipPath id='c{}'><rect x='{x}' y='{y}' width='{w}' height='{h}'/></clipPath>",
            self.clips.len()
        );
        self.clips.push([x, y, w, h]);
        Ok(())
    }
    fn pop_clip(&mut self) -> Result<(), GuiClipRefusal> {
        self.clips.pop();
        Ok(())
    }
    fn gui_paint(&mut self, draws: &[GuiDraw]) -> Result<(), TextFormError> {
        for d in draws {
            let ([tx, ty], s) = self.pose();
            match d {
                GuiDraw::Push(ops) => {
                    let (mut t, mut sc) = ([tx, ty], s);
                    for op in ops {
                        match op {
                            PoseOp::Translate(x, y) => {
                                t = [t[0] + x * sc, t[1] + y * sc];
                            }
                            PoseOp::Scale(x, _) => sc *= x,
                        }
                    }
                    self.poses.push((t, sc));
                }
                GuiDraw::Pop => {
                    self.poses.pop();
                }
                GuiDraw::Fill {
                    x0,
                    y0,
                    x1,
                    y1,
                    color,
                } => {
                    let r = [
                        tx + *x0 as f32 * s,
                        ty + *y0 as f32 * s,
                        (x1 - x0) as f32 * s,
                        (y1 - y0) as f32 * s,
                    ];
                    self.rect(r, *color);
                }
                GuiDraw::Text {
                    text, x, y, color, ..
                } => {
                    let TextContent::Literal(t) = &text.content else {
                        continue;
                    };
                    self.label(t, [tx + *x as f32 * s, ty + *y as f32 * s], s, *color);
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn item_stack(&mut self, _: &ItemStack, [x, y]: [f32; 2], size: f32) -> bool {
        self.rect([x + 1.0, y + 1.0, size - 2.0, size - 2.0], 0xFF8A8A9A);
        true
    }
}

fn gui(view: View) -> Gui {
    let mut g = Gui::new(view);
    g.opened = Instant::now() - Duration::from_secs(1);
    g
}

fn data() -> HudData {
    HudData {
        fps: 144,
        cps_left: 7,
        cps_right: 2,
        keys: Keys {
            forward: true,
            attack: true,
            ..Keys::default()
        },
        position: Some([-312.7, 71.0, 1024.2]),
        yaw: 200.0,
        dimension: "minecraft:overworld".into(),
        speed_ground: 5.61,
        speed_total: 5.61,
        effects: vec![Effect {
            name: "Strength".into(),
            amplifier: 1,
            ticks: 20 * 74,
        }],
        unix_seconds: 1_795_698_309,
        ..HudData::default()
    }
}

/// Every HUD module on, for the pictures.
fn all_on() -> Vec<Module> {
    let mut mods = modules::defaults();
    for m in &mut mods {
        m.enabled = true;
    }
    mods
}

fn frame(g: &mut Gui, size: [f32; 2], mouse: [f32; 2], mods: &[Module], name: &str) -> Recorder {
    let mut r = Recorder::new(size);
    g.draw(&mut r, size, mouse, mods, &data(), "Right Shift");
    r.write(name);
    r
}

fn centre_of(g: &Gui, target: Target) -> [f32; 2] {
    let (r, _) = g
        .hits
        .iter()
        .find(|(_, t)| *t == target)
        .unwrap_or_else(|| panic!("{target:?} drawn"));
    [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
}

const SIZE: [f32; 2] = [480.0, 270.0];

#[test]
fn the_editor_opens_the_menu_and_the_menu_the_editor() {
    let mut mods = all_on();
    let mut g = gui(View::Editor);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "editor");
    assert!(g.mouse_pressed(centre_of(&g, Target::OpenMenu), 0, &mut mods));
    assert_eq!(g.view, View::Menu);
    g.opened = Instant::now() - Duration::from_secs(1);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "menu");
    assert!(g.mouse_pressed(centre_of(&g, Target::EditHud), 0, &mut mods));
    assert_eq!(g.view, View::Editor);
}

#[test]
fn clicking_a_bar_toggles_its_module() {
    let mut mods = modules::defaults();
    let mut g = gui(View::Menu);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "grid");
    let before = mods[1].enabled;
    let at = centre_of(&g, Target::Toggle(1));
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert_eq!(mods[1].enabled, !before);
    assert!(g.dirty);
    frame(&mut g, SIZE, at, &mods, "grid-hover");
    assert!(!g.mouse_pressed(at, 1, &mut mods));
}

#[test]
fn tabs_and_search_filter_the_grid() {
    let mut mods = modules::defaults();
    let mut g = gui(View::Menu);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "grid");
    let at = centre_of(&g, Target::Tab(Some(Category::Utility)));
    assert!(g.mouse_pressed(at, 0, &mut mods));
    let r = frame(&mut g, SIZE, at, &mods, "utility");
    assert!(r.texts.iter().any(|(t, _)| t == "Zoom"));
    assert!(!r.texts.iter().any(|(t, _)| t == "FPS"));

    let mut g = gui(View::Menu);
    for c in "spri".chars() {
        assert!(g.typed(c));
    }
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "search");
    let names: Vec<_> = g.visible(&mods).into_iter().map(|i| mods[i].name).collect();
    assert_eq!(names, ["Toggle Sprint"]);
    assert!(g.backspace());
    let at = centre_of(&g, Target::ClearSearch);
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert!(g.search.is_empty());
    // The editor takes no typing.
    let mut e = gui(View::Editor);
    assert!(!e.typed('a'));
}

#[test]
fn options_switches_sliders_and_swatches_change_settings() {
    let mut mods = all_on();
    let fps = mods.iter().position(|m| m.id == Id::Keystrokes).unwrap();
    let mut g = gui(View::Menu);
    g.open_options(fps);
    g.opened = Instant::now() - Duration::from_secs(1);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "options");

    // The background switch.
    let bg = mods[fps]
        .settings
        .iter()
        .position(|s| s.key() == "background")
        .unwrap();
    let at = centre_of(
        &g,
        Target::Setting {
            module: fps,
            setting: bg,
        },
    );
    assert!(g.mouse_pressed(at, 0, &mut mods));
    assert!(!mods[fps].bool("background"));

    // The scale slider: a press at its right end, then a drag back to its left end.
    let scale = mods[fps]
        .settings
        .iter()
        .position(|s| s.key() == "scale")
        .unwrap();
    let (r, _) = *g
        .hits
        .iter()
        .find(|(_, t)| {
            *t == Target::Setting {
                module: fps,
                setting: scale,
            }
        })
        .unwrap();
    assert!(g.mouse_pressed([r[0] + r[2] - 4.0, r[1] + 5.0], 0, &mut mods));
    assert!((mods[fps].f32("scale") - 2.5).abs() < 1e-4);
    assert!(g.mouse_dragged([r[0] - 50.0, r[1] + 5.0], &mut mods));
    assert!((mods[fps].f32("scale") - 0.5).abs() < 1e-4);
    assert!(g.mouse_released());

    // A swatch.
    let colour = mods[fps]
        .settings
        .iter()
        .position(|s| s.key() == "text_colour")
        .unwrap();
    let target = Target::Swatch {
        module: fps,
        setting: colour,
        swatch: 2,
    };
    // The swatches may be below the fold: scroll down until they are drawn.
    for _ in 0..10 {
        if g.hits.iter().any(|(_, t)| *t == target) {
            break;
        }
        let c = [g.content[0] + 4.0, g.content[1] + 4.0];
        g.scroll(c, -1.0, &mut mods);
        frame(&mut g, SIZE, [0.0, 0.0], &mods, "options-scrolled");
    }
    assert!(g.mouse_pressed(centre_of(&g, target), 0, &mut mods));
    assert_eq!(mods[fps].colour("text_colour"), PALETTE[2]);

    // Reset puts it all back.
    for _ in 0..10 {
        let c = [g.content[0] + 4.0, g.content[1] + 4.0];
        g.scroll(c, -1.0, &mut mods);
        frame(&mut g, SIZE, [0.0, 0.0], &mods, "options-scrolled");
    }
    assert!(g.mouse_pressed(centre_of(&g, Target::ResetModule(fps)), 0, &mut mods));
    assert!(mods[fps].bool("background"));
    assert!((mods[fps].f32("scale") - 1.0).abs() < 1e-4);
}

#[test]
fn dragging_an_element_moves_and_snaps_it() {
    let mut mods = modules::defaults();
    let fps = mods.iter().position(|m| m.id == Id::Fps).unwrap();
    let mut g = gui(View::Editor);
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "editor-default");
    let start = centre_of(&g, Target::Element(fps));
    assert!(g.mouse_pressed(start, 0, &mut mods));
    // To near the middle of the screen: it snaps to the vertical middle line.
    let (_, r) = *g.elements.iter().find(|(m, _)| *m == fps).unwrap();
    let grab = [start[0] - r[0], start[1] - r[1]];
    let target_x = (SIZE[0] - r[2]) / 2.0 + 3.0;
    assert!(g.mouse_dragged([target_x + grab[0], 200.0 + grab[1]], &mut mods));
    let place = mods[fps].place.unwrap();
    assert!(((place.x * SIZE[0]) - (SIZE[0] - r[2]) / 2.0).abs() < 0.01);
    assert_eq!(g.guides[0], Some(SIZE[0] / 2.0));
    frame(
        &mut g,
        SIZE,
        [target_x + grab[0], 200.0 + grab[1]],
        &mods,
        "editor-drag",
    );
    assert!(g.mouse_released());
    assert_eq!(g.guides, [None, None]);
    assert!(g.dirty);

    // Scrolling over it resizes it; a right-click opens its options.
    frame(&mut g, SIZE, [0.0, 0.0], &mods, "editor-moved");
    let at = centre_of(&g, Target::Element(fps));
    assert!(g.scroll(at, 1.0, &mut mods));
    assert!((mods[fps].f32("scale") - 1.1).abs() < 1e-4);
    assert!(g.mouse_pressed(at, 1, &mut mods));
    assert_eq!(g.view, View::Menu);
    assert_eq!(g.page, Page::Options(fps));
}

#[test]
fn elements_stay_on_small_screens_and_reset() {
    let mut mods = all_on();
    let mut g = gui(View::Editor);
    let size = [320.0, 180.0];
    frame(&mut g, size, [0.0, 0.0], &mods, "editor-small");
    for (_, r) in &g.elements {
        assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= size[0] && r[1] + r[3] <= size[1]);
    }
    mods[0].place = Some(Place { x: 0.5, y: 0.5 });
    assert!(g.mouse_pressed(centre_of(&g, Target::ResetLayout), 0, &mut mods));
    assert_eq!(mods[0].place, mods[0].default_place);
}

#[test]
fn the_hud_preview() {
    // The world's HUD as the game draws it, for the README's picture.
    let mut mods = all_on();
    for m in &mut mods {
        if m.id == Id::Clock || m.id == Id::Speed {
            m.enabled = false;
        }
    }
    let mut r = Recorder::new(SIZE);
    let d = data();
    for m in &mods {
        let (true, Some(place)) = (m.enabled, m.place) else {
            continue;
        };
        let Some(content) = crate::hud::editor_content(m, &d) else {
            continue;
        };
        let scale = m.f32("scale");
        let unscaled = crate::hud::size(&content, &|s| r.text_width(s));
        let rect = crate::hud::rect(place, scale, unscaled, SIZE);
        crate::hud::draw(&mut r, m, &content, [rect[0], rect[1]], scale);
    }
    assert!(r.texts.iter().any(|(t, _)| t == "144 FPS"));
    assert!(r.texts.iter().any(|(t, _)| t == "X: -313"));
    r.write("hud");
}
