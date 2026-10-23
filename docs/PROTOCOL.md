# MonsGeek / Akko (RY5088) HID configuration protocol

Recovered from `MonsGeek_V4_Driver_500.3.4` (Electron app, `resources/app/dist/js`), and
checked with read-only commands against a FUN60 PRO (device id 2304, firmware 0x0309).

## How the official driver works

```
React UI (Electron renderer, nodeIntegration on)
   │  protobuf-ts gRPC-web, http://127.0.0.1:3838   (hid.HidService)
   ▼
iot_manager/iot_manager_rs.exe   (Rust, hidapi bridge, also holds a lock per device)
   │  raw HID feature reports
   ▼
Keyboard, vendor interface: usagePage 0xFFFF, usage 0x02, interface 2
```

- `hid.HidService` methods: `StartDeviceMonitoring`, `Write`, `GetFeatureReport`,
  `SendFeatureReport`, `ListenToReports`, `ListenManyReports`, `LockDevice`, `UnlockDevice`,
  `GetVersion`, `Shutdown`, `KillProcessOnPort`. The bridge only forwards bytes; the whole
  keyboard protocol lives in JavaScript.
- A second service, `iot_manager.IotManager` on port 6015, handles autostart, a key/value DB
  and "features" (music rhythm lighting, screen-colour sync, system-info OLED).
- The bundle also has a **WebHID** transport (`device.sendFeatureReport` /
  `receiveFeatureReport`), so the bridge isn't required. This app uses WebHID only.

## Framing

- Every command is one 64-byte **feature report, report ID 0**.
- Byte 0 is the command. Bytes 1..6 are arguments. The payload (if any) starts at byte 8.
- Checksum:
  - `Bit7` (almost every command): `buf[7] = 255 - (sum(buf[0..6]) & 0xFF)`
  - `Bit8` (only SET_LEDPARAM and SET_SLEDPARAM): `buf[8] = 255 - (sum(buf[0..7]) & 0xFF)`
- To read, send the GET command and then `receiveFeatureReport(0)`. The reply echoes the
  command in byte 0.
- The driver waits about 10 ms before each send and each read, and about 100 ms after a write
  that changes something.
- Bulk transfers split data into 56-byte pages: `[cmd, a, b, page, len, isLast, c, chk, ...56 bytes]`.

## Commands

