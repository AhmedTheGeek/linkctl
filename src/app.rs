use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::camera::{Camera, CameraError, CtrlId, CtrlKind, ALL_IDS};
use crate::input::{
    handle_key, Action, HeldKeys, KeyboardEnhancement, Panel,
};
use crate::profiles::{Profile, ProfileControls, ProfileStore};
use crate::ui;

const TICK: Duration = Duration::from_millis(33);
const STATUS_TTL: Duration = Duration::from_secs(4);

// Velocity-based motion. Each tick we integrate (rate × dt) into an accumulator
// and only fire a camera write when the accumulator crosses one full step
// (3600 arcsec for pan/tilt, 1 unit for zoom). This decouples motion smoothness
// from tick rate and keeps brief taps to ~1° instead of leaping by step×count.
const PAN_DEG_PER_SEC_NORMAL: f64 = 50.0;
const PAN_DEG_PER_SEC_FINE: f64 = 12.0;
const ZOOM_UNITS_PER_SEC_NORMAL: f64 = 80.0;
const ZOOM_UNITS_PER_SEC_FINE: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct StatusLine {
    pub text: String,
    pub level: StatusLevel,
    pub expires_at: Option<Instant>,
}

impl Default for StatusLine {
    fn default() -> Self {
        StatusLine {
            text: String::new(),
            level: StatusLevel::Info,
            expires_at: None,
        }
    }
}

impl StatusLine {
    fn set(&mut self, text: impl Into<String>, level: StatusLevel) {
        self.text = text.into();
        self.level = level;
        self.expires_at = Some(Instant::now() + STATUS_TTL);
    }
    /// Like `set` but the message stays until the user does something else
    /// (Tab, panel switch, next status update). Used for error detail the
    /// user needs time to read.
    fn set_long(&mut self, text: impl Into<String>, level: StatusLevel) {
        self.text = text.into();
        self.level = level;
        self.expires_at = None;
    }
    fn tick(&mut self) {
        if let Some(t) = self.expires_at {
            if Instant::now() >= t {
                self.text.clear();
                self.expires_at = None;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeleteProfile(String),
    ResetDefaults,
    QuitWithDirty,
}

#[derive(Debug, Clone)]
pub enum EditMode {
    None,
    EditingValue(CtrlId, String),
    NamingProfile(String),
    Confirm(String, ConfirmAction),
    Help,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MotionAccum {
    pub pan: f64,  // accumulated arcseconds
    pub tilt: f64,
    pub zoom: f64, // accumulated zoom units
}

pub struct App {
    pub camera: Camera,
    pub device_path: String,
    pub values: HashMap<CtrlId, i64>,
    pub bounds: HashMap<CtrlId, (i64, i64, i64)>, // (min, max, step)
    pub held: HeldKeys,
    pub prev_held: HeldKeys,
    pub motion_accum: MotionAccum,
    pub kitty_supported: bool,
    pub connected: bool,
    pub focus: Panel,
    pub image_cursor: usize,
    pub profile_cursor: usize,
    pub edit: EditMode,
    pub profiles: ProfileStore,
    pub status: StatusLine,
    pub dirty_profiles: bool,
    pub needs_redraw: bool,
    pub last_tick: Instant,
    pub last_reconcile: Instant,
    pub should_quit: bool,
}

impl App {
    pub fn new(device_path: String) -> Result<Self> {
        let camera = Camera::open(&device_path)?;
        let profile_path = ProfileStore::default_path()?;
        let profiles = ProfileStore::load_or_default(profile_path);

        let mut app = App {
            camera,
            device_path,
            values: HashMap::new(),
            bounds: HashMap::new(),
            held: HeldKeys::default(),
            prev_held: HeldKeys::default(),
            motion_accum: MotionAccum::default(),
            kitty_supported: false,
            connected: true,
            focus: Panel::Gimbal,
            image_cursor: 0,
            profile_cursor: 0,
            edit: EditMode::None,
            profiles,
            status: StatusLine::default(),
            dirty_profiles: false,
            needs_redraw: true,
            last_tick: Instant::now(),
            last_reconcile: Instant::now(),
            should_quit: false,
        };
        app.snapshot_bounds();
        app.reconcile_values();
        if let Some(warn) = app.profiles.load_warning.take() {
            app.status.set(warn, StatusLevel::Warn);
        }
        Ok(app)
    }

    pub fn value(&self, id: CtrlId) -> Option<i64> {
        self.values.get(&id).copied()
    }

    fn snapshot_bounds(&mut self) {
        self.bounds.clear();
        for id in ALL_IDS {
            if let Some(d) = self.camera.desc(*id) {
                self.bounds.insert(*id, (d.min, d.max, d.step));
            }
        }
    }

    fn reconcile_values(&mut self) {
        for id in ALL_IDS {
            if self.camera.desc(*id).is_none() {
                continue;
            }
            match self.camera.get(*id) {
                Ok(v) => {
                    self.values.insert(*id, v);
                }
                Err(CameraError::Disconnected) => {
                    self.connected = false;
                    return;
                }
                Err(_) => {}
            }
        }
        self.last_reconcile = Instant::now();
    }
}

pub fn run(device_path: String) -> Result<()> {
    let mut terminal = ratatui::try_init().context("init terminal")?;
    let kbd = KeyboardEnhancement::enable(&mut io::stdout());

    // Chain a panic hook on top of ratatui's that also pops kbd flags.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        crate::input::pop_kbd_flags();
        prev_hook(info);
    }));

    let mut app = match App::new(device_path) {
        Ok(a) => a,
        Err(e) => {
            kbd.disable();
            ratatui::restore();
            return Err(e);
        }
    };
    app.kitty_supported = kbd.supported();
    if !app.kitty_supported {
        app.status.set(
            "Kitty keyboard protocol unavailable — held-key motion disabled (use kitty/ghostty/foot for smooth motion)",
            StatusLevel::Warn,
        );
    }

    let result = main_loop(&mut terminal, &mut app);

    kbd.disable();
    ratatui::restore();
    result
}

fn main_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.should_quit {
        let timeout = TICK.saturating_sub(app.last_tick.elapsed());
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => on_key(app, key),
                Event::Resize(_, _) => app.needs_redraw = true,
                Event::FocusLost => {
                    // Stop motion if we lose focus while keys are held — we'll never see release.
                    app.held.release_all();
                }
                _ => {}
            }
        }
        if app.last_tick.elapsed() >= TICK {
            tick(app);
            app.last_tick = Instant::now();
        }
        if app.needs_redraw {
            terminal.draw(|f| ui::draw(f, app))?;
            app.needs_redraw = false;
        }
    }

    if app.dirty_profiles {
        if let Err(e) = app.profiles.save() {
            app.status
                .set(format!("save failed: {e}"), StatusLevel::Error);
        }
    }
    Ok(())
}

