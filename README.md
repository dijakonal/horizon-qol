# Horizon QoL

Quality-of-life tweaks for [Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of
Minecraft: Java Edition 26.3, with a client-style mod menu.

**Status:** the menu works; the modules in it are placeholders. Switching one on does nothing in
game yet.

![The mod menu](docs/menu.png)

![A module's options page](docs/options.png)

## What it does

- **Right Shift** opens the mod menu over the world. Right Shift or Esc closes it. The key can be
  changed under *Options > Controls > Horizon QoL*.
- The menu has a sidebar of categories (All, HUD, PvP, Utility, Visual), a grid of module cards
  with an ENABLED / DISABLED bar, and a search box: start typing to filter, Backspace to delete,
  click the `x` to clear.
- The `...` button on a card opens that module's options page. It has a working on/off switch and
  placeholder rows for settings that will come later.
- The menu blurs the world behind it, using the blur strength from your video settings.
- It is client-only (`side = "client"`), so it works on any server.

Module on/off states are kept in memory, so they reset when the game restarts.

## Building and installing

This is a native Horizon mod. You need the Horizon launcher (`horizon-launcher`) with an instance,
and the mod tool:

```sh
cargo install --locked --git https://github.com/horizon-rust/horizon cargo-horizon-mod
```

Then, from this folder:

```sh
cargo horizon-mod update            # fetches the SDK into .horizon/sdk/ (and moves the pin to the newest Nightly)
horizon-launcher native-build . --instance "My Instance" --trust-native-code
```

Start the instance, join a world and press Right Shift.

`horizon-mod.toml` pins the Nightly of 2026-11-20 (`0a875da`). The launcher builds the mod against
that exact game build, so run `cargo horizon-mod update` to move to a newer one.

## Layout

```text
horizon-mod.toml          the manifest: id, side, and the pinned Horizon build
Cargo.toml                depends only on horizon-mod-api from the pinned SDK
src/lib.rs                the hooks: the key mapping, opening and closing the screen, routing input
src/ui.rs                 the menu: layout, drawing, clicks, scroll and search
src/modules.rs            the placeholder module list and categories
assets/horizon_qol/lang/  names for the key mapping in Options > Controls
```

## Tests

`cargo test` checks the menu's layout and clicks with a fake draw context, without the game.
`HORIZON_QOL_PREVIEW=<folder> cargo test` also writes each tested frame as an SVG, which is how
`docs/menu.png` was made.

## Next steps

- Make the modules real, starting with the HUD ones (FPS, CPS, Keystrokes, Coordinates) through a
  `gui_render_post` HUD layer.
- Save module states and settings to a config file.
- A HUD editor for dragging HUD modules around the screen.
