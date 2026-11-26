fn qol_cosmetics_worn(
    value: Option<horizon_cosmetics::Worn>,
    m: &PlayerModel,
) -> Option<horizon_cosmetics::Worn> {
    // Horizon QoL's Cosmetics module: what its player put on, on their own model only.
    // Every other player wears what the service answered, as the game asked it.
    if m.kind != Kind::Own {
        return value;
    }
    crate::game::cosmetics::qol_cosmetics::worn()
}
