fn horizon_qol_config(game: &mut Game<'_>, i: usize) -> ::core::ops::ControlFlow<()> {
    // The Mods screen's Config button on Horizon QoL: in a world, open the mod's own menu over
    // it instead of the generic settings page. On the title screen no mod screen can open, so
    // the generic page (the mod's one setting, and where to find the menu) shows as usual.
    let ours = game
        .settings
        .mods
        .installed
        .get(i)
        .is_some_and(|m| m.id == mod_horizon_qol::MOD_ID);
    if !ours || game.title.is_some() || game.player.is_none() {
        return ::core::ops::ControlFlow::Continue(());
    }
    let back = game.screen;
    game.set_screen(crate::game::Screen::Playing);
    mod_horizon_qol::config_button();
    if game.mod_screen_open(mod_horizon_qol::MOD_ID) {
        ::core::ops::ControlFlow::Break(())
    } else {
        game.set_screen(back);
        ::core::ops::ControlFlow::Continue(())
    }
}
