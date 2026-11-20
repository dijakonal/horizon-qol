fn voicechat_name_tag(
    game: &crate::game::Game<'_>,
    o: glam::DVec3,
    lines: &[(horizon_client::text::Text, f64)],
    text_alpha: u32,
    background: bool,
    text_layer: u32,
    light: u32,
    unit: f32,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) {
    use mod_horizon_qol::voice::engine as vc;
    if !(game.on(vc::ID) && vc::enabled()) {
        return;
    }
    let Some((text, lift)) = lines.last() else {
        return;
    };
    let plain = text.plain();
    let Some(icon) = vc::name_tag_icon(&plain) else {
        return;
    };
    let Some(sprite) = game.gui.atlas.sprites.get(icon).copied() else {
        return;
    };
    let (yaw, pitch) = if game.camera_type == horizon_client::camera::CameraType::ThirdPersonFront
    {
        (game.yaw + 180.0, -game.pitch)
    } else {
        (game.yaw, game.pitch)
    };
    let forward = glam::DVec3::from_array(crate::game::view_vector(yaw, pitch));
    let right = forward.cross(glam::DVec3::Y).normalize_or_zero().as_vec3();
    let up = right.cross(forward.as_vec3()).normalize_or_zero() * unit;
    let right = right * unit;
    let o = (o + glam::DVec3::new(0.0, *lift, 0.0)).as_vec3();
    let at = |x: f32, y: f32| o + right * x - up * y;
    let name_w = horizon_client::hud::HudBatch::text_width(&game.gui.atlas, &plain);
    let x = -name_w / 2.0 - 10.0;
    let y = if plain == "deadmau5" { -10.0 } else { 0.0 };
    let mut batch = horizon_client::hud::HudBatch::new([1, 1], game.settings.gui_scale_inputs());
    if background {
        batch.fill(&game.gui.atlas, x - 1.0, y - 1.0, 10.0, 10.0, 0x3F00_0000);
        batch.fill(
            &game.gui.atlas,
            name_w / 2.0,
            y - 1.0,
            10.0,
            10.0,
            0x3F00_0000,
        );
    }
    batch.rect_uv(x, y, 8.0, 8.0, sprite, text_alpha << 24 | 0xFF_FFFF);
    for (i, quad) in batch.vertices.as_chunks::<6>().0.iter().enumerate() {
        let shift = if background && i < 2 {
            forward.as_vec3() * 0.002
        } else {
            glam::Vec3::ZERO
        };
        for v in [quad[5], quad[2], quad[1], quad[0]] {
            out.push(horizon_client::mesher::Vertex {
                normal: [0; 4],
                pos: (at(v.pos[0], v.pos[1]) + shift).to_array(),
                uv: v.uv,
                layer: text_layer,
                color: v.color,
                light,
                glint_uv: [0.0, 0.0],
            });
        }
    }
}
