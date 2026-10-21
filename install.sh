#!/usr/bin/env bash
# msi-center one-command installer for Arch Linux / Omarchy.
#
#   curl -fsSL https://github.com/tanmaypanda-14/msi-center-rust/releases/latest/download/install.sh | sudo bash
#   curl -fsSL .../install.sh | sudo bash -s -- --no-widget
#   curl -fsSL .../install.sh | bash -s -- --dry-run
#
# The installer downloads the latest release packages, verifies SHA256SUMS,
# installs the app and the DKMS driver source, adds the desktop user to the
# msi-ec group, removes older per-plugin copies of the permission files, and
# on Omarchy installs or updates the bar widget. It is idempotent.
#
# It never downloads and executes a second script: everything is in this file.
set -euo pipefail

REPO="tanmaypanda-14/msi-center-rust"
BASE="${MSI_INSTALL_BASE:-https://github.com/${REPO}/releases/latest/download}"
APP_ASSET="msi-center-x86_64.pkg.tar.zst"
DRIVER_ASSET="msi-ec-dkms-any.pkg.tar.zst"
WIDGET_REPO="https://github.com/tanmaypanda-14/omarchy-msi-power.git"
WIDGET_DIR_NAME="tanmay.msi-power"

with_widget="auto"
dry_run=0

usage() {
  cat <<'EOF'
Usage: install.sh [--with-widget | --no-widget] [--dry-run]

  --with-widget   install/update the Omarchy bar widget
                  (default: yes when Omarchy is detected)
  --no-widget     do not touch the Omarchy widget
  --dry-run       download and verify the packages, print the actions,
                  change nothing (can run as a normal user)
  -h, --help      show this help

Real install (root):

  curl -fsSL https://github.com/tanmaypanda-14/msi-center-rust/releases/latest/download/install.sh | sudo bash

With the widget explicitly:

  curl -fsSL .../install.sh | sudo bash -s -- --with-widget
EOF
}

for arg in "$@"; do
  case "$arg" in
    --with-widget) with_widget=1 ;;
    --no-widget)   with_widget=0 ;;
    --dry-run)     dry_run=1 ;;
    -h|--help)     usage; exit 0 ;;
    *) echo "install.sh: unknown option: $arg" >&2; usage >&2; exit 2 ;;
  esac
done

log()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

if [ "$dry_run" -eq 0 ] && [ "$(id -u)" -ne 0 ]; then
  die "run as root: curl -fsSL <install.sh URL> | sudo bash"
fi

[ "$(uname -s)" = Linux ] || die "Linux is required"
[ "$(uname -m)" = x86_64 ] || die "x86_64 is required (the app package is x86_64)"
command -v pacman >/dev/null 2>&1 || die "Arch Linux / Omarchy (pacman) is required"
command -v curl >/dev/null 2>&1 || die "curl is required"
command -v sha256sum >/dev/null 2>&1 || die "sha256sum (coreutils) is required"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

log "downloading the latest release"
for f in "$APP_ASSET" "$DRIVER_ASSET" SHA256SUMS; do
  curl -fL --proto '=https' --tlsv1.2 --retry 3 --connect-timeout 15 \
    -o "$tmp/$f" "$BASE/$f" || die "could not download $BASE/$f"
done

log "verifying checksums"
( cd "$tmp" && sha256sum -c --ignore-missing SHA256SUMS ) \
  || die "checksum verification failed"

app_ver=$(pacman -Qip "$tmp/$APP_ASSET" 2>/dev/null | awk -F': ' '/^Version/{print $2; exit}')
drv_ver=$(pacman -Qip "$tmp/$DRIVER_ASSET" 2>/dev/null | awk -F': ' '/^Version/{print $2; exit}')
log "msi-center ${app_ver:-unknown}, msi-ec-dkms ${drv_ver:-unknown}"

# Never silently downgrade a locally built/newer package.
app_installed=$(pacman -Q msi-center 2>/dev/null | awk '{print $2}' || true)
app_skip=0
if [ -n "$app_installed" ] && [ -n "$app_ver" ] && command -v vercmp >/dev/null 2>&1; then
  if [ "$(vercmp "$app_installed" "$app_ver")" -gt 0 ]; then
    warn "msi-center $app_installed is newer than the release; leaving it alone"
    app_skip=1
  fi
fi

