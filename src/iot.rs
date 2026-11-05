//! The official IOT bridge (`iot_manager_rs.exe` 0.1.8 and its helper services), taken from
//! `MonsGeek_V4_Driver_500.3.4` and embedded in this exe (see `vendor/iot_manager`).
//!
//! This app doesn't need it: it talks to the keyboard over HID itself. The bridge is bundled
//! for things that do, such as the MonsGeek web driver, which connects to it on
//! 127.0.0.1:3838 (`hid.HidService`, served by `common_hid_rs.exe`) and 127.0.0.1:6015
//! (`iot_manager.IotManager`). Like the official app, it's unpacked to the user's app-data
//! folder and started hidden, with no arguments.

use anyhow::{anyhow, bail, Context as _, Result};
use std::net::{SocketAddr, TcpStream};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

include!(concat!(env!("OUT_DIR"), "/iot/iot_files.rs"));

/// Version of the bundled `iot_manager_rs.exe`.
pub const VERSION: &str = "0.1.8";
const EXE: &str = "iot_manager_rs.exe";
/// `iot_manager.IotManager` and `hid.HidService`.
const PORTS: [u16; 2] = [6015, 3838];
const STAMP: &str = ".monsgeek-app-bundle";

const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `%APPDATA%\Monsgeek App\iot_manager`.
pub fn install_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Monsgeek App").join("iot_manager"))
}

/// Identifies the embedded file set, so an older unpacked copy is replaced.
fn bundle_id() -> String {
    let mut s = format!("iot_manager {VERSION}\n");
    for (name, size, packed) in FILES {
        s.push_str(&format!("{name} {size} {}\n", packed.len()));
    }
    s
}

/// True while either bridge port accepts connections (this copy or the official driver's).
pub fn running() -> bool {
    PORTS.iter().any(|&p| {
        let addr = SocketAddr::from(([127, 0, 0, 1], p));
        TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
    })
}

/// Unpacks the embedded files into [`install_dir`] unless the same set is already there.
/// Returns the folder.
pub fn install() -> Result<PathBuf> {
    let dir = install_dir().ok_or_else(|| anyhow!("APPDATA isn't set"))?;
    let id = bundle_id();
    let current = std::fs::read_to_string(dir.join(STAMP)).is_ok_and(|s| s == id)
        && FILES.iter().all(|(name, size, _)| {
            std::fs::metadata(dir.join(name)).is_ok_and(|m| m.len() == *size as u64)
        });
    if current {
        return Ok(dir);
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let _ = std::fs::remove_file(dir.join(STAMP));
    for (name, size, packed) in FILES {
        let data = miniz_oxide::inflate::decompress_to_vec_with_limit(packed, *size)
            .map_err(|e| anyhow!("unpacking {name}: {e:?}"))?;
        let tmp = dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, &data).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, dir.join(name)).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            anyhow!("replacing {name}: {e} (is the bridge running?)")
        })?;
    }
    std::fs::write(dir.join(STAMP), id)?;
    Ok(dir)
}

/// Unpacks if needed, starts the bridge and waits up to 8 s for its ports.
pub fn start() -> Result<()> {
    if running() {
        return Ok(());
    }
    let dir = install()?;
    Command::new(dir.join(EXE))
        .current_dir(&dir)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .with_context(|| format!("starting {EXE}"))?;
    let until = Instant::now() + Duration::from_secs(8);
    while Instant::now() < until {
        if running() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    bail!("{EXE} started but isn't listening on 127.0.0.1:{}", PORTS[0])
}

/// Stops the bridge and the services it started. This also stops the official driver's copy
/// if that one is running.
pub fn stop() -> Result<()> {
    for exe in [EXE, "common_hid_rs.exe"] {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/IM", exe])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    let until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until {
        if !running() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    bail!("the bridge is still running")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_files_unpack() {
        assert!(FILES.iter().any(|f| f.0 == EXE));
        for (name, size, packed) in FILES {
            let data = miniz_oxide::inflate::decompress_to_vec_with_limit(packed, *size).unwrap();
            assert_eq!(data.len(), *size, "{name}");
            assert_eq!(&data[..2], b"MZ", "{name}");
        }
    }
}
