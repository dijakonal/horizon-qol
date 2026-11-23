fn horizon_qol_locator_heads(
    game: &crate::game::Game<'_>,
    draws: Vec<horizon_client::hud_layout::GuiDraw>,
) -> Vec<horizon_client::hud_layout::GuiDraw> {
    // Horizon QoL's Player Heads: each player's dot on the locator bar becomes their face, framed
    // in the dot's colour. A dot is told by its colour, which the game makes from the player's
    // UUID (`TrackedWaypoint`'s default colour); a dot a server coloured itself stays a dot.
    use horizon_client::hud_layout::{Blend, GuiDraw};
    if !mod_horizon_qol::locator_heads() {
        return draws;
    }
    let atlas = &game.gui.atlas;
    let uuid_hash = |u: u128| {
        let hilo = (u >> 64) as u64 ^ u as u64;
        ((hilo >> 32) as i32) ^ (hilo as i32)
    };
    let players: Vec<(u32, &String)> = atlas
        .object_skins
        .iter()
        .filter_map(|(key, skin)| {
            let uuid = u128::from_str_radix(key.strip_prefix("id:")?, 16).ok()?;
            let colour = horizon_client::locator_bar::set_brightness(
                0xFF00_0000 | uuid_hash(uuid) as u32,
                0.9,
            );
            Some((colour, skin))
        })
        .collect();
    let mut out = Vec::with_capacity(draws.len());
    for draw in draws {
        let head = match &draw {
            GuiDraw::Sprite {
                sprite,
                x,
                y,
                w: 9,
                h: 9,
                color,
                ..
            } if !sprite.contains("arrow") => players
                .iter()
                .find(|(c, skin)| c == color && atlas.sprites.contains_key(skin.as_str()))
                .map(|(c, skin)| (*x, *y, *c, (*skin).clone())),
            _ => None,
        };
        let Some((x, y, colour, skin)) = head else {
            out.push(draw);
            continue;
        };
        out.push(GuiDraw::Fill {
            x0: x,
            y0: y,
            x1: x + 9,
            y1: y + 9,
            color: colour,
        });
        for uv in [[8, 8], [40, 8]] {
            out.push(GuiDraw::Sprite {
                sprite: skin.clone().into(),
                x: x + 1,
                y: y + 1,
                w: 7,
                h: 7,
                region: Some([64, 64, uv[0], uv[1]]),
                color: u32::MAX,
                blend: Blend::Textured,
            });
        }
    }
    out
}
