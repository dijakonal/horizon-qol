use crate::voice::Protocol;
use crate::voice::audio::AudioIo;
use crate::voice::net::Conn;
use crate::voice::proto::{COMPAT_26, COMPAT_27, Group, PlayerState, Secret};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// One server visit.
pub(crate) struct Session {
    pub(crate) address: String,
    pub(crate) joined: Instant,
    /// The versions asked so far, and when the next is due.
    pub(crate) asked: Vec<i32>,
    pub(crate) next_ask: Instant,
    /// The version that answered here before.
    pub(crate) remembered: Option<i32>,
    pub(crate) secret: Option<Secret>,
    pub(crate) conn: Option<Conn>,
    pub(crate) audio: Option<AudioIo>,
    pub(crate) states: HashMap<u128, PlayerState>,
    names: HashMap<String, u128>,
    pub(crate) groups: HashMap<u128, Group>,
    pub(crate) group: Option<u128>,
}

impl Session {
    pub(crate) fn new(address: &str, remembered: Option<i32>) -> Self {
        let now = Instant::now();
        Self {
            address: address.to_owned(),
            joined: now,
            asked: Vec::new(),
            // The server knows the player a moment after the join.
            next_ask: now + Duration::from_secs(1),
            remembered,
            secret: None,
            conn: None,
            audio: None,
            states: HashMap::new(),
            names: HashMap::new(),
            groups: HashMap::new(),
            group: None,
        }
    }

    pub(crate) fn set_states(&mut self, states: Vec<PlayerState>) {
        self.states = states.into_iter().map(|s| (s.uuid, s)).collect();
        self.names = self
            .states
            .values()
            .map(|s| (s.name.clone(), s.uuid))
            .collect();
    }

    pub(crate) fn insert_state(&mut self, state: PlayerState) {
        let id = state.uuid;
        let name = state.name.clone();
        if let Some(old) = self.states.insert(id, state) {
            self.names.remove(&old.name);
        }
        self.names.insert(name, id);
    }

    pub(crate) fn remove_state(&mut self, id: u128) {
        if let Some(old) = self.states.remove(&id) {
            self.names.remove(&old.name);
        }
    }

    pub(crate) fn player_for_name_tag(&self, text: &str) -> Option<(u128, &PlayerState)> {
        text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|token| !token.is_empty())
            .filter_map(|token| {
                let id = *self.names.get(token)?;
                Some((id, self.states.get(&id)?))
            })
            .max_by_key(|(_, player)| player.name.len())
    }

    pub(crate) fn memberships(&self) -> HashMap<u128, Vec<String>> {
        let mut members: HashMap<u128, Vec<String>> = HashMap::new();
        for player in self.states.values().filter(|p| !p.disconnected) {
            if let Some(group) = player.group {
                members.entry(group).or_default().push(player.name.clone());
            }
        }
        for names in members.values_mut() {
            names.sort_by_cached_key(|n| n.to_lowercase());
        }
        members
    }

    pub(crate) fn compat(&self) -> i32 {
        self.secret.as_ref().map_or(COMPAT_26, |s| s.compat)
    }

    /// The versions to ask, in order: the one that answered here before, then
    /// the releases', then 2.7's, twice over for a server still starting.
    pub(crate) fn plan(&self, protocol: Protocol) -> Vec<i32> {
        match protocol {
            Protocol::V26 => vec![COMPAT_26, COMPAT_26, COMPAT_26],
            Protocol::V27 => vec![COMPAT_27, COMPAT_27, COMPAT_27],
            Protocol::Auto => match self.remembered {
                Some(COMPAT_27) => vec![COMPAT_27, COMPAT_26, COMPAT_27, COMPAT_26],
                _ => vec![COMPAT_26, COMPAT_27, COMPAT_26, COMPAT_27],
            },
        }
    }
}