fn on_key(app: &mut App, ev: KeyEvent) {
    // Text-input mode: capture characters into the buffer.
    match &mut app.edit {
        EditMode::EditingValue(_, buf) | EditMode::NamingProfile(buf) => {
            if matches!(ev.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                use crossterm::event::KeyCode::*;
                match ev.code {
                    Esc => {
                        app.edit = EditMode::None;
                        app.needs_redraw = true;
                    }
                    Enter => commit_edit(app),
                    Backspace => {
                        buf.pop();
                        app.needs_redraw = true;
                    }
                    Char(c) => {
                        buf.push(c);
                        app.needs_redraw = true;
                    }
                    _ => {}
                }
            }
            return;
        }
        EditMode::Confirm(_, action) => {
            if matches!(ev.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                use crossterm::event::KeyCode::*;
                match ev.code {
                    Char('y') | Char('Y') => {
                        let action = action.clone();
                        app.edit = EditMode::None;
                        execute_confirmed(app, action);
                        app.needs_redraw = true;
                    }
                    Char('n') | Char('N') | Esc => {
                        app.edit = EditMode::None;
                        app.needs_redraw = true;
                    }
                    _ => {}
                }
            }
            return;
        }
        EditMode::Help => {
            if matches!(ev.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                app.edit = EditMode::None;
                app.needs_redraw = true;
            }
            return;
        }
        EditMode::None => {}
    }

    let action = handle_key(ev, &mut app.held, app.focus, app.kitty_supported);
    apply_action(app, action);
}

