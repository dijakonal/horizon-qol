fn horizon_qol_chat_heads_draw(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    now: f32,
    focused: bool,
    offset: [f32; 2],
) {
    // Horizon QoL's Player Heads: the sender's face left of each chat line that names a player,
    // laid out as the chat laid its lines (`Chat::layout`) and faded with them. The sender is the
    // player a line's hover shows (vanilla's and most servers' names), else the first of its
    // first words that is a listed player's name.
    use horizon_client::hud_layout::{Blend, GuiDraw, PoseOp};
    if !mod_horizon_qol::chat_heads() {
        return;
    }
    let atlas = &game.gui.atlas;
    let skins = &atlas.object_skins;
    let uuid_of = |v: &serde_json::Value| -> Option<u128> {
        let u = &v["uuid"];
        if let Some(s) = u.as_str() {
            return u128::from_str_radix(&s.replace('-', ""), 16).ok();
        }
        let parts = u.as_array()?;
        if parts.len() != 4 {
            return None;
        }
        parts.iter().try_fold(0u128, |acc, p| {
            Some(acc << 32 | u128::from(p.as_i64()? as i32 as u32))
        })
    };
    let sender_skin = |text: &horizon_client::text::Text| -> Option<String> {
        for span in &text.spans {
            if let Some(entity) = &span.style.hover_entity
                && let Some(uuid) = uuid_of(entity)
                && let Some(skin) = skins.get(&format!("id:{uuid:032x}"))
            {
                return Some(skin.clone());
            }
        }
        let plain = text.plain();
        plain
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|w| (3..=16).contains(&w.len()))
            .take(4)
            .find_map(|w| skins.get(&format!("name:{}", w.to_ascii_lowercase())).cloned())
    };
    // Where the chat was drawn: moved right as `chat_heads_room.rs` moved it.
    let shift = 12.0 * game.ui.chat.options.scale as f32;
    let mut out = vec![GuiDraw::Push(vec![PoseOp::Translate(
        offset[0] + shift,
        offset[1],
    )])];
    let mut any = false;
    for draw in game.ui.chat.layout(batch.height as i32, now, focused) {
        match draw {
            GuiDraw::Push(_) | GuiDraw::Pop => out.push(draw),
            GuiDraw::Text { text, x, y, color, .. } => {
                let Some(skin) = sender_skin(&text).filter(|s| atlas.sprites.contains_key(s))
                else {
                    continue;
                };
                let tint = (color & 0xFF00_0000) | 0x00FF_FFFF;
                for uv in [[8, 8], [40, 8]] {
                    out.push(GuiDraw::Sprite {
                        sprite: skin.clone().into(),
                        x: x - 12,
                        y,
                        w: 8,
                        h: 8,
                        region: Some([64, 64, uv[0], uv[1]]),
                        color: tint,
                        blend: Blend::Textured,
                    });
                }
                any = true;
            }
            _ => {}
        }
    }
    out.push(GuiDraw::Pop);
    if any {
        horizon_client::hud_layout::paint(batch, atlas, &out);
    }
}