| Cmd | Name | Request | Reply / notes |
|----:|------|---------|---------------|
| 0x01 | SET_RESET | – | Factory reset (wait about 2 s) |
| 0x03 / 0x83 | SET/GET_REPORT | `[2]=rate idx` | rate idx 0..6 = 8000,4000,2000,1000,500,250,125 Hz (reply byte 2) |
| 0x04 / 0x84 | SET/GET_PROFILE | `[1]=profile 0..3` | reply byte 1 |
| 0x06 / 0x86 | SET/GET_DEBOUNCE | `[1]=ms` | reply byte 1 |
| 0x07 / 0x87 | SET/GET_LEDPARAM | `[1]=effect [2]=4-speed [3]=bright(0-4) [4]=(option<<4)\|colorMode [5..7]=RGB` | colorMode 7 = fixed colour, 8 = rainbow ("dazzle"). **Bit8 checksum** on set |
| 0x08 / 0x88 | SET/GET_SLEDPARAM | as LEDPARAM, side light | |
| 0x09 / 0x89 | SET/GET_KBOPTION | `[2]=fnIndex [3]=anti-mistouch [4]=RT stability/25 [5]=WASD↔arrows` | reply `[1]`=OS (0 win, 1 mac, 2 ios, 3 android) |
| 0x0A / 0x8A | SET/GET_KEYMATRIX | get: `[1]=profile [2]=0xFF [3]=page 0..7 [4]=layer`; set single: `[1]=profile [2]=keyIdx [5]=commit [6]=layer [8..11]=action` | 4 bytes per key |
| 0x0B / 0x8B | SET/GET_MACRO | SET `[1]=2×slot [2]=page [3]=56 [4]=last`; GET `[1]=slot [2]=page` | `u16 repeat`, then `[usage, down<<7\|delay]` (delay 1–127 ms) or `[usage, down<<7, u16 delay]`, ended by 4 zero bytes. 32 slots × 256 B; SET uses 128-byte units on fw 3.09 (odd indices are ignored) and commits to flash in up to ~1 s. Key action `[9, mode, slot, 0]`, mode 0 = N times, 1 = toggle, 2 = while held |
| 0x1C / 0x1E | SET_MAGNETISM_CAL / SET_MAGNETISM_MAXIMUM_CALIBRATION | `[1]=on` | Calibration: 0x1C on, ~2 s hands off, 0x1C off (rest position); then 0x1E on while every key is pressed fully down, 0x1E off saves. During the bottom pass GET_MULTI_MAGNETISM sub 254 returns per-key progress; the driver counts ≥ 300 as done |
| 0x0C / 0x8C | SET/GET_USERPIC | per-key RGB, 7×56-byte pages | |
| 0x10 / 0x90 | SET/GET_FN | `[1]=os [2]=profile [3]=keyIdx\|0xFF` | Fn layer |
| 0x11 / 0x91 | SET/GET_SLEEPTIME | u16 LE at 8,10,12,14 = BT sleep, 2.4G sleep, BT deep, 2.4G deep (s) | |
| 0x17 / 0x97 | SET/GET_AUTOOSEN | `[1]=0/1` | OS auto-detect |
| 0x1B | SET_MAGNETISM_REPORT | `[1]=0/1` | Live key-travel streaming |
| 0x1C / 0x1E | MAGNETISM_CAL min / max | `[1]=0/1` | Calibration start/stop |
| 0x65 / 0xE5 | SET/GET_MULTI_MAGNETISM | see below | Hall-effect per-key settings |
| 0x8F | GET_USB_VERSION | – | `u32 LE [1..4]` = **device id** (identifies the model), `[7..8]` = fw version |
| 0x80 | GET_RF_VERSION | – | `[1..2]` |
| 0xE6 | GET_FEATURE_LIST | – | `[1]==0xAA` means supported. `[2]` = precision enum (0 = 0.01 mm, 1 = 0.005, 2 = 0.001), `[3]` = gamepad |
| 0x0F | light sync stream | `[1]=1 [2]=page [3]=56 [4]=last` | per-key RGB for screen/music sync |

### Hall-effect settings (0xE5 / 0x65)

GET request: `[0xE5, sub, 1, page]`. Sub-tables are indexed by **matrix key index**:

| sub | meaning | encoding |
|----:|---------|----------|
| 0 | actuation point | u16 / step |
| 1 | release point | u16 / step |
| 2 | Rapid Trigger press sensitivity | u16 / step |
| 3 | Rapid Trigger release sensitivity | u16 / step |
| 4 | DKS dynamic travel | u16 / step |
| 5 | Mod-Tap hold time | u8 × 10 ms |
| 6 | bottom dead zone | u16 / step |
| 7 | key mode | u8: low 7 bits 0 normal, 2 DKS, 3 mod-tap, 4 toggle-hold, 5 toggle-tap, 7 snap-tap. **bit 7 = Rapid Trigger on** |
| 8 | DKS trigger modes | 4 bytes per key |
| 9 | Snap-tap partner | u8 matrix index |
| 10 | DKS table | 4 × 128 bytes |
| 251 | top dead zone | u16 / step (fw ≥ 0x0400) |
| 252 | switch type | u8 (0 Gateron, 1 "Jade", 2 Jade Pro, 3 Jade Gaming, 4 TTC King, 5 Magnetic King, …) |
| 254 | live travel (calibration readout) | u16 |