fn apply_action(app: &mut App, action: Action) {
    use Action::*;
    match action {
        None => {}
        NudgePan(steps) => {
            nudge_axis(app, CtrlId::Pan, steps);
        }
        NudgeTilt(steps) => {
            nudge_axis(app, CtrlId::Tilt, steps);
        }
        NudgeZoom(steps) => {
            nudge_axis(app, CtrlId::Zoom, steps);
        }
        NextPanel => {
            app.focus = app.focus.next();
            app.needs_redraw = true;
        }
        PrevPanel => {
            app.focus = app.focus.prev();
            app.needs_redraw = true;
        }
        CursorUp => match app.focus {
            Panel::Image => {
                if app.image_cursor > 0 {
                    app.image_cursor -= 1;
                    app.needs_redraw = true;
                }
            }
            Panel::Profiles => {
                if app.profile_cursor > 0 {
                    app.profile_cursor -= 1;
                    app.needs_redraw = true;
                }
            }
            Panel::Gimbal => {}
        },
        CursorDown => match app.focus {
            Panel::Image => {
                let max = ui::image::IMAGE_ROWS.len().saturating_sub(1);
                if app.image_cursor < max {
                    app.image_cursor += 1;
                    app.needs_redraw = true;
                }
            }
            Panel::Profiles => {
                let max = app.profiles.profiles.len().saturating_sub(1);
                if app.profile_cursor < max {
                    app.profile_cursor += 1;
                    app.needs_redraw = true;
                }
            }
            Panel::Gimbal => {}
        },
        AdjustSelected(steps) => {
            if let Some(id) = ui::image::IMAGE_ROWS.get(app.image_cursor).copied() {
                let step_count = if app.held.fine { steps } else { steps * 5 };
                nudge_control(app, id, step_count);
            }
        }
        ToggleAutoOnSelected => toggle_auto(app),
        LoadSlot(idx) => load_slot(app, idx),
        SaveProfile => {
            app.edit = EditMode::NamingProfile(String::new());
            app.needs_redraw = true;
        }
        DeleteProfile => {
            if let Some(p) = app.profiles.profiles.get(app.profile_cursor).cloned() {
                let q = format!("Delete profile \"{}\"?", p.name);
                app.edit = EditMode::Confirm(q, ConfirmAction::DeleteProfile(p.name));
                app.needs_redraw = true;
            }
        }
        PersistProfiles => match app.profiles.save() {
            Ok(()) => {
                app.dirty_profiles = false;
                app.status
                    .set("Saved profiles.toml", StatusLevel::Info);
                app.needs_redraw = true;
            }
            Err(e) => {
                app.status
                    .set(format!("save failed: {e}"), StatusLevel::Error);
                app.needs_redraw = true;
            }
        },
        ResetDefaults => {
            app.edit = EditMode::Confirm(
                "Reset all controls to defaults?".to_string(),
                ConfirmAction::ResetDefaults,
            );
            app.needs_redraw = true;
        }
        HelpToggle => {
            app.edit = EditMode::Help;
            app.needs_redraw = true;
        }
        Cancel => {
            app.edit = EditMode::None;
            app.needs_redraw = true;
        }
        Quit => {
            if app.dirty_profiles {
                app.edit = EditMode::Confirm(
                    "Save profile changes before quitting?".to_string(),
                    ConfirmAction::QuitWithDirty,
                );
                app.needs_redraw = true;
            } else {
                app.should_quit = true;
            }
        }
        EnterEdit => match app.focus {
            Panel::Profiles => {
                if let Some(p) = app.profiles.profiles.get(app.profile_cursor).cloned() {
                    apply_profile(app, &p);
                }
            }
            Panel::Image => {
                if let Some(id) = ui::image::IMAGE_ROWS.get(app.image_cursor).copied() {
                    if let Some(d) = app.camera.desc(id) {
                        match d.kind {
                            CtrlKind::Bool => {
                                let cur = app.value(id).unwrap_or(0);
                                set_value(app, id, if cur != 0 { 0 } else { 1 });
                            }
                            CtrlKind::Menu(_) => {
                                cycle_menu(app, id);
                            }
                            CtrlKind::Int => {
                                let cur = app.value(id).unwrap_or(d.default);
                                app.edit = EditMode::EditingValue(id, cur.to_string());
                                app.needs_redraw = true;
                            }
                        }
                    }
                }
            }
            Panel::Gimbal => {}
        },
    }
}

fn execute_confirmed(app: &mut App, action: ConfirmAction) {
    match action {
        ConfirmAction::DeleteProfile(name) => {
            if app.profiles.delete(&name) {
                app.dirty_profiles = true;
                app.profile_cursor = app
                    .profile_cursor
                    .min(app.profiles.profiles.len().saturating_sub(1));
                app.status
                    .set(format!("deleted profile \"{name}\""), StatusLevel::Info);
            }
        }
        ConfirmAction::ResetDefaults => {
            let mut errors = 0;
            for id in ALL_IDS {
                if let Some(d) = app.camera.desc(*id) {
                    let default = d.default;
                    if let Err(e) = app.camera.set(*id, default) {
                        if !matches!(e, CameraError::Inactive(_) | CameraError::Unsupported(_)) {
                            errors += 1;
                        }
                    } else {
                        app.values.insert(*id, default);
                    }
                }
            }
            let _ = app.camera.refresh_flags();
            app.snapshot_bounds();
            app.status.set(
                if errors == 0 {
                    "reset to defaults".to_string()
                } else {
                    format!("reset (with {errors} errors)")
                },
                if errors == 0 {
                    StatusLevel::Info
                } else {
                    StatusLevel::Warn
                },
            );
        }
        ConfirmAction::QuitWithDirty => {
            if let Err(e) = app.profiles.save() {
                app.status
                    .set(format!("save failed: {e}"), StatusLevel::Error);
                return;
            }
            app.dirty_profiles = false;
            app.should_quit = true;
        }
    }
}

