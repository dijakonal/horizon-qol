fn voicechat_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use horizon_client::extras::Corner;
    use mod_horizon_qol::voice::engine as vc;
    if !(game.on(vc::ID) && vc::enabled()) {
        return;
    }
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
    if game.native_corner_shown(Corner::TopLeft) {
        if let Some(group) = vc::hud_group() {
            let icon = group
                .icon
                .filter(|&name| game.gui_sprite_pixels(name).is_some());
            let icon_w = if icon.is_some() { 12.0 } else { 0.0 };
            let w = icon_w + game.native_text_width(batch, &group.text) + 4.0;
            let (x, y) = corners.place(Corner::TopLeft, w, 12.0);
            game.native_fill(batch, [x, y, w, 12.0], 0x9000_0000);
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
        }
        let mute_badge = game
            .gui_sprite_pixels("horizon_qol:gui/voice/icon_mute")
            .is_some();
        let max_heads = (batch.height / 32.0).floor().max(3.0) as usize;
        let heads = vc::hud_heads(max_heads);
        let mut tab_skins = game
            .remote_tab_players()
            .map(|(players, _, _)| {
                players
                    .into_iter()
                    .map(|p| (p.uuid, p.skin))
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .unwrap_or_default();
        for head in heads {
            let name_w = game.native_text_width(batch, &head.name);
            let w = name_w + 24.0;
            let (x, y) = corners.place(Corner::TopLeft, w, 16.0);
            game.native_fill(batch, [x, y, w, 16.0], 0x7000_0000);
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
            game.native_text(
                batch,
                &head.name,
                [x + 20.0, y + 4.0],
                if head.talking { 0xFFFF_FFFF } else { 0xFFAA_AAAA },
                true,
            );
        }
    }
    if !game.native_corner_shown(Corner::BottomLeft) {
        return;
    }
    for line in vc::hud() {
        let icon = line
            .icon
            .filter(|&name| game.gui_sprite_pixels(name).is_some());
        let icon_w = if icon.is_some() { 12.0 } else { 0.0 };
        let w = icon_w + game.native_text_width(batch, &line.text) + 4.0;
        let (x, y) = corners.place(Corner::BottomLeft, w, 12.0);
        game.native_fill(batch, [x, y, w, 12.0], 0x9000_0000);
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
            &line.text,
            [x + icon_w + 2.0, y + 2.0],
            line.rgb | 0xFF00_0000,
            true,
        );
    }
}
