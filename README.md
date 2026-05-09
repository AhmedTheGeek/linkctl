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

These apply regardless of how you install:

- **Linux** with the `uvcvideo` kernel module (`uname -r` ≥ 4.4 — anything from 2016+). V4L2 is a Linux-only kernel API; macOS, Windows, and WSL1 are not supported.
- A **UVC PTZ webcam** at `/dev/video*`. Optimized for the **Insta360 Link**; other UVC PTZ cams (Logitech CC3000e, Razer Kiyo Pro Ultra, OBSBOT Tiny PTZ) will mostly work since `linkctl` only uses standard UVC controls. Non-PTZ webcams will launch fine but most controls will be absent.
- A terminal emulator that supports the [kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) for held-key motion *(optional, recommended)*: **kitty**, **ghostty**, **foot**, **wezterm**, **alacritty ≥ 0.13**. In other terminals (xterm, gnome-terminal, konsole) motion degrades to one nudge per keypress and a banner explains the fallback.

## Install

### Option 1 — pre-built binary (fastest)

Grab the tarball from the [latest release](https://github.com/AhmedTheGeek/linkctl/releases/latest):

```sh
curl -L https://github.com/AhmedTheGeek/linkctl/releases/download/v0.1.0/linkctl-v0.1.0-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo install -m 755 linkctl-v0.1.0-x86_64-unknown-linux-gnu/linkctl /usr/local/bin/
```

The pre-built binary has tighter requirements than building from source:

- **x86_64 only** (no ARM / Raspberry Pi / Apple Silicon).
- **glibc ≥ 2.39** (the binary is dynamically linked).

| Distro | glibc | Pre-built binary works? |
|---|---|---|
| Arch / EndeavourOS / Manjaro | rolling (≥ 2.43) | ✅ |
| Fedora 40+ | 2.39+ | ✅ |
| Ubuntu 24.04 LTS / 24.10 / 25.04 | 2.39+ | ✅ |
| Debian 13 (Trixie) | 2.41 | ✅ |
| openSUSE Tumbleweed | rolling | ✅ |
| Ubuntu 22.04 LTS | 2.35 | ❌ — build from source |
| Debian 12 (Bookworm) | 2.36 | ❌ — build from source |
| RHEL 9 / Rocky 9 / Alma 9 | 2.34 | ❌ — build from source |
| Any non-x86_64 (ARM, RISC-V) | — | ❌ — build from source for your arch |

If you're not on the list, run `ldd --version | head -1` to check your glibc.

### Option 2 — build from source

This is the most portable path. Works on any architecture, any reasonable glibc, any Linux from the last decade.

You'll need a Rust toolchain (≥ 1.78):

- Arch: `sudo pacman -S rust`
- Fedora: `sudo dnf install cargo`
- Debian / Ubuntu: install via [rustup.rs](https://rustup.rs)

Then:

```sh
git clone https://github.com/AhmedTheGeek/linkctl
cd linkctl
./install.sh
```

`install.sh` runs `cargo install --path .` (lands in `~/.cargo/bin/linkctl`) and adds that directory to your shell rc if it isn't on `$PATH` yet (handles bash, zsh, fish). Pass `--no-path` to skip the rc edit.

### Option 3 — static musl build (broadest reach)

If you want a single binary that runs on **any** x86_64 Linux including ancient distros:

```sh
rustup target add x86_64-unknown-linux-musl
# arch:   sudo pacman -S musl
# debian: sudo apt install musl-tools
cargo build --release --target x86_64-unknown-linux-musl
sudo install -m 755 target/x86_64-unknown-linux-musl/release/linkctl /usr/local/bin/
```

The resulting binary is ~2 MB, fully static, has no glibc dependency, and runs on Ubuntu 18.04 / RHEL 7 etc. without complaint.

### Development build (no install)

```sh
cargo build --release
./target/release/linkctl
```

### Uninstall

```sh
./install.sh --uninstall   # cargo uninstall + remove PATH entry
# or
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