fn commit_edit(app: &mut App) {
    let edit = std::mem::replace(&mut app.edit, EditMode::None);
    match edit {
        EditMode::EditingValue(id, buf) => {
            let trimmed = buf.trim();
            match trimmed.parse::<i64>() {
                Ok(v) => {
                    set_value(app, id, v);
                }
                Err(_) => {
                    app.status
                        .set(format!("invalid number: {buf}"), StatusLevel::Warn);
                }
            }
        }
        EditMode::NamingProfile(buf) => {
            let name = buf.trim().to_string();
            if name.is_empty() {
                app.status.set("name cannot be empty", StatusLevel::Warn);
            } else {
                let controls = ProfileControls::from_camera(&app.camera);
                app.profiles.upsert(Profile {
                    name: name.clone(),
                    controls,
                });
                app.profiles.active = Some(name.clone());
                app.dirty_profiles = true;
                if let Err(e) = app.profiles.save() {
                    app.status
                        .set(format!("save failed: {e}"), StatusLevel::Error);
                } else {
                    app.dirty_profiles = false;
                    app.status
                        .set(format!("saved profile \"{name}\""), StatusLevel::Info);
                }
            }
        }
        other => {
            app.edit = other;
        }
    }
    app.needs_redraw = true;
}

fn nudge_axis(app: &mut App, id: CtrlId, signed_steps: i32) {
    nudge_control(app, id, signed_steps);
}

fn nudge_control(app: &mut App, id: CtrlId, signed_steps: i32) {
    let current = app.value(id).unwrap_or(0);
    match app.camera.nudge(id, signed_steps, current) {
        Ok(v) => {
            if app.values.get(&id) != Some(&v) {
                app.values.insert(id, v);
                app.needs_redraw = true;
            }
        }
        Err(CameraError::Disconnected) => {
            app.connected = false;
            app.held.release_all();
            app.status
                .set("camera disconnected — press R to retry", StatusLevel::Error);
            app.needs_redraw = true;
        }
        Err(CameraError::Inactive(_)) => {
            app.status
                .set(format!("{} is inactive", id.label()), StatusLevel::Warn);
            app.needs_redraw = true;
        }
        Err(e) => {
            app.status.set(format!("{e}"), StatusLevel::Error);
            app.needs_redraw = true;
        }
    }
}

fn set_value(app: &mut App, id: CtrlId, val: i64) {
    match app.camera.set(id, val) {
        Ok(snapped) => {
            app.values.insert(id, snapped);
            // If we toggled an auto-master, refresh flags so dependents grey/un-grey correctly.
            if matches!(id, CtrlId::WbAuto | CtrlId::FocusAuto) {
                let _ = app.camera.refresh_flags();
            }
            app.needs_redraw = true;
        }
        Err(CameraError::Disconnected) => {
            app.connected = false;
            app.status
                .set("camera disconnected", StatusLevel::Error);
            app.needs_redraw = true;
        }
        Err(e) => {
            app.status.set(format!("{e}"), StatusLevel::Error);
            app.needs_redraw = true;
        }
    }
}

fn cycle_menu(app: &mut App, id: CtrlId) {
    let Some(d) = app.camera.desc(id) else { return };
    let CtrlKind::Menu(items) = &d.kind else { return };
    if items.is_empty() {
        return;
    }
    let current = app.value(id).unwrap_or(d.default);
    let idx = items.iter().position(|(v, _)| *v == current).unwrap_or(0);
    let next = (idx + 1) % items.len();
    let next_val = items[next].0;
    set_value(app, id, next_val);
}

fn toggle_auto(app: &mut App) {
    if app.focus != Panel::Image {
        return;
    }
    let Some(id) = ui::image::IMAGE_ROWS.get(app.image_cursor).copied() else { return };
    let target = match id {
        CtrlId::WbAuto | CtrlId::FocusAuto => id,
        CtrlId::WbTemp => CtrlId::WbAuto,
        CtrlId::Focus => CtrlId::FocusAuto,
        _ => {
            app.status
                .set("no auto-master for this control", StatusLevel::Warn);
            app.needs_redraw = true;
            return;
        }
    };
    let cur = app.value(target).unwrap_or(0);
    set_value(app, target, if cur != 0 { 0 } else { 1 });
}

