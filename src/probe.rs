//! `actuate --probe` (read-only dump) and `actuate --probe-stream [seconds]` (travel stream test).

use crate::devices;
use crate::protocol::hid::{HidTransport, TravelStream};
use crate::protocol::{fn_system, ColorModes, KeyboardClient};
use anyhow::Result;
use hidapi::HidApi;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

fn tally(values: impl Iterator<Item = String>) -> String {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for v in values {
        *m.entry(v).or_default() += 1;
    }
    m.into_iter()
        .map(|(k, n)| format!("{k}×{n}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn connect(api: &HidApi) -> Result<(KeyboardClient, &'static devices::DeviceDef)> {
    let mut kb = KeyboardClient::new(Box::new(HidTransport::open(api)?));
    let info = kb.identify()?;
    let def = devices::find(info.device_id)
        .ok_or_else(|| anyhow::anyhow!("unknown model id {}", info.device_id))?;
    if let Some(c) = def.color_modes {
        kb.color_modes = ColorModes {
            rainbow: c.rainbow,
            fixed: c.fixed,
        };
    }
    Ok((kb, def))
}

/// Read-only: identify and read the settings the app shows. Sends no SET commands.
pub fn probe() -> Result<()> {
    let api = HidApi::new()?;
    let (mut kb, def) = connect(&api)?;
    let info = kb.info;
    println!(
        "model      {} (id {}), firmware v{}.{:02x}, {} steps/mm",
        def.display_name,
        info.device_id,
        info.firmware >> 8,
        info.firmware & 0xff,
        info.steps_per_mm
    );
    let profile = kb.get_profile()?;
    let opts = kb.get_options()?;
    println!(
        "profile    {}  os {}  active Fn layer {}  debounce {} ms",
        profile + 1,
        opts.os,
        opts.fn_index + 1,
        kb.get_debounce()?
    );
    println!("lighting   {:?}", kb.get_lighting()?);
    let layout = devices::build_layout(&def.default_actions());
    let keys = kb.get_key_settings()?;
    let physical: Vec<_> = layout.keys.iter().map(|k| keys[k.index]).collect();
    println!(
        "actuation  {}",
        tally(physical.iter().map(|k| format!("{:.2}", k.actuation)))
    );
    println!(
        "rapid trig {}",
        tally(physical.iter().map(|k| if k.rapid_trigger {
            format!("on {:.2}/{:.2}", k.rt_press, k.rt_release)
        } else {
            "off".into()
        }))
    );
    println!(
        "dead zone  {}",
        tally(physical.iter().map(|k| format!("{:.2}", k.deadzone)))
    );
    let keymap = kb.get_key_matrix(profile, 0)?;
    let remapped = layout
        .keys
        .iter()
        .filter(|k| keymap[k.index] != k.base)
        .count();
    let fnmap = kb.get_fn_matrix(opts.fn_index, fn_system(opts.os))?;
    let fn_defaults = def.default_fn_actions();
    let fn_changed = layout
        .keys
        .iter()
        .filter(|k| fnmap[k.index] != fn_defaults[k.index])
        .count();
    println!(
        "remaps     {remapped} keys on profile {}, {fn_changed} Fn keys differ from factory",
        profile + 1
    );
    Ok(())
}

/// Turns the travel stream on, listens for `seconds`, then turns it off again.
pub fn probe_stream(seconds: u64) -> Result<()> {
    let api = HidApi::new()?;
    let (mut kb, def) = connect(&api)?;
    let (events, _keep) = async_channel::unbounded();
    let stream = TravelStream::open(&api, events)?;
    kb.set_travel_stream(true)?;
    println!("stream on for {seconds} s — press and wiggle some keys");
    let started = Instant::now();
    let names: Vec<(usize, String)> = devices::build_layout(&def.default_actions())
        .keys
        .iter()
        .map(|k| (k.index, crate::keycodes::action_label(&k.base)))
        .collect();
    while started.elapsed() < Duration::from_secs(seconds) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let result = kb.set_travel_stream(false);
    let count = *stream.count.lock().unwrap();
    let peak = stream.peaks.lock().unwrap().clone();
    drop(stream);
    result?;
    println!(
        "stream off; {count} reports ({:.0}/s)",
        count as f64 / seconds as f64
    );
    for (i, p) in peak.iter().enumerate().filter(|(_, p)| **p > 0) {
        let name = names
            .iter()
            .find(|(k, _)| *k == i)
            .map(|(_, n)| n.as_str())
            .unwrap_or("?");
        println!(
            "  key {i:3} {name:<8} deepest {:.2} mm",
            *p as f32 / kb.info.steps_per_mm
        );
    }
    Ok(())
}
