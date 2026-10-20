# msi-ec driver → Modern 15 B7M (MS-15HK)

Source: [BeardOverflow/msi-ec](https://github.com/BeardOverflow/msi-ec) at
`d7fbbd8` (0.13 + post-release commits), read against the EC on this laptop
(`15HKEMS1.104`, release date 2022-10-24).

Only the configuration that this board actually picks is documented here. No
driver code is copied into this project — the app talks to the documented sysfs
ABI only. The driver is GPL-2.0; msi-center is MIT.

## 1. Which configuration this laptop gets

`load_configuration()` reads the EC firmware string at `0xA0` (12 bytes) and
does an **exact** match against every `ALLOWED_FW_*` list. The first list that
contains the string wins.

`15HKEMS1.104` is in `ALLOWED_FW_G1_6`, together with
`15HKEMS1.102`, `15HKEMS1.500`, `15HKEMS1.501` → `CONF_G1_6`.

```
grep -n "15HKEMS1" msi-ec.c
# 528:  "15HKEMS1.102", // Modern 15 B7M
# 529:  "15HKEMS1.104",
# 530:  "15HKEMS1.500",
# 531:  "15HKEMS1.501",
```

**Failure mode:** any other firmware string (after a BIOS update, or on a
different B7M revision) makes `modprobe msi_ec` fail with `-EOPNOTSUPP`; there is
no `/sys/devices/platform/msi-ec` at all, and msi-center falls back to the mock
backend. Newer builds accept `debug=1` / `firmware=...` module parameters to
inspect an unknown EC, but the installed 0.13 has no parameters.

## 2. Register map for MS-15HK

Every row was verified against the live EC via `/sys/kernel/debug/ec/ec0/io`
(the "live" column is one snapshot; values move).

| Function | EC addr | Encoding | sysfs attribute | Live |
| --- | --- | --- | --- | --- |
| Shift mode | `0xF2` | `0xC2` eco, `0xC1` comfort, `0xC0` sport | `shift_mode` | `0xC1` |
| Fan mode | `0xF4` | `0x0D` auto, `0x1D` silent, `0x8D` advanced | `fan_mode` | `0x8D` |
| Cooler Boost | `0x98` | bit 7 | `cooler_boost` | `0x06` (off) |
| Fn/Win swap | `0xBF` | bit 4, `invert = true` | `fn_key`, `win_key` | `0x08` (Win left) |
| Webcam | `0x2E` | bit 1 | `webcam` | — |
| Webcam block | `0x2F` | bit 1 | `webcam_block` | — |
| Charge limit | `0xEF` | bit 7 = valid flag, bits 0-6 = percent | `BAT1/charge_control_end_threshold` | `0xBC` (60 %) |
| CPU temperature | `0x68` | °C, 0-255 | `cpu/realtime_temperature` | `0x38` (56 °C) |
| CPU "fan speed" | `0x71` | **thermal level 0-100, not RPM** | `cpu/realtime_fan_speed` | `0x37` (55) |
| Keyboard backlight | `0xF3` | `0x80 \| level`, level 0-3 | `leds/msiacpi::kbd_backlight/brightness` | `0x81` (1) |
| Mute LED | `0x2C` | bit 2 | `leds/platform::mute` | — |
| Mic-mute LED | `0x2B` | bit 2 | `leds/platform::micmute` | — |
| Fan tachometer | `0xCC/0xCD` | big-endian ticks | *not exposed* | `0x008A` |
| Fan 1 curve temps | `0x6A..0x6F` | °C, 6 points | *not exposed* | 54, 58, 62, 66, 77, 93 |
| Fan 1 curve speeds | `0x72..0x78` | %, 7 points | *not exposed* | 30, 55, 70, 85, 100, 100, 100 |
| Fan 2 curve temps | `0x82..0x87` | °C, 6 points | *not exposed* | 50, 60, 70, 82, 90, 93 |
| Fan 2 curve speeds | `0x8A..0x90` | %, 7 points | *not exposed* | 45, 50, 65, 72, 80, 85, 100 |

Notes:

* The board has **one fan** (`gpu/` is empty, GPU config is `UNSUPP`), yet the
  EC keeps a complete second curve table. It is written by MSI Center and looks
  valid, but nothing reads it.
* `cpu/realtime_fan_speed` reads `0x71` raw. MSI's EC uses that byte as the
  current thermal/fan **level** (0-100). It is not RPM; msi-center shows it as
  "fan level" and computes RPM separately.
* Fan RPM is not implemented by the driver at all. `480000 / ticks` from
  `0xCC/0xCD` is MControlCenter's formula and matches the observed fan
  behaviour (e.g. 138 ticks → 3478 RPM).
* The charge *start* threshold is synthetic: `start = end - 10`, and writing
  `start` writes `end = start + 10` to the same `0xEF` byte. There is only one
  real threshold. This is why `BAT1` shows 50/60 on a 60 % limit.
* Keyboard backlight is a normal LED class device, but the EC byte stores
  `0x80 | level`; the driver handles the offset, so userspace writes 0-3.

## 3. What the driver exposes vs. what it doesn't

Exposed: shift mode, fan mode, cooler boost, webcam + block, Fn/Win swap,
battery charge limit, CPU temp, fan level, keyboard/mute/micmute LEDs, firmware
version/date.

Not exposed:

| Missing | Where it is | Consequence |
| --- | --- | --- |
| Fan RPM | tach at `0xCC/0xCD` | msi-center reads the raw EC (`ec_sys` debugfs, or `debug/ec_dump` on newer builds) |
| Fan curves | tables above | no curve editor without raw EC writes |
| Super Battery | `0xED` (bits 5/4/2) | **unusable on this board**: `CONF_G1_6` sets `.address = MSI_EC_ADDR_UNKNOWN`, and `MSI_EC_ADDR_UNKNOWN == MSI_EC_ADDR_UNSUPP == 0xff01`, so the attribute (present in newer builds) always fails |
| Turbo shift mode | — | not available: `available_shift_modes` is eco/comfort/sport only |

## 4. Installed 0.13 vs. upstream HEAD

Same version string (`0.13`), different feature set — worth knowing when
debugging a machine:

| Feature | Installed 0.13 | HEAD `d7fbbd8` |
| --- | --- | --- |
| `super_battery` attribute | no | yes (no-op on this board) |
| `debug/ec_dump`, `debug/ec_get`, `debug/ec_set` | no | yes |
| Module params `debug`, `firmware` | no | yes |
| `charge_control_*_threshold` on BAT1 | yes | yes |
| `fn_key` / `win_key` | yes | yes |

`debug/ec_dump` prints a 16×16 hex table; the app parses row `0xc_` and computes
RPM from bytes `0xc/0xd`, so a future driver update removes the `ec_sys`
dependency. `debug/ec_set` would make a fan-curve editor possible — as root, or
with a udev rule granting group write to that one file.

## 5. What msi-center does with this

| Feature | Implementation | Register |
| --- | --- | --- |
| Power modes | `shift_mode` | `0xF2` |
| Fan modes | `fan_mode` | `0xF4` |
| Cooler Boost | `cooler_boost` | `0x98` bit 7 |
| Fn/Win swap | `win_key` (write `left`/`right`) | `0xBF` bit 4 |
| Charge limit | `BAT1/charge_control_end_threshold` | `0xEF` |
| Temperature | `cpu/realtime_temperature` | `0x68` |
| Fan level | `cpu/realtime_fan_speed` | `0x71` |
| Fan RPM | `debug/ec_dump` → `ec_sys` fallback | `0xCC/0xCD` |
| Keyboard light | LED class | `0xF3` |
| Webcam block | `webcam_block` | `0x2F` bit 1 |

Still unused, deliberately: raw EC writes (fan curves, `ec_set`) need a
privileged helper; the app stays read-only on the EC image.

## 6. Reproducing the analysis

```sh
git clone https://github.com/BeardOverflow/msi-ec
grep -n "15HKEMS1" msi-ec.c            # board's firmware list
sed -n '518,600p' msi-ec.c             # CONF_G1_6

cat /sys/devices/platform/msi-ec/fw_version
for a in 0x68 0x71 0x98 0xbf 0xef 0xf2 0xf3 0xf4; do
  printf "%s = " "$a"
  dd if=/sys/kernel/debug/ec/ec0/io bs=1 skip=$((a)) count=1 2>/dev/null | od -An -tu1
done
```
