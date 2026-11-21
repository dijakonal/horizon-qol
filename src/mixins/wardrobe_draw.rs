fn horizon_qol_wardrobe(game: &mut crate::game::Game<'_>) {
    // Horizon QoL's skin manager: the picked skin and cape on the player's own model, just after
    // the game has set the account's (update_remote_skins) and before the frame draws them.
    crate::game::remote::qol_wardrobe::apply(game);
}
