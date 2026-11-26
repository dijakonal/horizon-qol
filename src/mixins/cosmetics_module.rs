//! Horizon QoL's Cosmetics module, game side (`mod_horizon_qol::cosmetics`): what its
//! player wears, parsed with the game's own item reader and kept while the worn set is the
//! same. The mod's mixin on `Seat::player_layers` puts it on the player's own model; every
//! other player wears what the service answered, as the game asked it.

use horizon_cosmetics::{Item, Worn};
use std::sync::{Arc, Mutex, PoisonError};

/// What the mod's player wears, parsed and kept while the worn set is the same: the items
/// in the mod's own order. `None` with nothing on.
pub(in crate::game) fn worn() -> Option<Worn> {
    static CACHE: Mutex<Option<(Vec<&'static str>, Option<Worn>)>> = Mutex::new(None);
    let wanted = mod_horizon_qol::cosmetics::worn();
    let ids: Vec<&'static str> = wanted.iter().map(|(_, c)| c.id).collect();
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    if cache
        .as_ref()
        .is_some_and(|(kept, _)| *kept == ids)
    {
        return cache.as_ref().and_then(|(_, worn)| worn.clone());
    }
    // An item that won't parse is left out; it is tried again the next time the set changes.
    let items: Vec<Arc<Item>> = wanted
        .iter()
        .filter_map(|(_, c)| Item::parse(c.json.as_bytes()).ok())
        .map(Arc::new)
        .collect();
    let worn = (!items.is_empty()).then(|| Arc::from(items.into_boxed_slice()));
    *cache = Some((ids, worn.clone()));
    worn
}
