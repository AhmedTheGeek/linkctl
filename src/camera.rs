use std::collections::HashMap;
use std::io;

use anyhow::{Context, Result};
use thiserror::Error;
use v4l::control::{Control, Description, Flags, MenuItem, Type, Value};
use v4l::Device;

const V4L2_CID_BRIGHTNESS: u32 = 0x0098_0900;
const V4L2_CID_CONTRAST: u32 = 0x0098_0901;
const V4L2_CID_SATURATION: u32 = 0x0098_0902;
const V4L2_CID_HUE: u32 = 0x0098_0903;
const V4L2_CID_AUTO_WHITE_BALANCE: u32 = 0x0098_090c;
const V4L2_CID_POWER_LINE_FREQUENCY: u32 = 0x0098_0918;
const V4L2_CID_WHITE_BALANCE_TEMPERATURE: u32 = 0x0098_091a;
const V4L2_CID_SHARPNESS: u32 = 0x0098_091b;

const V4L2_CID_PAN_ABSOLUTE: u32 = 0x009a_0908;
const V4L2_CID_TILT_ABSOLUTE: u32 = 0x009a_0909;
const V4L2_CID_FOCUS_ABSOLUTE: u32 = 0x009a_090a;
const V4L2_CID_FOCUS_AUTO: u32 = 0x009a_090c;
const V4L2_CID_ZOOM_ABSOLUTE: u32 = 0x009a_090d;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CtrlId {
    Brightness,
    Contrast,
    Saturation,
    Hue,
    WbAuto,
    PowerLineFreq,
    WbTemp,
    Sharpness,
    Pan,
    Tilt,
    FocusAuto,
    Focus,
    Zoom,
}

impl CtrlId {
    pub const fn v4l2_id(self) -> u32 {
        match self {
            CtrlId::Brightness => V4L2_CID_BRIGHTNESS,
            CtrlId::Contrast => V4L2_CID_CONTRAST,
            CtrlId::Saturation => V4L2_CID_SATURATION,
            CtrlId::Hue => V4L2_CID_HUE,
            CtrlId::WbAuto => V4L2_CID_AUTO_WHITE_BALANCE,
            CtrlId::PowerLineFreq => V4L2_CID_POWER_LINE_FREQUENCY,
            CtrlId::WbTemp => V4L2_CID_WHITE_BALANCE_TEMPERATURE,
            CtrlId::Sharpness => V4L2_CID_SHARPNESS,
            CtrlId::Pan => V4L2_CID_PAN_ABSOLUTE,
            CtrlId::Tilt => V4L2_CID_TILT_ABSOLUTE,
            CtrlId::FocusAuto => V4L2_CID_FOCUS_AUTO,
            CtrlId::Focus => V4L2_CID_FOCUS_ABSOLUTE,
            CtrlId::Zoom => V4L2_CID_ZOOM_ABSOLUTE,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            CtrlId::Brightness => "brightness",
            CtrlId::Contrast => "contrast",
            CtrlId::Saturation => "saturation",
            CtrlId::Hue => "hue",
            CtrlId::WbAuto => "wb_auto",
            CtrlId::PowerLineFreq => "power_line_freq",
            CtrlId::WbTemp => "wb_temp",
            CtrlId::Sharpness => "sharpness",
            CtrlId::Pan => "pan",
            CtrlId::Tilt => "tilt",
            CtrlId::FocusAuto => "focus_auto",
            CtrlId::Focus => "focus",
            CtrlId::Zoom => "zoom",
        }
    }

