use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::camera::{Camera, CameraError, CtrlId};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileControls {
    pub pan: Option<i64>,
    pub tilt: Option<i64>,
    pub zoom: Option<i64>,
    pub brightness: Option<i64>,
    pub contrast: Option<i64>,
    pub saturation: Option<i64>,
    pub hue: Option<i64>,
    pub sharpness: Option<i64>,
    pub wb_auto: Option<bool>,
    pub wb_temp: Option<i64>,
    pub focus_auto: Option<bool>,
    pub focus: Option<i64>,
    pub power_line_freq: Option<i64>,
}

impl ProfileControls {
    pub fn from_camera(cam: &Camera) -> Self {
        let g = |id| cam.get(id).ok();
        ProfileControls {
            pan: g(CtrlId::Pan),
            tilt: g(CtrlId::Tilt),
            zoom: g(CtrlId::Zoom),
            brightness: g(CtrlId::Brightness),
            contrast: g(CtrlId::Contrast),
            saturation: g(CtrlId::Saturation),
            hue: g(CtrlId::Hue),
            sharpness: g(CtrlId::Sharpness),
            wb_auto: g(CtrlId::WbAuto).map(|v| v != 0),
            wb_temp: cam
                .desc(CtrlId::WbTemp)
                .filter(|d| !d.is_inactive())
                .and_then(|_| g(CtrlId::WbTemp)),
            focus_auto: g(CtrlId::FocusAuto).map(|v| v != 0),
            focus: cam
                .desc(CtrlId::Focus)
                .filter(|d| !d.is_inactive())
                .and_then(|_| g(CtrlId::Focus)),
            power_line_freq: g(CtrlId::PowerLineFreq),
        }
    }

    /// (master writes, dependent writes) in the order they should be applied.
    pub fn to_writes(&self) -> (Vec<(CtrlId, i64)>, Vec<(CtrlId, i64)>) {
        let mut masters = Vec::new();
        let mut deps = Vec::new();
        if let Some(v) = self.wb_auto {
            masters.push((CtrlId::WbAuto, v as i64));
        }
        if let Some(v) = self.focus_auto {
            masters.push((CtrlId::FocusAuto, v as i64));
        }
        if let Some(v) = self.pan {
            deps.push((CtrlId::Pan, v));
        }
        if let Some(v) = self.tilt {
            deps.push((CtrlId::Tilt, v));
        }
        if let Some(v) = self.zoom {
            deps.push((CtrlId::Zoom, v));
        }
        if let Some(v) = self.brightness {
            deps.push((CtrlId::Brightness, v));
        }
        if let Some(v) = self.contrast {
            deps.push((CtrlId::Contrast, v));
        }
        if let Some(v) = self.saturation {
            deps.push((CtrlId::Saturation, v));
        }
        if let Some(v) = self.hue {
            deps.push((CtrlId::Hue, v));
        }
        if let Some(v) = self.sharpness {
            deps.push((CtrlId::Sharpness, v));
        }
        if let Some(v) = self.wb_temp {
            deps.push((CtrlId::WbTemp, v));
        }
        if let Some(v) = self.focus {
            deps.push((CtrlId::Focus, v));
        }
        if let Some(v) = self.power_line_freq {
            deps.push((CtrlId::PowerLineFreq, v));
        }
        (masters, deps)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(flatten)]
    pub controls: ProfileControls,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct ProfileFile {
    version: Option<u32>,
    active: Option<String>,
    #[serde(rename = "profile", default)]
    profiles: Vec<Profile>,
}

#[derive(Debug, Default)]
pub struct ProfileStore {
    pub path: PathBuf,
    pub profiles: Vec<Profile>,
    pub active: Option<String>,
    pub load_warning: Option<String>,
}

impl ProfileStore {
    pub fn default_path() -> Result<PathBuf> {
        let base = dirs::config_dir().context("no XDG config dir")?;
        Ok(base.join("linkctl").join("profiles.toml"))
    }

    pub fn load_or_default(path: PathBuf) -> Self {
        let mut store = ProfileStore {
            path,
            ..Default::default()
        };
        match fs::read_to_string(&store.path) {
            Ok(text) => match toml::from_str::<ProfileFile>(&text) {
                Ok(parsed) => {
                    store.profiles = dedup_by_name(parsed.profiles);
                    store.active = parsed.active;
                }
                Err(e) => {
                    store.load_warning = Some(format!(
                        "profiles.toml unreadable, file preserved ({e})"
                    ));
                }
            },
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                store.load_warning = Some(format!("profiles.toml read error: {e}"));
            }
        }
        store
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create_dir_all {}", parent.display()))?;
        }
        let file = ProfileFile {
            version: Some(1),
            active: self.active.clone(),
            profiles: self.profiles.clone(),
        };
        let text = toml::to_string_pretty(&file).context("serialize profiles")?;
        let tmp = with_extension(&self.path, "toml.tmp");
        fs::write(&tmp, text).with_context(|| format!("write {}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("rename {} -> {}", tmp.display(), self.path.display()))?;
        Ok(())
    }

    pub fn upsert(&mut self, profile: Profile) {
        if let Some(slot) = self.profiles.iter_mut().find(|p| p.name == profile.name) {
            *slot = profile;
        } else {
            self.profiles.push(profile);
        }
    }

    pub fn delete(&mut self, name: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.name != name);
        if self.active.as_deref() == Some(name) {
            self.active = None;
        }
        self.profiles.len() != before
    }

    /// Apply a profile: write masters first, refresh, then dependents.
    pub fn apply(cam: &mut Camera, profile: &Profile) -> Vec<(CtrlId, CameraError)> {
        let (masters, deps) = profile.controls.to_writes();
        let mut errors = Vec::new();
        if !masters.is_empty() {
            errors.extend(cam.apply_many(&masters));
            // Re-query to get refreshed INACTIVE flags before dependent writes.
            let _ = cam.refresh_flags();
        }
        if !deps.is_empty() {
            errors.extend(cam.apply_many(&deps));
        }
        errors
    }
}

fn dedup_by_name(profiles: Vec<Profile>) -> Vec<Profile> {
    let mut seen: BTreeMap<String, Profile> = BTreeMap::new();
    let mut order = Vec::new();
    for p in profiles {
        if !seen.contains_key(&p.name) {
            order.push(p.name.clone());
        }
        seen.insert(p.name.clone(), p);
    }
    order.into_iter().filter_map(|n| seen.remove(&n)).collect()
}

fn with_extension(path: &Path, new_ext: &str) -> PathBuf {
    let mut p = path.to_path_buf();
    p.set_extension(new_ext);
    p
}