`step` is 100 per mm for firmware 0x0300–0x04FF, 200 for ≥ 0x0500 and 10 for older firmware.
When the feature list is supported, the precision enum decides it instead.

SET whole table: `[0x65, sub, 1, page, isLast, 0,0,chk, 56 bytes]`.
SET one key: `[0x65, sub, 0, keyIdx, isLast, 0,0,chk, value bytes]`.

### Key matrix

The matrix is **column-major, 6 rows per column**: `keyIdx = col*6 + row`, where
row 0 is the F-row and row 5 is the bottom row. Each entry is 4 bytes:

| bytes | meaning |
|-------|---------|
| `[0, mod, hid, hid2]` | keyboard key (`mod` = HID modifier bits) |
| `[1, …]` | mouse button / wheel |
| `[3, 0, usage_lo, usage_hi]` | consumer (media) usage |
| `[9, mode, macroIdx, 0]` | macro (mode 0 repeat, 1 toggle, 2 while held) |
| `[10, …]`, `[13, …]` | firmware functions (Fn = `[0,10,1,0]`, profile, lighting…) |
| `[24, key, key2, time/10]` | mod-tap |
| `[0,0,0,0]` | disabled |

Each model's default matrix (which HID code sits at which index) is in
`assets/monsgeek.json`, extracted from the driver.

### Lighting effects (LEDPARAM byte 1)

0 Off, 1 Static, 2 Breath, 3 Neon, 4 Wave, 5 Ripple, 6 Raindrop, 7 Snake, 8 Reactive,
9 Converge, 10 Sine wave, 11 Kaleidoscope, 12 Line wave, 13 Custom (per-key), 14 Laser,
15 Circle wave, 16 Dazzle, 17 Rain down, 18 Meteor, 19 Reactive off, 20 Music 3, 21 Screen
colour, 22 Music 2, 23 Train, 24 Fireworks, 25 User colour.

## Dynamic keystroke (DKS) and mod-tap

From the driver's `getMultiMagnetismInfo` / `setMagnetismInfoSimple` and its advanced-key pages. Not yet confirmed on hardware.

Both modes store their actions in the key matrix **sub-layers**: byte 4 of `GET_KEYMATRIX` / byte 6 of `SET_KEYMATRIX` (`[0x0A, profile, key, 0, 0, commit, sublayer, chk, action…]`). The FUN60 PRO has 4 sub-layers per profile (`layer: 4`); sub-layer 0 is the normal remap.

**DKS** (MODE table value 2):

| Data | Where | Format |
|---|---|---|
| 4 actions | key matrix sub-layers 0–3 | normal 4-byte actions |
| Trigger point | Mag sub 4 | u16 steps per key; driver default 0.70 mm, range 0.5–2.5 mm |
| Phases | written per key with Mag sub 8 (`[0x65, 8, 0, key, commit, 0,0,chk, b0, b1, b2, b3]`), read from Mag sub 10 (8 pages = 4 × 128 bytes, action g of key i at `g*128+i`) | one byte per action, 2 bits per phase: `m3<<6 \| m2<<4 \| m1<<2 \| m0` |

Phases: 0 = press past the trigger point, 1 = bottom out, 2 = start of release from the bottom, 3 = release back past the trigger point. Values: 0 = nothing, 1 = tap at this phase, 2 = hold from here until the next phase, 3 = hold through the next phase (a run of 3s ends in a 2: `[3,3,2,0]` holds from phase 0 to phase 3, i.e. a normal key press).

**Mod-tap** (MODE value 3): hold action in sub-layer 0, tap action in sub-layer 1, hold time in Mag sub 5 as one byte = ms / 10 (driver range 10–1000 ms, default 200). The driver's `ConfigMT` key action `[24, key, key2, time/10]` belongs to other models and is not used by the FUN60 PRO.

Removing either mode: the driver writes the key's own action back to sub-layer 0 and clears sub-layers 1–3 (writing 0), then sets MODE to 0.