fn load_slot(app: &mut App, idx: usize) {
    let Some(p) = app.profiles.profiles.get(idx).cloned() else { return };
    apply_profile(app, &p);
}

fn apply_profile(app: &mut App, profile: &Profile) {
    let errors = ProfileStore::apply(&mut app.camera, profile);
    app.snapshot_bounds();
    app.reconcile_values();
    app.profiles.active = Some(profile.name.clone());
    if errors.is_empty() {
        app.status
            .set(format!("loaded profile \"{}\"", profile.name), StatusLevel::Info);
    } else {
        let detail = errors
            .iter()
            .take(3)
            .map(|(id, e)| format!("{}={e}", id.label()))
            .collect::<Vec<_>>()
            .join(", ");
        let extra = if errors.len() > 3 {
            format!(" (+{} more)", errors.len() - 3)
        } else {
            String::new()
        };
        app.status.set_long(
            format!(
                "loaded \"{}\" — {} error(s): {detail}{extra}",
                profile.name,
                errors.len()
            ),
            StatusLevel::Warn,
        );
    }
    app.needs_redraw = true;
}

fn tick(app: &mut App) {
    app.status.tick();
    if !app.connected {
        // Try to reopen the device.
        if let Ok(cam) = Camera::open(&app.device_path) {
            app.camera = cam;
            app.connected = true;
            app.snapshot_bounds();
            app.reconcile_values();
            app.status.set("camera reconnected", StatusLevel::Info);
            app.needs_redraw = true;
        }
        return;
    }

    if !app.held.any_motion() {
        app.motion_accum = MotionAccum::default();
        app.prev_held = app.held;
        if app.last_reconcile.elapsed() > Duration::from_secs(2) {
            self_reconcile(app);
        }
        return;
    }

    // Cap dt to avoid huge bursts after a stall (e.g. resize, slow ioctl).
    let dt = app.last_tick.elapsed().as_secs_f64().min(0.1);
    let (pan_dir, tilt_dir, zoom_dir) = app.held.delta();
    let (prev_pan, prev_tilt, prev_zoom) = app.prev_held.delta();
    let pan_rate = if app.held.fine {
        PAN_DEG_PER_SEC_FINE
    } else {
        PAN_DEG_PER_SEC_NORMAL
    };
    let zoom_rate = if app.held.fine {
        ZOOM_UNITS_PER_SEC_FINE
    } else {
        ZOOM_UNITS_PER_SEC_NORMAL
    };

    // Press-transition kick: a single 1-step nudge on the tick a direction first
    // appears. Guarantees taps register at any speed (otherwise fine-mode taps
    // shorter than ~83 ms accumulate <1 step and fire nothing).
    if pan_dir != 0 && pan_dir != prev_pan {
        nudge_axis(app, CtrlId::Pan, pan_dir);
    }
    if tilt_dir != 0 && tilt_dir != prev_tilt {
        nudge_axis(app, CtrlId::Tilt, tilt_dir);
    }
    if zoom_dir != 0 && zoom_dir != prev_zoom {
        nudge_axis(app, CtrlId::Zoom, zoom_dir);
    }

    accumulate_axis(app, CtrlId::Pan, pan_dir, pan_rate * 3600.0, dt, |a| &mut a.motion_accum.pan);
    accumulate_axis(app, CtrlId::Tilt, tilt_dir, pan_rate * 3600.0, dt, |a| &mut a.motion_accum.tilt);
    accumulate_axis(app, CtrlId::Zoom, zoom_dir, zoom_rate, dt, |a| &mut a.motion_accum.zoom);

    app.prev_held = app.held;
}

fn accumulate_axis(
    app: &mut App,
    id: CtrlId,
    dir: i32,
    rate_units_per_sec: f64,
    dt: f64,
    accum_field: impl Fn(&mut App) -> &mut f64,
) {
    if dir == 0 {
        *accum_field(app) = 0.0;
        return;
    }
    let step = app.bounds.get(&id).map(|b| b.2).unwrap_or(1) as f64;
    let accum = accum_field(app);
    *accum += dir as f64 * rate_units_per_sec * dt;
    let count = (*accum / step).trunc() as i32;
    if count != 0 {
        *accum -= count as f64 * step;
        nudge_axis(app, id, count);
    }
}

fn self_reconcile(app: &mut App) {
    // Slow sanity reconcile — only refresh if we got something different.
    let prev = app.values.clone();
    app.reconcile_values();
    if prev != app.values {
        app.needs_redraw = true;
    }
}
