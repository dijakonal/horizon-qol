fn horizon_qol_chat_heads_room(game: &crate::game::Game<'_>, offset: [f32; 2]) -> [f32; 2] {
    // Horizon QoL's Player Heads: the chat is drawn a head's width to the right (at the chat's
    // own scale), and the faces go in the room it leaves (`chat_heads_draw.rs`).
    if mod_horizon_qol::chat_heads() {
        [offset[0] + 12.0 * game.ui.chat.options.scale as f32, offset[1]]
    } else {
        offset
    }
}
