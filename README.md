# msi-center-rust

A native Linux app that does what MSI Center does on Windows, for the MSI
Modern 15 B7M (board MS-15HK) — the same card-based layout, but in the colours
of your desktop theme, as one small Rust binary instead of a .NET/WPF app,
three services and a webview.

![Dashboard](docs/screenshot.png)

No Windows support and no Windows code is reused: the hardware is driven
through the in-tree `msi-ec` kernel driver plus battery/LED sysfs.

## Install

**Arch / Omarchy** — one command installs the app package, the matching
`msi-ec` DKMS source, the permissions, and (when Omarchy is detected) the bar
widget:

```sh
curl -fsSL https://github.com/tanmaypanda-14/msi-center-rust/releases/latest/download/install.sh | sudo bash
```

What it does:

- downloads the latest release packages and verifies `SHA256SUMS`
- installs `msi-center` and `msi-ec-dkms` (the DKMS source is owned by a
  package, so pacman can cleanly upgrade or remove it)
- adds the invoking user to `msi-ec`
- removes older `/etc` copies of the permission files, enables
  `msi-ec-perms.service`, and applies the grant immediately
- updates `~/.config/omarchy/plugins/tanmay.msi-power` when Omarchy is present

Use `--no-widget` to skip the widget, `--no-driver` when you install
`msi-ec-dkms` yourself, or `--dry-run` to download and verify without
changing anything:

```sh
url=https://github.com/tanmaypanda-14/msi-center-rust/releases/latest/download/install.sh
curl -fsSL "$url" | sudo bash -s -- --no-widget
curl -fsSL "$url" | bash -s -- --dry-run
```

If you prefer packages by hand, every release also carries the `.pkg.tar.zst`
files:

```sh
sudo pacman -U msi-center-0.3.0-1-x86_64.pkg.tar.zst msi-ec-dkms-0.13-1-any.pkg.tar.zst
sudo usermod -aG msi-ec $USER # then log out and back in
```

