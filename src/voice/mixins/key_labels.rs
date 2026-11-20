use mod_horizon_qol::voice::engine as vc;

pub(crate) fn translate(game: &mut crate::game::Game<'_>) {
    if !game.on(vc::ID) {
        return;
    }
    let Some(page) = game
        .settings
        .mods
        .installed
        .iter_mut()
        .find(|installed| installed.id == vc::ID)
        .and_then(|installed| installed.native.as_mut())
    else {
        return;
    };
    for (id, label, _) in vc::HOTKEYS {
        if let Some(mapping) = page.mappings.iter_mut().find(|mapping| mapping.id == id)
            && let Some(text) = vc::i18n::game_translation(label)
        {
            mapping.label = text;
        }
    }
}
