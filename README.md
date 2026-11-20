# Horizon QoL

Quality-of-life tweaks for [Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of
Minecraft: Java Edition 26.3, with a client-style mod menu.

**Status:** the menu works; the modules in it are placeholders. Switching one on does nothing in
game yet.

![The mod menu](docs/menu.png)

![A module's options page](docs/options.png)

## What it does

- **Right Shift** opens the mod menu over the world. Right Shift or Esc closes it. The key is
  listed under *Options > Controls*, in the Horizon QoL section as **Open Mod Menu**, where you can
  rebind it.
- The menu has a sidebar of categories (All, HUD, PvP, Utility, Visual), a grid of module cards
  with an ENABLED / DISABLED bar, and a search box: start typing to filter, Backspace to delete,
  click the `x` to clear.
- The `...` button on a card opens that module's options page. It has a working on/off switch and
  placeholder rows for settings that will come later.
- The menu blurs the world behind it, using the blur strength from your video settings.
- It is client-only (`side = "client"`), so it works on any server.

Module on/off states are kept in memory, so they reset when the game restarts.

## Building and installing

This is a native Horizon mod: the launcher compiles it for your instance. You need the Horizon
launcher and the mod tool:

```sh
cargo install --locked --git https://github.com/horizon-rust/horizon cargo-horizon-mod
```

### From this folder (works for a private repo)

1. Pack the mod. This fetches the pinned SDK into `.horizon/sdk/` the first time:

   ```sh
   cargo horizon-mod build
   ```

   It writes `target/horizon-mod/horizon_qol.hmod`.
2. In the launcher, open **Mods**, then **Open folder**: that is the instance's game folder (the one
   with `options.txt`). Copy `horizon_qol.hmod` into its `horizon-native-mods/` folder (make it if
   it isn't there).
3. Turn the mod on: in the same game folder, add this line to `horizon-mods.txt` (make the file if
   it isn't there), or flip its switch on the Mods page:

   ```text
   mod.horizon_qol:true
   ```
4. Press **Build** on the Mods page (or **Play**), then join a world.

When the mod has loaded, chat says **"[Horizon QoL] Loaded. Press Right Shift to open the menu."**
when you join a world. No line means the launcher didn't build it: check step 3 and the build log
(`native/build.log`).

After a change, run `cargo horizon-mod build` again, copy the new `.hmod` over the old one, and build.

### From GitHub (public repo only)

On the launcher's **Mods** page, **Add from GitHub** with `https://github.com/dijakonal/horizon-qol`.

### The pin

`horizon-mod.toml` pins the Nightly of 2026-11-20 (`0a875da`). The launcher builds the mod against
that exact game build. `cargo horizon-mod update` moves the pin to the newest Nightly.

## Layout

```text
horizon-mod.toml          the manifest: id, side, and the pinned Horizon build
Cargo.toml                depends only on horizon-mod-api from the pinned SDK
src/lib.rs                the hooks: the Controls key mapping, opening and closing the screen, routing input, the loaded line in chat
src/ui.rs                 the menu: layout, drawing, clicks, scroll and search
src/modules.rs            the placeholder module list and categories
assets/horizon_qol/lang/  the key mapping's name in Options > Controls
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
