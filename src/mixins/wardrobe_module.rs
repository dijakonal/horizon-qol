//! Horizon QoL's skin manager, game side (`mod_horizon_qol::wardrobe`): the skin and cape the
//! player picked, drawn on their own player, and the account's skin and capes, read and changed
//! through Mojang's profile API with the game's Microsoft sign-in, on a thread of its own.

use crate::game::Game;
use mod_horizon_qol::wardrobe as w;
use std::io::Read as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";

/// The game's PNG decoder, lent to the mod for its previews.
fn decode(png: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    horizon_client::hud::decode_bytes(png).ok()
}

/// Each client tick: the decoder for the mod, and the jobs it queued, done on a thread.
pub(in crate::game) fn tick() {
    w::set_decoder(decode);
    static BUSY: AtomicBool = AtomicBool::new(false);
    if BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    let Some(first) = w::take_job() else {
        BUSY.store(false, Ordering::Release);
        return;
    };
    let spawned = std::thread::Builder::new()
        .name("horizon-qol-skins".into())
        .spawn(move || {
            let mut job = Some(first);
            while let Some(j) = job {
                run(j);
                job = w::take_job();
            }
            BUSY.store(false, Ordering::Release);
        });
    if spawned.is_err() {
        BUSY.store(false, Ordering::Release);
    }
}

/// Before the frame's skins settle (`update_remote_skins`): the picked skin and cape on the
/// player's own model, once their pictures are in the atlas.
pub(in crate::game) fn apply(game: &mut Game<'_>) {
    let look = w::look();
    if let Some(cape) = &look.cape {
        // `capes/`: a cape takes the top half of its atlas slot (`store_skin`).
        let location = format!("capes/horizon_qol_{}", cape.key);
        if !game.skin_slots.contains(&location)
            || !game.gui.atlas.sprites.contains_key(&location)
        {
            if let Some((width, height, rgba)) = decode(&cape.png)
                && let Some(rgba) = horizon_client::skins::process_cape(width, height, &rgba)
            {
                game.store_skin(location.clone(), rgba);
            }
        }
        let shown = game.settings.file.bool("modelPart_cape");
        if game.again_skin(&location) && shown {
            if let Some(model) = game.player_model.as_mut() {
                model.cape = Some(location);
            }
        }
    }
    if let Some(skin) = &look.skin {
        let location = format!("horizon_qol/skin_{}", skin.key);
        if !game.skin_slots.contains(&location)
            || !game.gui.atlas.sprites.contains_key(&location)
        {
            if let Some((width, height, rgba)) = decode(&skin.png)
                && let Some(rgba) = horizon_client::skins::process_legacy_skin(width, height, &rgba)
            {
                game.store_skin(location.clone(), rgba);
            }
        }
        if game.again_skin(&location) {
            let kind = game.player_model.as_ref().map(|m| m.kind.clone());
            let skin_3d = kind
                .as_ref()
                .and_then(|kind| game.skin_3d_model(kind, &location, look.slim));
            if let Some(model) = game.player_model.as_mut() {
                model.skin = Some(location);
                model.slim = look.slim;
                model.skin_3d = skin_3d;
            }
        }
    }
}

/// One job, on the skins thread.
fn run(job: w::Job) {
    let Some(session) = horizon_client::auth::current() else {
        w::set_account_state(w::AccountState::SignedOut);
        if job != w::Job::Refresh {
            w::note(
                false,
                "Sign in with a Microsoft account to change your account's skin or cape.".into(),
            );
        }
        return;
    };
    let bearer = format!("Bearer {}", session.access_token());
    match job {
        w::Job::Refresh => {
            w::set_account_state(w::AccountState::Loading);
            if let Err(e) = refresh(&bearer) {
                w::set_account_state(w::AccountState::Failed(e));
            }
        }
        w::Job::UploadSkin { png, slim } => match upload(&bearer, &png, slim) {
            Ok(()) => {
                w::note(
                    true,
                    "Your account's skin is changed. Other players see it once you rejoin.".into(),
                );
                let _ = refresh(&bearer);
            }
            Err(e) => w::note(false, format!("Couldn't change your skin: {e}")),
        },
        w::Job::Cape(id) => match set_cape(&bearer, id.as_deref()) {
            Ok(()) => {
                let said = if id.is_some() {
                    "Your account's cape is changed. Other players see it once you rejoin."
                } else {
                    "Your account's cape is hidden."
                };
                w::note(true, said.into());
                let _ = refresh(&bearer);
            }
            Err(e) => w::note(false, format!("Couldn't change your cape: {e}")),
        },
    }
}