driver_other=0
for p in msi-ec-dkms-git msi-ec-git; do
  if pacman -Qq "$p" >/dev/null 2>&1; then driver_other=1; fi
done

driver_skip=0
if [ "$driver_other" -eq 1 ]; then
  warn "another msi-ec driver package is installed; leaving it alone"
  driver_skip=1
elif pacman -Qq msi-ec-dkms >/dev/null 2>&1; then
  inst=$(pacman -Q msi-ec-dkms | awk '{print $2}')
  if [ -n "$drv_ver" ] && command -v vercmp >/dev/null 2>&1 \
       && [ "$(vercmp "$inst" "$drv_ver")" -gt 0 ]; then
    warn "msi-ec-dkms $inst is newer than the release; leaving it alone"
    driver_skip=1
  fi
fi

# The user who invoked sudo, not root.
target="${SUDO_USER:-}"
if [ -z "$target" ] || [ "$target" = root ]; then
  target="$(logname 2>/dev/null || true)"
fi
if [ -n "$target" ] && ! id "$target" >/dev/null 2>&1; then
  target=""
fi

if [ "$with_widget" = auto ]; then
  if [ -n "$target" ] && command -v omarchy >/dev/null 2>&1; then
    with_widget=1
  else
    with_widget=0
  fi
fi

if [ "$dry_run" -eq 1 ]; then
  log "dry run: no changes will be made"
  if [ "$app_skip" -eq 1 ]; then
    echo "  app:    leave installed msi-center $app_installed (newer)"
  else
    echo "  app:    install msi-center ${app_ver:-?} from $tmp/$APP_ASSET"
  fi
  if [ "$driver_skip" -eq 1 ]; then
    echo "  driver: leave the installed msi-ec driver"
  else
    echo "  driver: install msi-ec-dkms ${drv_ver:-?} (DKMS source at /usr/src/msi_ec-0.13)"
  fi
  echo "  user:   ${target:-<unknown>} -> msi-ec group"
  echo "  grant:  0710 debugfs directories, 0640 raw EC image, seven writable attributes"
  echo "  files:  remove older /etc copies that shadow the package"
  if [ "$with_widget" -eq 1 ]; then
    echo "  widget: install/update ~/.config/omarchy/plugins/$WIDGET_DIR_NAME"
  else
    echo "  widget: skip"
  fi
  exit 0
fi

log "installing dependencies"
pacman -S --needed --noconfirm fontconfig freetype2 >/dev/null \
  || die "could not install the app dependencies"
if ! pacman -Qq dkms >/dev/null 2>&1; then
  pacman -S --needed --noconfirm dkms >/dev/null || die "could not install dkms"
fi

if [ "$app_skip" -eq 1 ]; then
  log "msi-center is already newer; skipping the package"
else
  log "installing msi-center"
  pacman -U --noconfirm "$tmp/$APP_ASSET" || die "could not install msi-center"
fi

if [ "$driver_skip" -eq 1 ]; then
  log "leaving the installed msi-ec driver alone"
