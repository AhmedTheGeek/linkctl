# linkctl

A terminal controller for the [Insta360 Link](https://www.insta360.com/product/insta360-link) UVC PTZ webcam on Linux.

Insta360 ships no Linux client. `linkctl` drives the camera over V4L2 ioctls — no `v4l2-ctl` shell-out, no streaming side effects (control-only access). It gives you:

- **Held-key gimbal motion** with WASD or arrow keys (kitty keyboard protocol; falls back to per-press nudge in legacy terminals).
- **Image controls**: brightness, contrast, saturation, hue, sharpness, white balance (auto / temperature), focus (auto / manual), power-line frequency.
- **Named profiles** persisted to `~/.config/linkctl/profiles.toml`. Save the current state, recall it instantly with `1`–`9`.
- **Fine-tune mode** — hold Shift while moving for ~12 °/s precision instead of 50 °/s.

```
┌─ Insta360 Link  /dev/video0   pan +10.0°  tilt -2.0°  zoom 1.50x ──── kbd:kitty ─┐
│                                                                                    │
│  ┌─ Gimbal ────────────────┐  ┌─ Image ─────────────────┐  ┌─ Profiles ──────┐  │
│  │                         │  │ brightness  [####----] 55│  │ ▶ 1 desk     *  │  │
│  │           o             │  │ contrast    [#####---] 50│  │   2 meeting     │  │
│  │                         │  │ saturation  [#####---] 50│  │   3 lowlight    │  │
│  │                         │  │ wb_auto     [x] on        │  │                 │  │
│  └─────────────────────────┘  │ wb_temp     (inactive)    │  │                 │  │
│  pan  -145°.....+145°         │ focus_auto  [x] on        │  │                 │  │
│  tilt  -90°.....+100°         │ focus       (inactive)    │  │                 │  │
│  zoom 1.50x                   │ power_line  60Hz          │  │                 │  │
│                                                                                    │
│ WASD/← → ↑ ↓ pan/tilt  +/- zoom  Shift fine  Tab panel  1-9 load  ⇧S save        │
│ status: loaded profile "desk"                                                     │
└────────────────────────────────────────────────────────────────────────────────────┘
```

## Requirements

- Linux with the `uvcvideo` kernel module (built-in on every distro).
- An Insta360 Link plugged in (other UVC PTZ webcams may work but are untested).
- A Rust toolchain (≥ 1.78). Arch: `pacman -S rust`. Fedora: `dnf install cargo`. Debian/Ubuntu: install via [rustup.rs](https://rustup.rs).
- A terminal emulator that supports the [kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) for held-key motion (recommended): **kitty**, **ghostty**, **foot**, **wezterm**, or **alacritty ≥ 0.13**. In other terminals (xterm, gnome-terminal, konsole), motion degrades to one nudge per keypress and a banner explains the fallback.

## Install

### Quick install (recommended)

```sh
./install.sh
```

This builds in release mode and installs `linkctl` to `~/.cargo/bin/`. Make sure that directory is on your `$PATH` (`rustup` adds it automatically; otherwise add `export PATH="$HOME/.cargo/bin:$PATH"` to your shell rc).

### Manual install

```sh
cargo install --path .
```

### Development build (no install)

```sh
cargo build --release
./target/release/linkctl
```

### Uninstall

```sh
cargo uninstall linkctl
```

## Usage

```sh
linkctl                      # uses /dev/video0
linkctl --device /dev/video2 # different capture device
linkctl --help
```

### Keymap

| Key | Action |
|---|---|
| `W` `A` `S` `D` / arrow keys | tilt up / pan right / tilt down / pan left (held; pan is inverted to track your visual right) |
| `Shift` (with motion) | fine mode — 12 °/s and 0.2x/s zoom |
| `+` `=` PageUp / `-` PageDown | zoom in / out (held) |
| `Tab` / `Shift+Tab` | next / previous panel |
| `↑` `↓` / `k` `j` (Image, Profiles) | move cursor |
| `←` `→` / `h` `l` (Image) | decrement / increment focused control |
| `Enter` | edit numeric value (Image) / load selected profile (Profiles) |
| `T` | toggle auto-master on focused row (`wb_auto`, `focus_auto`) |
| `1` – `9` | quick-load profile slot |
| `⇧S` | save current state as a new profile (prompts name) |
| `⇧D` | delete selected profile (confirm) |
| `⇧W` | persist `profiles.toml` now |
| `R` / `⇧R` | reset all controls to defaults (confirm) |
| `?` / `F1` | help overlay |
| `Esc` | cancel edit / close overlay |
| `Q` / `Ctrl+C` | quit (prompts to save if dirty) |

### Profile file

Stored at `~/.config/linkctl/profiles.toml`. Hand-editable, atomic writes (write `.tmp` then rename), forward-compatible (unknown fields ignored).

```toml
version = 1
active  = "desk"

[[profile]]
name        = "desk"
pan         = 36000        # arcseconds (10°)
tilt        = -7200
zoom        = 150
brightness  = 55
contrast    = 50
saturation  = 50
hue         = 0
sharpness   = 60
wb_auto     = false
wb_temp     = 5200         # omitted when wb_auto is true
focus_auto  = true
power_line_freq = 2        # 60 Hz
```

## Tuning motion speed

The four motion-rate constants live at the top of [`src/app.rs`](src/app.rs):

```rust
const PAN_DEG_PER_SEC_NORMAL: f64 = 50.0;
const PAN_DEG_PER_SEC_FINE:   f64 = 12.0;
const ZOOM_UNITS_PER_SEC_NORMAL: f64 = 80.0;
const ZOOM_UNITS_PER_SEC_FINE:   f64 = 20.0;
```

Drop them to `30.0` / `8.0` for slower defaults. The motion uses a velocity accumulator that integrates `rate × dt` per tick and fires a 1° step (the camera's `pan_absolute` step) whenever the accumulator crosses a full step — so changing the rate does not introduce step quantization artifacts.

## Troubleshooting

- **"Kitty keyboard protocol unavailable" banner** — your terminal doesn't report key-release events. Switch to kitty / ghostty / foot / wezterm, or just live with per-press nudge.
- **`failed to open V4L2 device`** — wrong `--device`. Run `v4l2-ctl --list-devices` to find the right one.
- **`camera disconnected`** — `linkctl` retries every 2 s. Replug the camera, or press `R` to force a reset.
- **Profile applies with `N error(s)`** — the status line names the offenders. Most common cause is writing to a control whose auto-master is on (e.g. `wb_temp` while `wb_auto` is true). The save flow already filters these, but legacy profile files from before that fix may still trip.
- **Camera busy** — another process is streaming. Close it (e.g. quit OBS, Zoom, browser tabs holding `getUserMedia`).

## License

MIT.