/// A failed request as the player reads it: Mojang's own message when it sent one.
fn error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, response) => {
            let text = response.into_string().unwrap_or_default();
            let said = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| {
                    v["errorMessage"]
                        .as_str()
                        .or_else(|| v["error"].as_str())
                        .map(str::to_owned)
                });
            match said {
                Some(said) => format!("Mojang said {code}: {said}"),
                None => format!("Mojang said {code}"),
            }
        }
        other => other.to_string(),
    }
}

/// A texture from textures.minecraft.net, at most 1 MiB.
fn download(url: &str) -> Option<Vec<u8>> {
    let url = url.replacen("http://", "https://", 1);
    let response = ureq::get(&url).call().ok()?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(1 << 20)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

/// Reads the account (`GET /minecraft/profile`): its name, active skin and capes.
fn refresh(bearer: &str) -> Result<(), String> {
    let body = ureq::get(PROFILE)
        .set("Authorization", bearer)
        .call()
        .map_err(error)?
        .into_string()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let name = v["name"].as_str().unwrap_or_default().to_owned();
    let skin = v["skins"]
        .as_array()
        .and_then(|skins| skins.iter().find(|s| s["state"] == "ACTIVE"));
    let slim = skin.is_some_and(|s| {
        s["variant"]
            .as_str()
            .is_some_and(|v| v.eq_ignore_ascii_case("slim"))
    });
    let skin = skin.and_then(|s| s["url"].as_str()).and_then(download);
    let capes = v["capes"]
        .as_array()
        .map(|capes| {
            capes
                .iter()
                .filter_map(|c| {
                    Some(w::OfficialCape {
                        id: c["id"].as_str()?.to_owned(),
                        name: c["alias"].as_str().unwrap_or("Cape").to_owned(),
                        png: c["url"].as_str().and_then(download).map(Arc::new),
                        active: c["state"] == "ACTIVE",
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    w::set_account(w::Account {
        state: w::AccountState::Ready,
        name,
        capes,
        skin: skin.map(Arc::new),
        slim,
    });
    Ok(())
}

/// Makes `png` the account's skin (`POST /minecraft/profile/skins`, a multipart form).
fn upload(bearer: &str, png: &[u8], slim: bool) -> Result<(), String> {
    const BOUNDARY: &str = "----HorizonQolSkinBoundary7d1f2c";
    let variant = if slim { "slim" } else { "classic" };
    let mut body = Vec::with_capacity(png.len() + 512);
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{variant}\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; \
             filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(png);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    ureq::post(&format!("{PROFILE}/skins"))
        .set("Authorization", bearer)
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .send_bytes(&body)
        .map_err(error)?;
    Ok(())
}

/// Wears the account's cape `id` (`PUT .../capes/active`), or none (`DELETE`).
fn set_cape(bearer: &str, id: Option<&str>) -> Result<(), String> {
    let url = format!("{PROFILE}/capes/active");
    match id {
        Some(id) => ureq::put(&url)
            .set("Authorization", bearer)
            .set("Content-Type", "application/json")
            .send_string(&serde_json::json!({ "capeId": id }).to_string()),
        None => ureq::delete(&url).set("Authorization", bearer).call(),
    }
    .map_err(error)?;
    Ok(())
}
