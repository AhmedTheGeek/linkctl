use std::io::{self, Stdout, Write};

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::supports_keyboard_enhancement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    PanLeft,
    PanRight,
    TiltUp,
    TiltDown,
    ZoomIn,
    ZoomOut,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HeldKeys {
    pub pan_l: bool,
    pub pan_r: bool,
    pub tilt_u: bool,
    pub tilt_d: bool,
    pub zoom_i: bool,
    pub zoom_o: bool,
    pub fine: bool,
}

impl HeldKeys {
    /// (pan_steps, tilt_steps, zoom_steps) — net direction this tick.
    pub fn delta(&self) -> (i32, i32, i32) {
        let pan = self.pan_r as i32 - self.pan_l as i32;
        let tilt = self.tilt_u as i32 - self.tilt_d as i32;
        let zoom = self.zoom_i as i32 - self.zoom_o as i32;
        (pan, tilt, zoom)
    }

    pub fn any_motion(&self) -> bool {
        self.pan_l
            || self.pan_r
            || self.tilt_u
            || self.tilt_d
            || self.zoom_i
            || self.zoom_o
    }

    pub fn release_all(&mut self) {
        *self = HeldKeys {
            fine: self.fine,
            ..Default::default()
        };
    }
}

/// Action emitted from a discrete key event (after held-key tracking is updated).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    NudgePan(i32),
    NudgeTilt(i32),
    NudgeZoom(i32),
    AdjustSelected(i32), // ←/→ on Image panel
    NextPanel,
    PrevPanel,
    CursorUp,
    CursorDown,
    LoadSlot(usize),
    SaveProfile,
    DeleteProfile,
    PersistProfiles,
    ToggleAutoOnSelected,
    ResetDefaults,
    HelpToggle,
    Cancel,
    Quit,
    EnterEdit,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Gimbal,
    Image,
    Profiles,
}

impl Panel {
    pub fn next(self) -> Self {
        match self {
            Panel::Gimbal => Panel::Image,
            Panel::Image => Panel::Profiles,
            Panel::Profiles => Panel::Gimbal,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Panel::Gimbal => Panel::Profiles,
            Panel::Image => Panel::Gimbal,
            Panel::Profiles => Panel::Image,
        }
    }
}

/// Update held-key state from a KeyEvent and return the (optional) discrete action.
/// `panel` decides whether arrows act as motion or as cursor moves.
/// `kitty` controls whether KeyEventKind::Release is used; when false, we treat
/// every Press as both press-and-release for held-key motion (so motion is
/// effectively single-step per press).
pub fn handle_key(
    ev: KeyEvent,
    held: &mut HeldKeys,
    panel: Panel,
    kitty: bool,
) -> Action {
    held.fine = ev.modifiers.contains(KeyModifiers::SHIFT);

    let pressed = matches!(ev.kind, KeyEventKind::Press | KeyEventKind::Repeat);
    let released = matches!(ev.kind, KeyEventKind::Release);

    // ---------- gimbal motion bindings (only meaningful in Gimbal panel) ----------
    let dir = motion_direction(ev.code, panel);
    if let Some(d) = dir {
        if kitty {
            update_held(held, d, pressed, released);
            return Action::None;
        } else if pressed {
            // Fallback: each press emits one nudge action.
            return match d {
                Direction::PanLeft => Action::NudgePan(-1),
                Direction::PanRight => Action::NudgePan(1),
                Direction::TiltUp => Action::NudgeTilt(1),
                Direction::TiltDown => Action::NudgeTilt(-1),
                Direction::ZoomIn => Action::NudgeZoom(1),
                Direction::ZoomOut => Action::NudgeZoom(-1),
            };
        } else {
            return Action::None;
        }
    }

    // Non-motion events only act on Press/Repeat.
    if !pressed {
        return Action::None;
    }

    // ---------- discrete shortcuts ----------
    match (ev.code, ev.modifiers) {
        (KeyCode::Tab, m) if !m.contains(KeyModifiers::SHIFT) => Action::NextPanel,
        (KeyCode::BackTab, _) => Action::PrevPanel,
        (KeyCode::Tab, m) if m.contains(KeyModifiers::SHIFT) => Action::PrevPanel,

        (KeyCode::Char('?'), _) | (KeyCode::F(1), _) => Action::HelpToggle,
        (KeyCode::Esc, _) => Action::Cancel,

        // 'q' / 'Q' / Ctrl+C all quit.
        (KeyCode::Char('q' | 'Q'), _) => Action::Quit,
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Action::Quit,

        // Number slots — only valid digit keys.
        (KeyCode::Char(c @ '1'..='9'), m) if m.is_empty() || m == KeyModifiers::SHIFT => {
            // Shift+digits often produce symbols on US layout; rely on the digit char.
            Action::LoadSlot((c as u8 - b'1') as usize)
        }

        // Shift+letters for write-actions; lowercase letters do other things.
        (KeyCode::Char('S'), m) if m.contains(KeyModifiers::SHIFT) => Action::SaveProfile,
        (KeyCode::Char('D'), m) if m.contains(KeyModifiers::SHIFT) => Action::DeleteProfile,
        (KeyCode::Char('W'), m) if m.contains(KeyModifiers::SHIFT) => Action::PersistProfiles,
        (KeyCode::Char('R'), m) if m.contains(KeyModifiers::SHIFT) => Action::ResetDefaults,

        (KeyCode::Char('t'), _) => Action::ToggleAutoOnSelected,
        (KeyCode::Char('r'), _) => Action::ResetDefaults,
        (KeyCode::Enter, _) => Action::EnterEdit,

        // Cursor / adjust in non-Gimbal panels.
        (KeyCode::Up, _) if panel != Panel::Gimbal => Action::CursorUp,
        (KeyCode::Down, _) if panel != Panel::Gimbal => Action::CursorDown,
        (KeyCode::Char('k'), _) if panel != Panel::Gimbal => Action::CursorUp,
        (KeyCode::Char('j'), _) if panel != Panel::Gimbal => Action::CursorDown,
        (KeyCode::Left, _) | (KeyCode::Char('h'), _) if panel == Panel::Image => {
            Action::AdjustSelected(-1)
        }
        (KeyCode::Right, _) | (KeyCode::Char('l'), _) if panel == Panel::Image => {
            Action::AdjustSelected(1)
        }

        _ => Action::None,
    }
}

