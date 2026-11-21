fn horizon_qol_fullbright(game: &mut crate::game::Game<'_>) -> ::core::ops::ControlFlow<()> {
    // Horizon QoL's Fullbright module: full light wherever the player is, servers included
    // (Horizon's own Fullbright holds itself off on another's server). Off, the game decides.
    if !mod_horizon_qol::fullbright() {
        return ::core::ops::ControlFlow::Continue(());
    }
    game.settings.fullbright_held = false;
    if let Some(renderer) = game.renderer.as_mut() {
        renderer.fullbright = true;
    }
    ::core::ops::ControlFlow::Break(())
}
