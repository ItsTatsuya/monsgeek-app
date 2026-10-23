//! App-driven lighting per profile and per Fn layer. The firmware keeps one lighting setting per
//! profile and nothing per Fn layer, so while the app runs it watches for switches and writes the
//! matching look.
//!
//! Precedence: the active Fn layer's look, else the active profile's look, else whatever the
//! profile had before the app changed it (restored, so clearing a look puts the keyboard back).

use crate::protocol::Lighting;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const PROFILE_COUNT: usize = 4;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayerLooks {
    pub enabled: bool,
    pub profiles: Vec<Option<Lighting>>,
    pub fn_layers: Vec<Option<Lighting>>,
}

impl Default for LayerLooks {
    fn default() -> Self {
        Self::empty(2)
    }
}

impl LayerLooks {
    pub fn empty(fn_layers: usize) -> Self {
        Self {
            enabled: false,
            profiles: vec![None; PROFILE_COUNT],
            fn_layers: vec![None; fn_layers],
        }
    }

    /// Pad or trim to the model's profile and Fn-layer counts (config files may be older).
    pub fn fitted(mut self, fn_layers: usize) -> Self {
        self.profiles.resize(PROFILE_COUNT, None);
        self.fn_layers.resize(fn_layers, None);
        self
    }

    pub fn resolve(&self, profile: u8, fn_layer: u8) -> Option<Lighting> {
        if !self.enabled {
            return None;
        }
        self.fn_layers
            .get(fn_layer as usize)
            .copied()
            .flatten()
            .or_else(|| self.profiles.get(profile as usize).copied().flatten())
    }
}

/// Remembers each profile's own lighting while a look overrides it.
#[derive(Default)]
pub struct Engine {
    baseline: HashMap<u8, Lighting>,
}

impl Engine {
    /// What to write for the keyboard's current state, if anything.
    pub fn decide(
        &mut self,
        looks: &LayerLooks,
        profile: u8,
        fn_layer: u8,
        shown: Lighting,
    ) -> Option<Lighting> {
        if let Some(target) = looks.resolve(profile, fn_layer) {
            self.baseline.entry(profile).or_insert(shown);
            return (shown != target).then_some(target);
        }
        let before = self.baseline.remove(&profile)?;
        (shown != before).then_some(before)
    }

    /// Forget overrides (e.g. after a disconnect); the keyboard keeps what it shows now.
    pub fn reset(&mut self) {
        self.baseline.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(effect: u8) -> Lighting {
        Lighting {
            effect,
            speed: 2,
            brightness: 4,
            option: 0,
            rainbow: false,
            color: [1, 2, 3],
        }
    }

    #[test]
    fn fn_layer_wins_and_clearing_restores_the_profile() {
        let mut looks = LayerLooks::empty(2);
        looks.enabled = true;
        looks.profiles[1] = Some(look(2));
        looks.fn_layers[1] = Some(look(5));
        let own = look(1);
        let mut e = Engine::default();
        assert_eq!(e.decide(&looks, 0, 0, own), None);
        assert_eq!(e.decide(&looks, 1, 0, own), Some(look(2)));
        assert_eq!(e.decide(&looks, 1, 1, look(2)), Some(look(5)));
        assert_eq!(e.decide(&looks, 1, 1, look(5)), None);
        looks.enabled = false;
        // The profile's own lighting comes back.
        assert_eq!(e.decide(&looks, 1, 1, look(5)), Some(own));
        assert_eq!(e.decide(&looks, 1, 1, own), None);
        assert_eq!(LayerLooks::default().fitted(3).fn_layers.len(), 3);
    }
}
