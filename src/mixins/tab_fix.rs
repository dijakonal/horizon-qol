fn horizon_qol_tab_fix(game: &crate::game::Game<'_>, blocked: bool) -> bool {
    // Horizon QoL's Inventory Tab Fix: in the inventory (and the creative one), Tab and the arrows
    // don't move the keyboard's focus, so Tab doesn't select the recipe book. A key bound to Tab
    // still works: the game hands it to its mappings separately.
    blocked
        || (mod_horizon_qol::tab_fix()
            && game.title.is_none()
            && matches!(
                game.screen,
                crate::game::Screen::Inventory | crate::game::Screen::Creative
            ))
}