    pub fn from_v4l2(id: u32) -> Option<Self> {
        Some(match id {
            V4L2_CID_BRIGHTNESS => CtrlId::Brightness,
            V4L2_CID_CONTRAST => CtrlId::Contrast,
            V4L2_CID_SATURATION => CtrlId::Saturation,
            V4L2_CID_HUE => CtrlId::Hue,
            V4L2_CID_AUTO_WHITE_BALANCE => CtrlId::WbAuto,
            V4L2_CID_POWER_LINE_FREQUENCY => CtrlId::PowerLineFreq,
            V4L2_CID_WHITE_BALANCE_TEMPERATURE => CtrlId::WbTemp,
            V4L2_CID_SHARPNESS => CtrlId::Sharpness,
            V4L2_CID_PAN_ABSOLUTE => CtrlId::Pan,
            V4L2_CID_TILT_ABSOLUTE => CtrlId::Tilt,
            V4L2_CID_FOCUS_AUTO => CtrlId::FocusAuto,
            V4L2_CID_FOCUS_ABSOLUTE => CtrlId::Focus,
            V4L2_CID_ZOOM_ABSOLUTE => CtrlId::Zoom,
            _ => return None,
        })
    }
}

pub const ALL_IDS: &[CtrlId] = &[
    CtrlId::Brightness,
    CtrlId::Contrast,
    CtrlId::Saturation,
    CtrlId::Hue,
    CtrlId::WbAuto,
    CtrlId::WbTemp,
    CtrlId::Sharpness,
    CtrlId::PowerLineFreq,
    CtrlId::Pan,
    CtrlId::Tilt,
    CtrlId::FocusAuto,
    CtrlId::Focus,
    CtrlId::Zoom,
];

#[derive(Debug, Clone)]
pub enum CtrlKind {
    Int,
    Bool,
    Menu(Vec<(i64, String)>),
}

#[derive(Debug, Clone)]
pub struct ControlDesc {
    #[allow(dead_code)]
    pub id: CtrlId,
    pub kind: CtrlKind,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub default: i64,
    pub flags: Flags,
}

impl ControlDesc {
    pub fn is_inactive(&self) -> bool {
        self.flags.contains(Flags::INACTIVE)
    }

    pub fn is_disabled(&self) -> bool {
        self.flags.contains(Flags::DISABLED) || self.flags.contains(Flags::READ_ONLY)
    }
}

