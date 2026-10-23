//! Predicts what the keyboard reports for one key from its travel samples, so the switch tester
//! can show where Rapid Trigger fires and resets. The firmware's key state can't be read, so
//! this is a model.
//!
//! MonsGeek's Rapid Trigger is "continuous": the first press registers at the actuation point;
//! while pressed, the key releases once it rises `rt_release` above the deepest point since it
//! pressed; while released (still in Rapid Trigger), it presses again once it goes `rt_press`
//! below the highest point since it released. Rapid Trigger ends only when the key comes all the
//! way up. Without Rapid Trigger the key presses at the actuation point and releases above the
//! release point. The bottom dead zone is modelled as clamping travel to (full travel − dead
//! zone); how the firmware applies it exactly is not documented.

use crate::protocol::KeyHe;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RtConfig {
    pub actuation: f32,
    pub release: f32,
    pub rapid_trigger: bool,
    pub rt_press: f32,
    pub rt_release: f32,
    pub deadzone: f32,
    /// Bottom of the key's travel, mm.
    pub full_travel: f32,
}

impl RtConfig {
    pub fn from_key(k: &KeyHe, full_travel: f32) -> Self {
        Self {
            actuation: k.actuation,
            release: k.release,
            rapid_trigger: k.rapid_trigger,
            rt_press: k.rt_press,
            rt_release: k.rt_release,
            deadzone: k.deadzone,
            full_travel,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RtState {
    pub on: bool,
    /// Inside a continuous Rapid Trigger run (between the first actuation and a full release).
    pub in_rt: bool,
    /// Deepest point while pressed, or highest point while released inside Rapid Trigger, mm.
    pub extreme: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RtEvent {
    Press,
    Release,
}

/// Travel at or above this counts as fully released (the sensor rarely reads exactly 0).
pub const TOP_EPS: f32 = 0.05;

pub fn step(s: RtState, travel: f32, c: &RtConfig) -> (RtState, Option<RtEvent>) {
    let t = travel.min(c.actuation.max(c.full_travel - c.deadzone));
    if t <= TOP_EPS {
        return (RtState::default(), s.on.then_some(RtEvent::Release));
    }
    let state = |on, in_rt, extreme| RtState { on, in_rt, extreme };
    if !c.rapid_trigger {
        if !s.on && t >= c.actuation {
            return (state(true, false, t), Some(RtEvent::Press));
        }
        if s.on && t < c.release.min(c.actuation) {
            return (state(false, false, t), Some(RtEvent::Release));
        }
        return (s, None);
    }
    if !s.in_rt {
        if t >= c.actuation {
            return (state(true, true, t), Some(RtEvent::Press));
        }
        return (s, None);
    }
    if s.on {
        let deepest = s.extreme.max(t);
        if t <= deepest - c.rt_release {
            return (state(false, true, t), Some(RtEvent::Release));
        }
        return (RtState { extreme: deepest, ..s }, None);
    }
    let highest = s.extreme.min(t);
    if t >= highest + c.rt_press {
        return (state(true, true, t), Some(RtEvent::Press));
    }
    (RtState { extreme: highest, ..s }, None)
}

/// Where the next transition happens from this state, for the graph overlay.
pub fn next_threshold(s: &RtState, c: &RtConfig) -> (RtEvent, f32) {
    if !c.rapid_trigger {
        return if s.on {
            (RtEvent::Release, c.release.min(c.actuation))
        } else {
            (RtEvent::Press, c.actuation)
        };
    }
    if !s.in_rt {
        return (RtEvent::Press, c.actuation);
    }
    if s.on {
        (RtEvent::Release, TOP_EPS.max(s.extreme - c.rt_release))
    } else {
        (RtEvent::Press, s.extreme + c.rt_press)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(samples: &[f32], c: &RtConfig) -> Vec<(usize, RtEvent)> {
        let mut s = RtState::default();
        let mut out = Vec::new();
        for (i, &v) in samples.iter().enumerate() {
            let (next, e) = step(s, v, c);
            s = next;
            if let Some(e) = e {
                out.push((i, e));
            }
        }
        out
    }

    const RT: RtConfig = RtConfig {
        actuation: 1.0,
        release: 1.0,
        rapid_trigger: true,
        rt_press: 0.3,
        rt_release: 0.3,
        deadzone: 0.0,
        full_travel: 3.4,
    };

    #[test]
    fn rapid_trigger_fires_on_small_movements_until_fully_released() {
        let e = run(&[0.5, 1.2, 2.0, 1.8, 1.6, 1.95, 2.5, 0.8, 0.0], &RT);
        use RtEvent::*;
        assert_eq!(e, vec![(1, Press), (4, Release), (5, Press), (7, Release)]);
        assert_eq!(next_threshold(&RtState::default(), &RT), (Press, 1.0));
    }

    #[test]
    fn without_rapid_trigger_uses_actuation_and_release_points() {
        let c = RtConfig {
            rapid_trigger: false,
            release: 0.8,
            ..RT
        };
        use RtEvent::*;
        assert_eq!(run(&[0.9, 1.1, 2.0, 0.9, 0.7, 0.0], &c), vec![(1, Press), (4, Release)]);
    }

    #[test]
    fn dead_zone_keeps_bottom_wobble_from_releasing() {
        let c = RtConfig {
            deadzone: 0.5,
            ..RT
        };
        // Travel clamps at 2.9, so wobble between 3.4 and 3.0 is ignored.
        assert_eq!(run(&[1.2, 3.4, 3.0, 3.4], &c).len(), 1);
    }
}
