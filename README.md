# msi-center-rust

A native Linux app that does what MSI Center does on Windows, for the MSI
Modern 15 B7M (board MS-15HK) — same dark, card-based look, but one small Rust
binary instead of a .NET/WPF app, three services and a webview.

![Dashboard](docs/screenshot.png)

No Windows support and no Windows code is reused: the hardware is driven
through the in-tree `msi-ec` kernel driver plus battery/LED sysfs.

## Install

v0.1.0 ships as a single binary: see [Releases](https://github.com/tanmaypanda-14/msi-center-rust/releases).

**A fresh machine needs more than the binary**: the `msi-ec` kernel driver, the
`ec_sys` module for fan RPM, and udev rules so your user can write to the EC.
All of it is covered step by step in **[INSTALL.md](INSTALL.md)**.

```sh
tar xf msi-center-0.1.0-x86_64-linux.tar.gz
sudo install -Dm755 msi-center-0.1.0-x86_64-linux/msi-center /usr/local/bin/msi-center
msi-center --snapshot      # should print backend msi-ec, not mock
```

## Budgets

| Metric | Target | Current |
| --- | --- | --- |
| Binary (stripped, LTO) | < 10 MB | 10.1 MB |
| Idle RSS | < 30 MB | ~21 MB |
| Processes | 1 | 1 |
| Refresh | 1 s poll, redraw on change | ✓ |

## Layout

```
docker/Dockerfile      build image: Rust + Slint build deps only
docker-compose.yml     dev container; host /sys mounted read-only
crates/core            domain model + MsiControl seam + msi-ec backend
crates/app             Slint UI (ui/main.slint) + wiring
re/                    reverse-engineering notes on MSI Center 2.0.75.0
```

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
msi-center --set cooler=on       # battery=60..100, kbd=N, webcam=on|off
msi-center --page 2              # open on a given page (0-3)
```

## Hardware notes

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