**Any distro** — a single binary in a tarball (see
[Releases](https://github.com/tanmaypanda-14/msi-center-rust/releases)):

```sh
tar xf msi-center-0.3.0-x86_64-linux.tar.gz
sudo install -Dm755 msi-center-0.3.0-x86_64-linux/msi-center /usr/local/bin/msi-center
```

Either way, the machine also needs the **`msi-ec` kernel driver** (it cannot be
bundled — kernel modules are built per kernel), and the permission setup from
**[INSTALL.md](INSTALL.md)** if you install from the tarball.

```sh
msi-center --snapshot      # should print backend msi-ec, not mock
```

### Updating

The app updates itself from the command line:

```sh
msi-center --update            # check, ask, install
msi-center --update --dry-run  # print the plan only
msi-center --update --yes      # no prompt (scripts)
```

It resolves the latest release tag, downloads the artifact named in
`SHA256SUMS`, verifies its SHA-256, and then:

- installs the release package with `pacman -U` when `/usr/bin/msi-center` is
  owned by the `msi-center` package, so the package database stays correct;
- replaces a standalone tarball binary in place (atomically when the directory
  is writable).

It never executes a downloaded script. Elevation uses `sudo` in a terminal and
`pkexec` elsewhere. The kernel driver, permissions, and the Omarchy widget are
versioned separately: re-run the installer to update those as well.

## Budgets

| Metric | Target | Current |
| --- | --- | --- |
| Binary (stripped, LTO) | < 10 MB | 10.1 MB |
| Idle RSS | < 30 MB | ~21 MB |
| Processes | 1 | 1 |
| Refresh | 1 s poll, redraw on change | ✓ |

## Theming

The window follows the active Omarchy theme. At startup it reads
`~/.local/state/omarchy/current/theme/colors.toml`, maps the palette onto a
small set of semantic tokens (`bg`, `surface`, `accent`, `muted`, …), and takes
the type scale from the theme's `shell.toml`. Light and dark themes both work:
elevation is derived from `foreground`, and secondary text is contrast-checked
so a theme with dim comments stays readable.

```sh
msi-center --theme                          # theme, source, resolved tokens
MSI_CENTER_THEME=flexoki-light msi-center   # preview another theme
MSI_CENTER_FONT="JetBrainsMono Nerd Font" msi-center
```

Without Omarchy the app uses its built-in palette. The theme is read once,
before the window opens — there is no file watcher, so restart the app after
switching themes. Details and the full token table:
[docs/theming.md](docs/theming.md).

## Layout

```
docker/Dockerfile      build image: Rust + Slint build deps only
docker-compose.yml     dev container; host /sys mounted read-only
crates/core            domain model + MsiControl seam + msi-ec backend
crates/theme           Omarchy colors.toml → semantic tokens; no Slint, no TOML crate
crates/app             window: Slint views + the Rust bridge
  src/cli.rs             argument parsing and usage text
  src/headless.rs        --snapshot, --set, --theme
  src/update/            --update: release check, checksum, install
  src/ui.rs              generated Slint types and globals
  src/theme.rs           msi-theme → Slint Theme global
  src/view.rs            backend snapshot → AppState
  src/wiring.rs          callbacks and the 1 s refresh timer
  src/history.rs         trend ring buffer and chart path
  ui/theme.slint         design tokens, filled in by src/theme.rs
  ui/state.slint         AppState: what views read, what they emit
  ui/components/         nav, cards, controls, window chrome
  ui/pages/              dashboard, cooling, battery, system
  ui/icons.slint         canvas-drawn icons
re/                    reverse-engineering notes on MSI Center 2.0.75.0
```

The views are pure Slint: they read `AppState` and `Theme` and emit callbacks,
and all hardware access, formatting and theming happen in Rust.

## Quick start

```sh
make build     # build the image
make check     # cargo check (container)
make test
make release   # ./target/release/msi-center
make run       # build in the container, run on the host
```

The GUI has to run on the host: it needs your Wayland session and the real EC.
Builds happen in the container; `target/` and `.cargo-home/` are bind-mounted
project directories, so nothing is installed on the host.

### Dependencies

The app expects the `msi_ec` kernel module and the udev rule that makes its
attributes writable by the `msi-ec` group (see `omarchy-msi-power`):

```
/sys/devices/platform/msi-ec/{shift_mode,fan_mode,cooler_boost,fw_version,…}
/sys/class/power_supply/BAT1/charge_control_end_threshold
/sys/class/leds/msiacpi::kbd_backlight/
```

If the driver is missing the app falls back to a mock backend so the UI still
runs.

## Headless use

```sh
msi-center --snapshot            # print everything the UI shows
msi-center --set fan=silent      # shift=eco|comfort|sport, fan=auto|silent|advanced
msi-center --set cooler=on       # battery=60..100, kbd=N
msi-center --set fnswap=on       # swap Fn/Win (off = standard layout)
msi-center --set webcam=on       # EC-level camera block
msi-center --page 2              # open on a given page (0-3)
msi-center --theme               # print the resolved theme and tokens
msi-center --update --dry-run    # check for a newer release
```

## Hardware notes

* Fn/Win swap is one EC bit (`0xBF` bit 4, inverted on this board); "swapped"
  means the Win key sits left of Fn. The driver exposes both `fn_key` and
  `win_key`, so the app reads `win_key` and writes its opposite.
* The driver only loads if the EC firmware string matches its whitelist
  exactly. `15HKEMS1.104` is supported; a newer BIOS firmware may need a newer
  driver. The full board analysis is in
  [`re/msi-ec-ms15hk.md`](re/msi-ec-ms15hk.md).
* Fan RPM is not `cpu/realtime_fan_speed` (that is the EC's thermal level at
  `0x71`). RPM comes from the tick counter at `0xCC/0xCD`, `480000 / ticks`,
  read from the debugfs EC image; without read access to it the RPM row shows
  `--` instead of a wrong number.
* Charge limit is the standard `charge_control_end_threshold` (currently
  owned by root on this laptop — the udev rule needs to cover it).
* Keyboard backlight and webcam block are exposed by `msi-ec` / `msiacpi`.

## Licence

MIT — see [LICENSE](LICENSE).

## Legal

Interoperability work on software the author owns. No MSI code, assets or
branding is copied into this app; the UI only imitates the layout language.
