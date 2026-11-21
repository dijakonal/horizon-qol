fn voicechat_tick(game: &mut crate::game::Game<'_>) {
    // Keys chosen in Horizon QoL's menu: bound and saved as Options > Controls does.
    let rebinds = mod_horizon_qol::take_rebinds();
    if !rebinds.is_empty() {
        for (name, key) in &rebinds {
            match key {
                Some(key) => {
                    game.settings
                        .mods
                        .set_binding(mod_horizon_qol::MOD_ID, name, key, 0)
                }
                None => game.settings.mods.reset_binding(mod_horizon_qol::MOD_ID, name),
            }
        }
        game.apply_settings();
        game.settings.save(&crate::game::Game::options_path());
    }
    use mod_horizon_qol::voice::engine as vc;
    crate::game::remote::voicechat_audio::ensure_installed();
    let Some(r) = game.remote.as_ref() else {
        return;
    };
    if !(game.on(vc::ID) && vc::enabled()) || !r.joined || r.reconfiguring.is_some() {
        vc::tick(None, &|_| None);
        return;
    }
    let address = r.session.address.clone();
    let players: Vec<(u128, [f64; 3])> = r
        .level_players
        .iter()
        .filter_map(|(&id, &uuid)| {
            let id = u32::try_from(id).ok()?;
            let mob = game.mobs.iter().find(|mob| mob.id == id)?;
            Some((
                uuid,
                [
                    mob.position.x,
                    mob.position.y + vc::EYE_HEIGHT,
                    mob.position.z,
                ],
            ))
        })
        .collect();
    let listener = game.native_player().map(|p| vc::mix::Listener {
        eye: [p.x, p.y + vc::EYE_HEIGHT, p.z],
        yaw: p.yaw,
    });
    let world = vc::World {
        address: &address,
        listener,
        players,
        redirect: horizon_protocol::server_redirect,
    };
    let done = vc::tick(Some(world), &mod_horizon_qol::voice::config::get);
    for (channel, data) in done.payloads {
        game.send(horizon_protocol::packets::play::Serverbound::CustomPayload(
            horizon_protocol::packets::configuration::CustomPayload { channel, data },
        ));
    }
    for line in done.chat {
        game.native_chat(&line);
    }
}
