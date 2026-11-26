fn qol_cosmetics_tag(
    value: Option<horizon_cosmetics::Worn>,
    m: &PlayerModel,
) -> Option<horizon_cosmetics::Worn> {
    // Horizon QoL's Cosmetics module: its player's name tag style, and how tall what they
    // wear stands (the tag lifts over it), on their own model only.
    if m.kind != Kind::Own {
        return value;
    }
    crate::game::cosmetics::qol_cosmetics::worn()
}
