fn horizon_qol_scoreboard(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> ::core::ops::ControlFlow<()> {
    // Horizon QoL's Scoreboard module: the game's own sidebar layout, moved to where the HUD
    // editor put it, scaled, and restyled. With the module off, the game draws its own.
    use horizon_client::hud_layout::GuiDraw;
    let Some(look) = mod_horizon_qol::scoreboard::look() else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if !look.shown {
        return ::core::ops::ControlFlow::Break(());
    }
    let board = &game.score.board;
    let Some(objective) = game.sidebar_objective() else {
        return ::core::ops::ControlFlow::Break(());
    };
    let mut objective = objective.clone();
    if !look.numbers {
        objective.number_format = Some(horizon_client::scoreboard::NumberFormat::Blank);
    }
    let font = &game.gui.atlas.font;
    let layout = |w: i32, h: i32| {
        let mut draws = horizon_client::hud_overlays::sidebar(
            w,
            h,
            board,
            &objective,
            font,
            look.background,
            look.header,
        );
        // The title band is the first fill, the title the first text.
        if !look.title {
            if let Some(i) = draws.iter().position(|d| matches!(d, GuiDraw::Fill { .. })) {
                draws.remove(i);
            }
            if let Some(i) = draws.iter().position(|d| matches!(d, GuiDraw::Text { .. })) {
                draws.remove(i);
            }
        }
        draws.retain(|d| !matches!(d, GuiDraw::Fill { color, .. } if *color >> 24 == 0));
        for d in &mut draws {
            if let GuiDraw::Text { shadow, .. } = d {
                *shadow = look.shadow;
            }
        }
        draws
    };
    // Laid out once to measure it (its box is its fills and lines), then where it goes.
    let bounds = |draws: &[GuiDraw]| {
        let mut b = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
        for d in draws {
            let r = match d {
                GuiDraw::Fill { x0, y0, x1, y1, .. } => [*x0, *y0, *x1, *y1],
                GuiDraw::Text { x, y, .. } => [*x, *y, *x, *y + 9],
                _ => continue,
            };
            b = [b[0].min(r[0]), b[1].min(r[1]), b[2].max(r[2]), b[3].max(r[3])];
        }
        (b[0] <= b[2]).then_some(b)
    };
    let Some(b) = bounds(&layout(0, 0)) else {
        return ::core::ops::ControlFlow::Break(());
    };
    let size = [(b[2] - b[0]) as f32, (b[3] - b[1]) as f32];
    let Some(([x, y], scale)) =
        mod_horizon_qol::scoreboard::place(size, [batch.width, batch.height])
    else {
        return ::core::ops::ControlFlow::Break(());
    };
    // The layout's x moves with the screen width, its y with half the screen height.
    let (dx, dy) = (x.round() as i32 - b[0], y.round() as i32 - b[1]);
    let draws = layout(dx, dy * 2);
    let start = batch.vertices.len();
    horizon_client::hud_layout::paint(batch, &game.gui.atlas, &draws);
    batch.scale_since(start, x.round(), y.round(), scale[0], scale[1]);
    ::core::ops::ControlFlow::Break(())
}
