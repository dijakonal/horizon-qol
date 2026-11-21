fn voicechat_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    // The voice HUD's three elements, each where Horizon QoL's HUD editor put it and at the
    // scale it was given there: each is drawn unscaled at its top left, then scaled about it.
    use mod_horizon_qol::voice as qv;
    use qv::engine as vc;
    let _ = corners;
    if !game.on(vc::ID) || !vc::enabled() {
        return;
    }
    let screen = [batch.width, batch.height];
    let draw_face = |batch: &mut horizon_client::hud::HudBatch,
                     atlas: &horizon_client::hud::HudAtlas,
                     skin: &str,
                     x: f32,
                     y: f32,
                     size: f32| {
        let Some(base) = atlas.sprites.get(skin).copied() else {
            return;
        };
        let du = (base.u1 - base.u0) / 64.0;
        let dv = (base.v1 - base.v0) / 64.0;
        for u in [8.0, 40.0] {
            let mut face = base;
            face.u0 = base.u0 + u * du;
            face.u1 = base.u0 + (u + 8.0) * du;
            face.v0 = base.v0 + 8.0 * dv;
            face.v1 = base.v0 + 16.0 * dv;
            batch.rect_uv(x, y, size, size, face, u32::MAX);
        }
    };

    // The group the player is in.
    if let Some(group) = vc::hud_group()
        && let Some(slot) = qv::hud_slot(qv::HUD_GROUP)
    {
        let icon = group
            .icon
            .filter(|&name| game.gui_sprite_pixels(name).is_some());
        let icon_w = if icon.is_some() { 12.0 } else { 0.0 };
        let w = icon_w + game.native_text_width(batch, &group.text) + 4.0;
        if let Some(([x, y], scale)) = qv::hud_place(qv::HUD_GROUP, [w, 12.0], screen) {
            let start = batch.vertices.len();
            if slot.background >> 24 != 0 {
                game.native_fill(batch, [x, y, w, 12.0], slot.background);
            }
            if let Some(name) = icon {
                game.native_sprite(
                    batch,
                    name,
                    [0.0, 0.0, 16.0, 16.0],
                    [x + 1.0, y + 1.0, 10.0, 10.0],
                    None,
                );
            }
            game.native_text(
                batch,
                &group.text,
                [x + icon_w + 2.0, y + 2.0],
                group.rgb | 0xFF00_0000,
                true,
            );
            batch.scale_since(start, x, y, scale[0], scale[1]);
        }
    }

    // The players talking, and the group's members.
    if let Some(slot) = qv::hud_slot(qv::HUD_SPEAKERS) {
        let heads = vc::hud_heads(slot.limit.max(1));
        if !heads.is_empty() {
            let mute_badge = game
                .gui_sprite_pixels("horizon_qol:gui/voice/icon_mute")
                .is_some();
            let w = heads
                .iter()
                .map(|h| {
                    if slot.names {
                        game.native_text_width(batch, &h.name) + 24.0
                    } else {
                        16.0
                    }
                })
                .fold(16.0_f32, f32::max);
            let h = heads.len() as f32 * 17.0 - 1.0;
            if let Some(([x, y0], scale)) = qv::hud_place(qv::HUD_SPEAKERS, [w, h], screen) {
                let start = batch.vertices.len();
                let mut tab_skins = game
                    .remote_tab_players()
                    .map(|(players, _, _)| {
                        players
                            .into_iter()
                            .map(|p| (p.uuid, p.skin))
                            .collect::<std::collections::HashMap<_, _>>()
                    })
                    .unwrap_or_default();
                for (i, head) in heads.iter().enumerate() {
                    let y = y0 + i as f32 * 17.0;
                    let row_w = if slot.names {
                        game.native_text_width(batch, &head.name) + 24.0
                    } else {
                        16.0
                    };
                    if slot.background >> 24 != 0 {
                        game.native_fill(batch, [x, y, row_w, 16.0], slot.background);
                    }
                    let own = game
                        .remote
                        .as_ref()
                        .is_some_and(|r| r.profile() == head.uuid);
                    let skin = if own {
                        game.player_model.as_ref().and_then(|m| m.skin.clone())
                    } else {
                        tab_skins.remove(&head.uuid).or_else(|| {
                            game.remote
                                .as_ref()
                                .and_then(|r| r.player_entity(head.uuid))
                                .and_then(|id| game.mobs.iter().find(|m| m.id == id))
                                .and_then(|m| m.skin.clone())
                        })
                    }
                    .unwrap_or_else(|| horizon_client::tab_list::default_skin(head.uuid).to_owned());
                    let skin = game.face_skin(head.uuid, &skin);
                    draw_face(batch, &game.gui.atlas, &skin, x + 1.0, y + 1.0, 14.0);
                    if !head.talking {
                        game.native_fill(batch, [x + 1.0, y + 1.0, 14.0, 14.0], 0x4000_0000);
                    }
                    let frame = if head.talking {
                        0xFFFF_FFFF
                    } else {
                        0x70AA_AAAA
                    };
                    game.native_fill(batch, [x, y, 16.0, 1.0], frame);
                    game.native_fill(batch, [x, y + 15.0, 16.0, 1.0], frame);
                    game.native_fill(batch, [x, y, 1.0, 16.0], frame);
                    game.native_fill(batch, [x + 15.0, y, 1.0, 16.0], frame);
                    if head.muted && mute_badge {
                        game.native_fill(batch, [x + 8.0, y + 8.0, 8.0, 8.0], 0xB010_1014);
                        game.native_sprite(
                            batch,
                            "horizon_qol:gui/voice/icon_mute",
                            [0.0, 0.0, 16.0, 16.0],
                            [x + 8.0, y + 8.0, 8.0, 8.0],
                            None,
                        );
                    }
                    if slot.names {
                        game.native_text(
                            batch,
                            &head.name,
                            [x + 20.0, y + 4.0],
                            if head.talking { 0xFFFF_FFFF } else { 0xFFAA_AAAA },
                            true,
                        );
                    }
                }
                batch.scale_since(start, x, y0, scale[0], scale[1]);
            }
        }
    }

    // The connection and the player's own voice: talking, muted, off. Each status with an icon
    // is the icon alone, in a 16-pixel square, unless the element also shows its text.
    if let Some(slot) = qv::hud_slot(qv::HUD_STATUS) {
        let lines = vc::hud();
        if !lines.is_empty() {
            let rows: Vec<_> = lines
                .iter()
                .map(|line| {
                    let icon = line
                        .icon
                        .filter(|&name| game.gui_sprite_pixels(name).is_some());
                    if icon.is_some() && !slot.names {
                        return (icon, true, 16.0, 16.0);
                    }
                    let icon_w = if icon.is_some() { 12.0 } else { 0.0 };
                    let w = icon_w + game.native_text_width(batch, &line.text) + 4.0;
                    (icon, false, w, 12.0)
                })
                .collect();
            let w = rows.iter().map(|r| r.2).fold(0.0_f32, f32::max);
            let h = rows.iter().map(|r| r.3 + 1.0).sum::<f32>() - 1.0;
            if let Some(([x, y0], scale)) = qv::hud_place(qv::HUD_STATUS, [w, h], screen) {
                let start = batch.vertices.len();
                let mut y = y0;
                for (line, (icon, alone, row_w, row_h)) in lines.iter().zip(rows) {
                    if slot.background >> 24 != 0 {
                        game.native_fill(batch, [x, y, row_w, row_h], slot.background);
                    }
                    if let Some(name) = icon {
                        let s = if alone { 14.0 } else { 10.0 };
                        game.native_sprite(
                            batch,
                            name,
                            [0.0, 0.0, 16.0, 16.0],
                            [x + 1.0, y + 1.0, s, s],
                            None,
                        );
                    }
                    if !alone {
                        let icon_w = if icon.is_some() { 12.0 } else { 0.0 };
                        game.native_text(
                            batch,
                            &line.text,
                            [x + icon_w + 2.0, y + 2.0],
                            line.rgb | 0xFF00_0000,
                            true,
                        );
                    }
                    y += row_h + 1.0;
                }
                batch.scale_since(start, x, y0, scale[0], scale[1]);
            }
        }
    }
}