#[derive(Debug, Error)]
pub enum CameraError {
    #[error("control {0:?} not exposed by camera")]
    Unsupported(CtrlId),
    #[error("control {0:?} is inactive (auto-master takes precedence)")]
    Inactive(CtrlId),
    #[error("camera disconnected")]
    Disconnected,
    #[error("camera busy: {0}")]
    Busy(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct Camera {
    dev: Device,
    controls: HashMap<CtrlId, ControlDesc>,
}

impl Camera {
    pub fn open(path: &str) -> Result<Self> {
        let dev = Device::with_path(path)
            .with_context(|| format!("failed to open V4L2 device {path}"))?;
        let mut cam = Camera {
            dev,
            controls: HashMap::new(),
        };
        cam.enumerate()?;
        Ok(cam)
    }

    pub fn enumerate(&mut self) -> Result<()> {
        let descs: Vec<Description> = self
            .dev
            .query_controls()
            .context("VIDIOC_QUERY_EXT_CTRL failed")?;
        self.controls.clear();
        for d in descs {
            let Some(id) = CtrlId::from_v4l2(d.id) else {
                continue;
            };
            let kind = match d.typ {
                Type::Boolean => CtrlKind::Bool,
                Type::Menu | Type::IntegerMenu => {
                    let items = d
                        .items
                        .as_ref()
                        .map(|v| {
                            v.iter()
                                .map(|(idx, item)| {
                                    let label = match item {
                                        MenuItem::Name(n) => n.clone(),
                                        MenuItem::Value(v) => v.to_string(),
                                    };
                                    (*idx as i64, label)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    CtrlKind::Menu(items)
                }
                _ => CtrlKind::Int,
            };
            self.controls.insert(
                id,
                ControlDesc {
                    id,
                    kind,
                    min: d.minimum,
                    max: d.maximum,
                    step: d.step.max(1) as i64,
                    default: d.default,
                    flags: d.flags,
                },
            );
        }
        Ok(())
    }

    pub fn desc(&self, id: CtrlId) -> Option<&ControlDesc> {
        self.controls.get(&id)
    }

    pub fn get(&self, id: CtrlId) -> Result<i64, CameraError> {
        if !self.controls.contains_key(&id) {
            return Err(CameraError::Unsupported(id));
        }
        let ctrl = self.dev.control(id.v4l2_id()).map_err(map_io_err)?;
        Ok(match ctrl.value {
            Value::Integer(i) => i,
            Value::Boolean(b) => b as i64,
            _ => 0,
        })
    }

    pub fn set(&self, id: CtrlId, val: i64) -> Result<i64, CameraError> {
        let desc = self.controls.get(&id).ok_or(CameraError::Unsupported(id))?;
        if desc.is_inactive() {
            return Err(CameraError::Inactive(id));
        }
        if desc.is_disabled() {
            return Err(CameraError::Unsupported(id));
        }
        let snapped = clamp_snap(val, desc);
        let value = match desc.kind {
            CtrlKind::Bool => Value::Boolean(snapped != 0),
            _ => Value::Integer(snapped),
        };
        self.dev
            .set_control(Control {
                id: id.v4l2_id(),
                value,
            })
            .map_err(map_io_err)?;
        Ok(snapped)
    }

    /// Refresh the INACTIVE flags after toggling an auto-master.
    pub fn refresh_flags(&mut self) -> Result<()> {
        self.enumerate()
    }

    /// Step a control by `signed_steps * desc.step`, clamping.
    pub fn nudge(&self, id: CtrlId, signed_steps: i32, current: i64) -> Result<i64, CameraError> {
        let desc = self.controls.get(&id).ok_or(CameraError::Unsupported(id))?;
        let target = current.saturating_add((signed_steps as i64).saturating_mul(desc.step));
        self.set(id, target)
    }

    /// Apply a list of (id, value). Skips Unsupported / Inactive entries.
    /// Tries an atomic write first; on failure, retries each control
    /// individually so the returned error list identifies the real culprit(s).
    pub fn apply_many(&self, items: &[(CtrlId, i64)]) -> Vec<(CtrlId, CameraError)> {
        let mut errors = Vec::new();
        let mut writable: Vec<(CtrlId, i64, CtrlKind)> = Vec::new();
        for (id, raw) in items {
            let Some(desc) = self.controls.get(id) else {
                errors.push((*id, CameraError::Unsupported(*id)));
                continue;
            };
            if desc.is_inactive() {
                errors.push((*id, CameraError::Inactive(*id)));
                continue;
            }
            if desc.is_disabled() {
                errors.push((*id, CameraError::Unsupported(*id)));
                continue;
            }
            writable.push((*id, clamp_snap(*raw, desc), desc.kind.clone()));
        }
        if writable.is_empty() {
            return errors;
        }
        let to_control = |id: CtrlId, val: i64, kind: &CtrlKind| Control {
            id: id.v4l2_id(),
            value: match kind {
                CtrlKind::Bool => Value::Boolean(val != 0),
                _ => Value::Integer(val),
            },
        };
        let atomic: Vec<Control> = writable
            .iter()
            .map(|(id, val, kind)| to_control(*id, *val, kind))
            .collect();
        if self.dev.set_controls(atomic).is_ok() {
            return errors;
        }
        // Atomic batch was rejected; try each individually to surface the real cause.
        for (id, val, kind) in writable {
            if let Err(e) = self.dev.set_control(to_control(id, val, &kind)) {
                errors.push((id, map_io_err(e)));
            }
        }
        errors
    }
}

pub fn clamp_snap(val: i64, desc: &ControlDesc) -> i64 {
    let step = desc.step.max(1);
    let clamped = val.clamp(desc.min, desc.max);
    let rel = clamped - desc.min;
    let snapped = desc.min + (rel / step) * step;
    snapped.clamp(desc.min, desc.max)
}

fn map_io_err(e: io::Error) -> CameraError {
    match e.raw_os_error() {
        Some(libc_enodev) if libc_enodev == 19 => CameraError::Disconnected,
        Some(ebusy) if ebusy == 16 => CameraError::Busy(e.to_string()),
        _ => CameraError::Io(e),
    }
}
