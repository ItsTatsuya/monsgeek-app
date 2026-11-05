# Monsgeek App

A native Windows configurator for MonsGeek / Akko Hall-effect keyboards (RY5088 platform:
FUN60, FUN68, M1/M2/M3 V5 HE, …). Built with Rust, [GPUI Kit](https://crates.io/crates/gpui-kit) **0.9.0** (GPUI 0.3.11 and
GPUI Component) and hidapi **2.6.7**, it
talks to the keyboard directly over USB HID; no Electron runtime, browser, or vendor bridge
(`iot_manager_rs.exe`) is needed. The bridge is still bundled, for the MonsGeek web driver
(see [IOT bridge](#iot-bridge)).

This is an unofficial, community-made tool and is not affiliated with MonsGeek or Akko.

## Download

Grab `monsgeek-app.exe` from the [latest release](https://github.com/ItsTatsuya/monsgeek-app/releases/latest).
It is a single portable executable: model definitions, fonts, icons and the
official IOT bridge are embedded. Close the
official driver before connecting, so the two apps don't send commands at the same time.

```powershell
monsgeek-app.exe            # connection screen; plug in over USB and click Connect keyboard
monsgeek-app.exe --demo     # a simulated FUN60 PRO, to try the UI without a keyboard
```

Changes write to the keyboard immediately; moving a slider batches the writes until you
release it or pause. Demo mode uses an in-memory keyboard and never opens a HID device.

## Features

- **Performance:** click/drag multi-selection, WASD/letters/all groups, actuation and
  release points, Rapid Trigger with linked or separate press/release values, bottom dead
  zone, and presets. Click any value box to type an exact number (Enter or click away applies
  it, Esc cancels); it's clamped to the range and rounded to the slider's step.
- **Remap:** base layer, numbered Fn layers, factory reset per key, keyboard/media/firmware
  function palette with search, and a readable list of Fn shortcuts.
- **Macros:** record from the keyboard with or without real timing, type text as keystrokes,
  add taps with held modifiers, edit each delay, and play on press / while held / as a toggle.
- **Advanced keys:** Snap Tap pairs, toggle on tap/hold, dynamic keystroke (four actions ×
  four phases, trigger point) and mod-tap (tap/hold actions, hold time).
- **Lighting:** 20 effects with brightness, speed, direction, rainbow or single colour, and
  an animated preview that lights whole keys the way the keyboard does (reactive effects
  respond to your typing). Pick any colour: presets, or a colour picker with a palette,
  HSLA sliders and a hex box. Per-key colours paint the model's user pictures with the same
  picker as the brush.
- **Lighting per profile and Fn layer:** when the keyboard switches profile or layer, the app
  writes the matching look (while the app runs).
- **Tester:** live analog travel at 60 fps with per-key peaks, a Rapid Trigger prediction and
  travel graph, an event log and a tuner; a Raw Input polling-rate test with a gap histogram;
  and calibration (rest position, then a bottom pass).
- **Automatic profiles:** map an app's exe name to a profile. Closing the window can hide the
  app to the tray so switching keeps working.
- **Settings:** polling rate (firmware 4.00+), switch type, debounce, RT stability, WASD/arrow
  swap, accidental-press guard, device info, factory reset, optional background blur, and
  whole-keyboard backup & restore to JSON.
- **IOT bridge:** start or stop the bundled official `iot_manager` from Settings.
- **4 onboard profiles**, switched from the header; changes made on the keyboard itself are
  read back from its notifications.

Settings, rules and looks are saved to `%APPDATA%/Monsgeek App/config.json`.

## Performance

Measured on the same machine against the earlier Electron 33 version of this app (now
removed), both in demo mode, window off-screen, release builds, averaged over two runs.
Memory is summed over all of the app's processes.

| | Native (this app) | Electron |
|---|---|---|
| Processes | 1 | 4 |
| Working set, idle | ~70 MB | ~330 MB |
| Private memory, idle | ~102 MB | ~274 MB |
| Working set, animated lighting preview | ~72 MB | ~391 MB |
| CPU, idle (one core) | ~2–4 % | ~0–0.5 % |
| CPU, animated lighting preview (one core) | ~17 % | ~85 % |
| Startup, warm (to window / UI ready) | ~0.5 s | ~0.6 s |
| Startup, cold first launch | ~0.7 s | ~3.7 s |
| Size on disk | 15 MB exe (now 40 MB: GPUI Kit components and the bundled IOT bridge) | 268 MB unpacked (78.5 MB installer) |

## Build

Requires Rust for Windows MSVC and the Visual Studio C++ build tools / Windows SDK.

```powershell
cargo build --release --locked        # target/release/monsgeek-app.exe
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Launch options:

```powershell
# Open a demo page directly (performance, remap, advanced, lighting, tester, auto, settings):
target/release/monsgeek-app.exe --demo --page settings
# --shot opens the window off-screen without focus, for scripted screenshots.

# Hardware diagnostics; use the debug/console build to see terminal output:
cargo run -- --probe              # identify the keyboard, read settings, no SET commands
cargo run -- --probe-stream 20    # enable travel reporting for 20 s, then disable it
```

Screenshots from demo checks are in `shots/`.

## IOT bridge

The exe embeds the official driver's `iot_manager` 0.1.8 (`iot_manager_rs.exe` and its helper
services, from `MonsGeek_V4_Driver_500.3.4`; see [vendor/iot_manager](vendor/iot_manager/README.md)).
This app doesn't use it. It's there for the MonsGeek web driver, which needs it running locally.
Settings › IOT bridge › **Start** unpacks it to `%APPDATA%/Monsgeek App/iot_manager` and starts
it hidden. It listens on 127.0.0.1:6015 and starts `common_hid_rs.exe` on 127.0.0.1:3838.
**Stop** ends both, including a copy that the official driver started. The bridge keeps running
after the app closes. Don't change settings from the web driver and this app at the same time.

## Known limits

- Changes another app makes over USB aren't announced by the keyboard; reconnect to read them.
- The tester requires the vendor notify collection; it doesn't fall back to slow travel polling.
- Lighting previews approximate the firmware animations, which aren't documented.

## How the protocol was recovered

The official installer is an NSIS package around an Electron app. The keyboard logic is in its
minified React bundle, so the command table, packet layouts, checksum rules and per-model key
matrices were read from there, then checked against a real FUN60 PRO: reads, no-op writes, and
a reversible actuation change. See [docs/PROTOCOL.md](docs/PROTOCOL.md).
