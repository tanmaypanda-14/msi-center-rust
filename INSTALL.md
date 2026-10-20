# Installing msi-center

Tested on an **MSI Modern 15 B7M (MS-15HK)**, Omarchy/Arch, kernel
`7.2.8-9-omarchy`, `msi-ec` DKMS 0.13. Other boards supported by `msi-ec`
should work, with one caveat about fan RPM (see [Other models](#other-models)).

## At a glance

| Requirement | Why | Needed |
| --- | --- | --- |
| Linux x86_64, **Wayland** session | the release is built with the Wayland backend only | yes |
| glibc ≥ 2.35 | binary is built on Debian 12 | yes |
| `msi-ec` kernel module ≥ 0.13 | every control on the dashboard | **yes** |
| `ec_sys` module | fan tachometer → RPM (the rest works without it) | recommended |
| `msi-ec` group + udev rules | lets your user write to the EC attributes | yes |
| fontconfig + freetype2 | text rendering (present on any desktop) | yes |
| docker + docker compose | only if you build from source | optional |

Nothing else is needed: no .NET, no webview, no daemon.

---

## 1. Kernel driver: `msi-ec`

This is the part a fresh laptop will not have. The app talks to
`/sys/devices/platform/msi-ec/`, which the out-of-tree `msi-ec` driver creates.

```sh
# is it already there?
ls /sys/devices/platform/msi-ec
modinfo msi_ec
```

If that fails, install it from source (DKMS, survives kernel updates):

```sh
git clone https://github.com/BeardOverflow/msi-ec
cd msi-ec
sudo make dkms-install
sudo modprobe msi_ec
```

Newer kernels (6.15+) may ship `msi_ec` in-tree — check `modinfo msi_ec` before
building anything. The module loads automatically on supported hardware
(matching `modalias`); `echo msi_ec | sudo tee /etc/modules-load.d/msi-ec.conf`
makes it explicit.

Verify:

```sh
$ cat /sys/devices/platform/msi-ec/available_shift_modes
eco
comfort
sport
$ cat /sys/devices/platform/msi-ec/available_fan_modes
auto
silent
advanced
```

An empty or missing file means this EC firmware is newer than your driver
version — update `msi-ec`.

## 2. `ec_sys` (for fan RPM)

`msi-ec` exposes the EC's *thermal level*, not RPM. Real RPM is computed from
the tachometer ticks in the raw EC image:

```sh
sudo modprobe ec_sys write_support=0
dd if=/sys/kernel/debug/ec/ec0/io bs=1 skip=204 count=2 2>/dev/null | od -An -tu1
```

Make it persistent and **read-only** (this app never writes raw EC memory):

```sh
# /etc/modprobe.d/msi-ec-ec_sys.conf
options ec_sys write_support=0
```

`write_support=1` is only needed by tools that write fan curves straight to the
EC. If you already use such a tool, keep whatever you have — the app only reads,
and the permissions below still block group writes.

## 3. Permissions: group + udev

The sysfs attributes are `root:root` by default. Create a group, add yourself,
and hand that group write access:

```sh
sudo groupadd -r msi-ec
sudo usermod -aG msi-ec "$USER"      # log out / back in afterwards
id -nG | grep msi-ec
```

Install the two files from [`packaging/`](packaging/):

```sh
sudo install -Dm644 packaging/99-msi-ec-permissions.rules /etc/udev/rules.d/99-msi-ec-permissions.rules
sudo install -Dm644 packaging/msi-ec-tmpfiles.conf       /etc/tmpfiles.d/msi-ec.conf
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=platform     --action=add
sudo udevadm trigger --subsystem-match=power_supply --action=add
sudo udevadm trigger --subsystem-match=leds         --action=add
```

Check the result:

```sh
$ ls -l /sys/devices/platform/msi-ec/shift_mode \
        /sys/class/power_supply/BAT1/charge_control_end_threshold \
        /sys/class/leds/msiacpi::kbd_backlight/brightness \
        /sys/kernel/debug/ec/ec0/io
-rw-rw-r-- 1 root msi-ec ... /sys/devices/platform/msi-ec/shift_mode
-rw-rw-r-- 1 root msi-ec ... /sys/class/power_supply/BAT1/charge_control_end_threshold
-rw-rw-r-- 1 root msi-ec ... /sys/class/leds/msiacpi::kbd_backlight/brightness
-rw-r----- 1 root msi-ec ... /sys/kernel/debug/ec/ec0/io
```

All four must be group `msi-ec` and group-writable (the last one is read-only on
purpose — that is all the RPM calculation needs).

> **Battery threshold still `root:root`?** On some boots the charge attributes
> appear after the udev event. Re-run the `udevadm trigger` lines above, or add
> to `/etc/tmpfiles.d/msi-ec.conf`:
> `m /sys/class/power_supply/BAT1/charge_control_end_threshold 0664 root msi-ec -`
> then `sudo systemd-tmpfiles --create /etc/tmpfiles.d/msi-ec.conf`.

## 4. Install the binary

### From a release tarball (recommended)

```sh
tar xf msi-center-0.1.0-x86_64-linux.tar.gz
cd msi-center-0.1.0-x86_64-linux
sudo install -Dm755 msi-center /usr/local/bin/msi-center
sudo install -Dm644 msi-center.desktop /usr/share/applications/msi-center.desktop
sudo install -Dm644 msi-center.svg /usr/share/icons/hicolor/scalable/apps/msi-center.svg
```

It then appears in your launcher as **MSI Center** (window class `msi-center`).

### From source with Docker

The repo's build environment is containerised; only `docker` is needed.

```sh
git clone https://github.com/tanmaypanda-14/msi-center-rust
cd msi-center-rust
make build          # image: Rust + Slint build deps
make run            # builds and launches on the host
```

### From source with plain Cargo

```sh
sudo apt install build-essential pkg-config libfontconfig1-dev libfreetype6-dev \
                 libxkbcommon-dev libwayland-dev      # Debian/Ubuntu names
cargo build --release
```

To build for **X11 instead of Wayland**, swap the backend feature in
`crates/app/Cargo.toml`:

```toml
slint = { version = "1", default-features = false, features = [
    "std", "backend-winit-x11", "renderer-software", "compat-1-2" ] }
```

(build with `--features` won't work — the backend is a feature of the Slint
dependency, so it has to be edited or exposed as a crate feature.)

## 5. Verify

```sh
$ msi-center --snapshot
backend      msi-ec
model        Modern 15 B7M
ec firmware  15HKEMS1.104
shift mode   Some(Comfort)
fan mode     Some(Advanced)
cooler boost false
cpu temp     Some(76.0)
fan rpm      Some(4173)
fan level    Some(100)
battery      Some(60)%
charge limit Some(60)%
ac online    true
kbd light    Some(1)/Some(3)
webcam block Some(false)
```

Then start the GUI with `msi-center` (or from the launcher). The window opens on
the Dashboard and refreshes once a second.

## 6. Autostart (optional)

Hyprland (`~/.config/hypr/autostart.conf` or wherever your config runs exec-once):

```
exec-once = msi-center
```

Generic XDG autostart (`~/.config/autostart/msi-center.desktop`):

```ini
[Desktop Entry]
Type=Application
Name=MSI Center
Exec=msi-center
X-GNOME-Autostart-enabled=true
```

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| `msi-ec is not available` banner / `backend mock` | module missing or unsupported EC | install/update `msi-ec`, check `dmesg | grep -i msi` |
| `fan rpm --` | `ec_sys` not loaded, or no read access to the debugfs image | `modprobe ec_sys`, check group perms in step 3 |
| `... is not writable - check the msi-ec udev rule` | attribute not group-writable | re-run the udev triggers; verify with `ls -l` |
| Shift/fan buttons do nothing, no error | driver too old for this firmware | update `msi-ec`, compare `available_*` |
| Window never appears | X11 session (release is Wayland-only) | use a Wayland session, or rebuild with the X11 backend |
| `error while loading shared libraries: libfontconfig.so.1` | runtime fonts missing | install `fontconfig` / `freetype2` |
| Charge limit resets after reboot | firmware or BIOS setting | the EC may override it; check BIOS battery settings |

## Other models

The app itself is model-agnostic: modes come from `available_shift_modes` /
`available_fan_modes`, so the buttons only appear for what the EC reports.

**Fan RPM is the exception.** `480000 / ticks` read from `0xCC/0xCD` is the
MS-15HK layout. On a different board the same registers may hold something
else. If RPM looks wrong, delete the raw EC read (`MsiEcBackend::fan_rpm`) or
open an issue with your model and `msi-ec` registers.

## Uninstall

```sh
sudo rm /usr/local/bin/msi-center \
        /usr/share/applications/msi-center.desktop \
        /usr/share/icons/hicolor/scalable/apps/msi-center.svg
sudo rm /etc/udev/rules.d/99-msi-ec-permissions.rules /etc/tmpfiles.d/msi-ec.conf
# optional, if nothing else uses them:
sudo rm /etc/modules-load.d/msi-ec.conf /etc/modprobe.d/msi-ec-ec_sys.conf
sudo dkms remove msi_ec/0.13 --all
sudo groupdel msi-ec
```
