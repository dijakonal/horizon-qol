fn horizon_qol_wardrobe(game: &mut crate::game::Game<'_>) {
    // Horizon QoL's skin manager: the picked skin and cape on the player's own model, after the
    // game has set the account's and before the frame uses them.
    crate::game::remote::qol_wardrobe::apply(game);
}
