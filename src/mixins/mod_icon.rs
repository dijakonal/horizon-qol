fn horizon_qol_mod_icon(game: &mut crate::game::Game<'_>) {
    // The Mods screen shows no icon for a native mod (the game lists it without its
    // package's icon.png): give Horizon QoL's row its logo.
    if let Some(info) = game.settings.mods.info.get_mut(mod_horizon_qol::MOD_ID) {
        if info.icon.is_none() {
            info.icon = Some(std::sync::Arc::<[u8]>::from(mod_horizon_qol::ICON_PNG));
        }
    }
}