else
  if [ ! -d "/lib/modules/$(uname -r)/build" ]; then
    warn "kernel headers for $(uname -r) not found; DKMS may not build"
    warn "install the matching headers package, then: sudo dkms autoinstall"
  fi
  log "installing the msi-ec DKMS source"
  pacman -U --noconfirm \
    --overwrite '/usr/src/msi_ec-0.13/*' \
    --overwrite 'usr/src/msi_ec-0.13/*' \
    "$tmp/$DRIVER_ASSET" || die "could not install msi-ec-dkms"
  rm -f /usr/src/msi_ec-0.13/*.bak.* 2>/dev/null || true
fi

was_member=0
if [ -n "$target" ]; then
  if id -nG "$target" 2>/dev/null | tr ' ' '\n' | grep -qx msi-ec; then
    was_member=1
  fi
  log "adding $target to the msi-ec group"
  usermod -aG msi-ec "$target" || warn "could not add $target to msi-ec"
else
  warn "could not determine the desktop user; run: sudo usermod -aG msi-ec <user>"
fi

log "removing older per-plugin copies of the permission files"
# Widget standalone unit first, so it is not left enabled.
systemctl disable --now msi-power-perms.service 2>/dev/null || true
rm -f \
  /etc/udev/rules.d/90-msi-ec.rules \
  /etc/udev/rules.d/90-msi-power.rules \
  /etc/udev/rules.d/99-msi-ec-permissions.rules \
  /etc/tmpfiles.d/msi-ec.conf \
  /etc/tmpfiles.d/msi-power.conf \
  /etc/modprobe.d/msi-ec.conf \
  /etc/modprobe.d/msi-power.conf \
  /etc/modules-load.d/msi-ec.conf \
  /etc/modules-load.d/msi-power.conf \
  /etc/systemd/system/msi-ec-perms.service \
  /etc/systemd/system/msi-power-perms.service

log "applying the read-only raw EC grant"
systemctl daemon-reload 2>/dev/null || true
systemctl enable msi-ec-perms.service 2>/dev/null \
  || warn "could not enable msi-ec-perms.service"
systemd-tmpfiles --create /usr/lib/tmpfiles.d/msi-ec.conf 2>/dev/null \
  || warn "could not apply /usr/lib/tmpfiles.d/msi-ec.conf"
udevadm control --reload-rules 2>/dev/null || true
for p in /sys/devices/platform/msi-ec \
         /sys/class/leds/msiacpi::kbd_backlight \
         /sys/class/power_supply/BAT1; do
  [ -e "$p" ] && udevadm trigger --action=change "$p" 2>/dev/null || true
done
udevadm settle 2>/dev/null || true
systemctl restart msi-ec-perms.service 2>/dev/null || true

modprobe msi_ec 2>/dev/null || true
if [ -d /sys/devices/platform/msi-ec ]; then
  log "msi-ec is bound"
else
  warn "msi-ec did not bind; check dmesg (BIOS/EC firmware support)"
fi
if command -v dkms >/dev/null 2>&1; then
  drv_status=$(dkms status 2>/dev/null | grep '^msi_ec/' || true)
  if [ -n "$drv_status" ]; then
    log "DKMS: $drv_status"
  else
    warn "DKMS has no msi_ec module; check the kernel headers"
  fi
fi

if [ "$with_widget" -eq 1 ]; then
  if [ -z "$target" ] || [ "$target" = root ]; then
    warn "the widget needs a desktop user; skipping it"
  elif ! command -v git >/dev/null 2>&1; then
    warn "git is not installed; skipping the widget"
  else
    user_home=$(getent passwd "$target" | cut -d: -f6)
    plugin="$user_home/.config/omarchy/plugins/$WIDGET_DIR_NAME"
    as_user() {
      if command -v runuser >/dev/null 2>&1; then
        runuser -u "$target" -- env HOME="$user_home" "$@"
      else
        sudo -u "$target" -H "$@"
      fi
    }
    log "installing the Omarchy widget for $target"
    as_user mkdir -p "$(dirname "$plugin")" 2>/dev/null \
      || warn "could not create $(dirname "$plugin")"
    if [ -d "$plugin/.git" ]; then
      as_user git -C "$plugin" pull --ff-only \
        || warn "could not update $plugin; leaving the existing checkout"
    elif [ -e "$plugin" ]; then
      warn "$plugin exists but is not a git checkout; update it manually"
    else
      as_user git clone --depth 1 "$WIDGET_REPO" "$plugin" \
        || warn "could not clone the widget"
    fi
  fi
fi

if [ "$was_member" -eq 0 ]; then
  group_note="Log out and back in so your session joins the msi-ec group."
else
  group_note="Your session already has the msi-ec group."
fi

printf '\n\033[1;32mDone.\033[0m\n\n'
cat <<EOF
Installed:
  msi-center ${app_ver:-unknown}
  msi-ec-dkms ${drv_ver:-unknown} (DKMS source; the module is built per kernel)
  permissions: 0710 debugfs directories, 0640 raw EC image,
               write access to exactly the seven attributes the app uses

$group_note
EOF

if [ "$with_widget" -eq 1 ]; then
  cat <<EOF
Widget:
  ~/.config/omarchy/plugins/$WIDGET_DIR_NAME
  load it with: omarchy restart shell
EOF
fi

cat <<'EOF'

Verify:
  msi-center --snapshot
  stat -c '%a %U:%G' /sys/kernel/debug/ec/ec0/io

No maintenance is needed after a reboot or a module reload: the
msi-ec-perms.service unit and the ec_sys modprobe hook re-apply the grant.
EOF