fn motion_direction(code: KeyCode, panel: Panel) -> Option<Direction> {
    // WASD always works in Gimbal; arrows only in Gimbal (in other panels they're cursor moves).
    let in_gimbal = panel == Panel::Gimbal;
    // Pan is inverted vs. pan_absolute so pressing "right" tracks the user's
    // visual right (image pans right). The Insta360 Link's pan_absolute axis
    // increases toward what feels like "left" from the operator's seat.
    match code {
        KeyCode::Char('a' | 'A') => Some(Direction::PanRight),
        KeyCode::Char('d') => Some(Direction::PanLeft),
        // 'D' is reserved for delete-profile shortcut; only lowercase d pans.
        KeyCode::Char('w' | 'W') => Some(Direction::TiltUp),
        KeyCode::Char('s') => Some(Direction::TiltDown),
        // 'S' (Shift+s) saves a profile; only lowercase s tilts.
        KeyCode::Left if in_gimbal => Some(Direction::PanRight),
        KeyCode::Right if in_gimbal => Some(Direction::PanLeft),
        KeyCode::Up if in_gimbal => Some(Direction::TiltUp),
        KeyCode::Down if in_gimbal => Some(Direction::TiltDown),
        KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::PageUp => Some(Direction::ZoomIn),
        KeyCode::Char('-') | KeyCode::Char('_') | KeyCode::PageDown => Some(Direction::ZoomOut),
        _ => None,
    }
}

fn update_held(held: &mut HeldKeys, d: Direction, pressed: bool, released: bool) {
    let slot = match d {
        Direction::PanLeft => &mut held.pan_l,
        Direction::PanRight => &mut held.pan_r,
        Direction::TiltUp => &mut held.tilt_u,
        Direction::TiltDown => &mut held.tilt_d,
        Direction::ZoomIn => &mut held.zoom_i,
        Direction::ZoomOut => &mut held.zoom_o,
    };
    if pressed {
        *slot = true;
    }
    if released {
        *slot = false;
    }
}

pub struct KeyboardEnhancement {
    enabled: bool,
}

impl KeyboardEnhancement {
    pub fn enable(out: &mut Stdout) -> Self {
        let supported = supports_keyboard_enhancement().unwrap_or(false);
        if supported {
            let _ = execute!(
                out,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                        | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                )
            );
        }
        KeyboardEnhancement { enabled: supported }
    }

    pub fn supported(&self) -> bool {
        self.enabled
    }

    pub fn disable(self) {
        if self.enabled {
            let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
            let _ = io::stdout().flush();
        }
    }
}

/// Best-effort restoration that doesn't borrow stdout exclusively — for panic hooks.
pub fn pop_kbd_flags() {
    let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    let _ = io::stdout().flush();
}
