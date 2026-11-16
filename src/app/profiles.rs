//! Onboard profile management: names, copying one profile onto another, and exporting or
//! importing a single profile as a file.

use super::{reload, AppView, OnFail};
use crate::backup::{self, DeviceStamp, ProfileFile};
use anyhow::anyhow;
use gpui::Context;

/// The four onboard profiles.
pub const PROFILES: u8 = 4;

impl AppView {
    /// The name shown for a profile of the connected keyboard.
    pub fn profile_name(&self, profile: u8) -> String {
        let id = self.device.as_ref().map_or(0, |d| d.info.device_id);
        self.config.profile_name(id, profile)
    }

    /// Renames a profile; an empty name goes back to "Profile N". The config is written once
    /// the name stops changing for a moment.
    pub fn rename_profile(&mut self, profile: u8, name: &str, cx: &mut Context<Self>) {
        let Some(id) = self.device.as_ref().map(|d| d.info.device_id) else {
            return;
        };
        let names = self.config.profile_names.entry(id).or_default();
        // Long enough for anything sensible; the header shortens it further.
        let name: String = name.trim().chars().take(32).collect();
        if names[profile as usize] == name {
            return;
        }
        names[profile as usize] = name;
        if names.iter().all(String::is_empty) {
            self.config.profile_names.remove(&id);
        }
        self.tools.name_gen += 1;
        let gen = self.tools.name_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
            this.update(cx, |s, cx| {
                if s.tools.name_gen == gen {
                    s.save_config(cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn stamp(&self) -> Option<DeviceStamp> {
        self.device.as_ref().map(|d| DeviceStamp {
            id: d.info.device_id,
            name: d.def.display_name.clone(),
            firmware: d.info.firmware,
        })
    }

    /// Sends pending edits and blocks other edits until a bulk profile job finishes.
    fn begin_profile_job(&mut self, cx: &mut Context<Self>) -> bool {
        if self.transitioning || self.device.is_none() {
            return false;
        }
        self.end_drag(cx);
        self.commit_edit(cx);
        self.flush_keys(cx);
        self.flush_lighting(cx);
        true
    }

    /// Copies profile `from` onto profile `to` (keymap, key settings and lighting).
    pub fn copy_profile(&mut self, from: u8, to: u8, cx: &mut Context<Self>) {
        if from == to || !self.begin_profile_job(cx) {
            return;
        }
        let (from_name, to_name) = (self.profile_name(from), self.profile_name(to));
        self.transitioning = true;
        self.history.clear();
        self.selection.clear();
        self.features.adv = None;
        self.features.macro_draft = None;
        self.run_or(
            cx,
            "Copying profile",
            // A failure part-way may leave another profile active.
            OnFail::Reconnect,
            move |kb| {
                backup::copy_profile(kb, from, to)?;
                reload(kb)
            },
            move |s, fresh, cx| {
                s.transitioning = false;
                if let Some(d) = s.device.as_mut() {
                    fresh.apply(d);
                }
                s.toast(format!("Copied {from_name} to {to_name}"), false, cx);
            },
            |_, _| {},
        );
    }

    /// Reads a profile and asks where to save it.
    pub fn export_profile(&mut self, profile: u8, cx: &mut Context<Self>) {
        let Some(stamp) = self.stamp() else { return };
        if !self.begin_profile_job(cx) {
            return;
        }
        let name = self.profile_name(profile);
        let dir = documents_dir();
        self.run(
            cx,
            "Exporting profile",
            move |kb| Ok((backup::read_profile(kb, profile)?, backup::read_profile_led(kb, profile)?)),
            move |_, (p, led), cx| {
                let file = backup::profile_file(stamp, &name, p, led);
                let rx = cx.prompt_for_new_path(&dir, Some(&backup::profile_file_name(&file)));
                cx.spawn(async move |this, cx| {
                    let Some(path) = rx.await.ok().and_then(|p| p.ok()).flatten() else {
                        return;
                    };
                    let written = serde_json::to_string_pretty(&file)
                        .map_err(|e| anyhow!(e))
                        .and_then(|json| std::fs::write(&path, json).map_err(|e| anyhow!(e)));
                    this.update(cx, |s, cx| match written {
                        Ok(()) => s.toast(format!("Exported {}", file.name), false, cx),
                        Err(e) => s.toast(format!("Couldn't save the profile: {e}"), true, cx),
                    })
                    .ok();
                })
                .detach();
            },
        );
    }

    /// Asks for a profile file; once it's read, the page asks before replacing `profile`.
    pub fn pick_profile_file(&mut self, profile: u8, cx: &mut Context<Self>) {
        if self.transitioning {
            return;
        }
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Some(path) = rx.await.ok().and_then(|r| r.ok()).flatten().and_then(|p| p.into_iter().next())
            else {
                return;
            };
            let parsed = std::fs::read_to_string(&path)
                .map_err(|e| anyhow!("Can't read the file: {e}"))
                .and_then(|t| backup::parse_profile(&t));
            this.update(cx, |s, cx| {
                match parsed {
                    Ok(f) if s.device.as_ref().is_some_and(|d| d.info.device_id != f.device.id) => s.toast(
                        format!("This profile is for a {}, not this keyboard.", f.device.name),
                        true,
                        cx,
                    ),
                    Ok(f) => s.features.pending_import = Some((profile, f)),
                    Err(e) => s.toast(format!("{e:#}"), true, cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Writes an imported profile into `profile` (after the user confirmed).
    pub fn import_profile(&mut self, profile: u8, file: ProfileFile, cx: &mut Context<Self>) {
        if !self.begin_profile_job(cx) {
            return;
        }
        let target = self.profile_name(profile);
        self.transitioning = true;
        self.history.clear();
        self.selection.clear();
        self.features.adv = None;
        self.features.macro_draft = None;
        let name = file.name.clone();
        self.run_or(
            cx,
            "Importing profile",
            OnFail::Reconnect,
            move |kb| {
                let current = backup::read_profile(kb, profile)?;
                backup::write_profile(kb, &current, &file.profile, profile)?;
                if let Some(led) = &file.led {
                    backup::write_profile_led(kb, profile, led)?;
                }
                reload(kb)
            },
            move |s, fresh, cx| {
                s.transitioning = false;
                if let Some(d) = s.device.as_mut() {
                    fresh.apply(d);
                }
                s.toast(format!("Imported {name} into {target}"), false, cx);
            },
            |_, _| {},
        );
    }
}

/// The user's Documents folder, where save dialogs start.
pub fn documents_dir() -> std::path::PathBuf {
    std::env::var_os("USERPROFILE")
        .map(|h| std::path::PathBuf::from(h).join("Documents"))
        .unwrap_or_default()
}
