fn voicechat_tick(game: &mut crate::game::Game<'_>) {
    // Horizon QoL's skin manager: the game's PNG decoder for its previews, and its account jobs.
    crate::game::remote::qol_wardrobe::tick();
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
    // Horizon QoL's Key Remaps: the game's own keys chosen in its menu, bound as Controls
    // does and kept in the game's options (None: the mapping's default back).
    let remaps = mod_horizon_qol::take_remaps();
    if !remaps.is_empty() {
        for (name, key) in &remaps {
            let key = match key {
                Some(key) => key.clone(),
                None => {
                    if let Some((_, default, _)) =
                        horizon_client::keybinds::all_mappings().find(|m| m.0 == name)
                    {
                        default.to_string()
                    } else {
                        "key.keyboard.unknown".to_owned()
                    }
                }
            };
            game.settings.keys.set(name, &key);
        }
        game.settings.save(&crate::game::Game::options_path());
    }
    // Horizon QoL's Replay module: the recording it asked to start or stop (Horizon's own
    // `/studio record`), and how the one running goes. A replay playing isn't recorded again.
    {
        use mod_horizon_qol::replay as qr;
        let replaying = game.replay.playing();
        // A start waits until the player has joined the world (and isn't reconfiguring), so the
        // recording begins with the world it shows.
        let ready = game.player.is_some()
            && game
                .remote
                .as_ref()
                .is_some_and(|r| r.joined && r.reconfiguring.is_none());
        let request = match qr::take_request() {
            Some(Some(name)) if !ready && !replaying => {
                qr::start(&name);
                None
            }
            other => other,
        };
        if let Some(request) = request {
            let start = request.is_some();
            let result = match &request {
                Some(_) if replaying => Err("a replay is playing".to_owned()),
                Some(name) => game
                    .start_studio_recording(Some(name.as_str()))
                    .map_err(|e| e.to_string()),
                None => game.stop_studio_recording().map_err(|e| e.to_string()),
            };
            match result {
                Ok(v) => {
                    let ok = v["ok"].as_bool() == Some(true);
                    let text = if ok { &v["path"] } else { &v["error"] };
                    qr::push_outcome(start, ok, text.as_str().unwrap_or_default().to_owned());
                }
                Err(e) => qr::push_outcome(start, false, e),
            }
        }
        let s = game.studio_recording_status();
        qr::set_status(
            s["active"].as_bool() == Some(true),
            s["elapsed_ms"].as_u64().unwrap_or(0),
            s["bytes_written"].as_u64().unwrap_or(0),
            s["path"].as_str().unwrap_or_default().to_owned(),
            replaying,
        );
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
