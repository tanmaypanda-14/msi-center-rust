# MSI Center 2.0.75.0 — analysis notes

Working notes. Every claim here is something observed in the shipped artifact,
not inferred from marketing copy. Evidence ids refer to REA sessions.

## Packaging chain

```
MSI Center_2.0.75.0.exe                  Inno Setup 5.4.2 (unicode), 590 MB
└── app/707c…127.appxbundle              ZIP64, 561 MiB
    └── MSI%20Center_2.0.75.0_x64.appx   single x64 package
        ├── DCv2/                        735 MB payload
        ├── WinMetadata/, resources.pri
        └── AppxManifest.xml
```

Inno payload also carries `SetAccessRight.exe`, `setup.ps1`, a licence XML and
an MPAP provisioning XML (AUMID registration).

## Platform

- Runtime: .NET Framework (`TargetFrameworkAttribute` = .NET Framework,
  runtime version 2.5 metadata header, CIL only, x86_64).
- UI: WPF (`GongSolutions.WPF.DragDrop`, `LiveCharts.Wpf`,
  `Microsoft.Expression.Drawing`), partially WebView2
  (`Microsoft.Web.WebView2.*` present).
- Local state: SQLite (`Microsoft.Data.Sqlite`, `SQLitePCLRaw`, `x64/SQLite.Interop.dll`).
- IPC: `NamedPipeClientLib.dll`.
- Auth/telemetry: `Microsoft.Identity.Client.dll`, `Newtonsoft.Json`.

## Binary census

| Category | Count | Notes |
| --- | --- | --- |
| Managed assemblies | 104 | app logic, UI, libs |
| Native DLLs | 31 | device SDKs + sqlite/WebView2 interop |

`DCv2.exe`: 194 types, 1,401 methods, 1,665 fields, 13.7 MB.
Main UI class `DCv2.MainWindow` has 251 methods — the app is a single large
window with feature pages, not a modular plugin host.

Config is a single struct: `DCv2.User_Config_Struct` (53 methods),
settings via `DCv2.Properties.Settings`.

## P/Invoke surface of DCv2.exe

Windows: `user32`, `kernel32`, `Shcore` (`GetDpiForMonitor`), `dwmapi`,
`wininet` (`InternetGetConnectedState`), `WritePrivateProfileString`/`GetPrivateProfileString`.

MSI SDKs:
- `Device\MsiHid_x64.dll` — HID device layer
- `Device\GM6070\IcMSIDll.dll` — keyboard/mouse EC

Observed native entry points (worth decompiling first):

| Symbol | Module | Meaning |
| --- | --- | --- |
| `NativeGetSpbInfo` | IcMSIDll | SPB (Simple Peripheral Bus) info |
| `NativeProcessDeviceConnectionChanged` | IcMSIDll | device attach/detach events |
| `NativeProcessInfoReport` | IcMSIDll | process reporting |
| `NativeMonitorInfoReport` | IcMSIDll | monitor info |
| `NativeIsDeviceConnect` | IcMSIDll | presence check |
| `NativeReturnUrlLightUp` | IcMSIDll | Mystic Light URL hook |

## Native modules needing Ghidra

```
API_LiveUpdate_x64.dll, CMSILedDll.dll, MSILedDll_MyDevice.dll
Device/MsiHid_x64.dll, MsiHid.dll, MsiGKbApi.dll, MsiGM08.dll, MsiHid.dll
Device/GM6070/{IcGM60,IcGM70ND,IcGM70NDRx32,IcGM70TI,IcMSIDll}.dll
Device/GM20Elite/PXICtrl.dll, PXICtrl.dll
Device/GH70/XearAudio.dll, NahimicAPI.dll, MSI_Touchpad.dll
gcapi_dll.dll
```

## Feature modules (`UC_*.dll`)

MSI Center is a shell that loads one UserControl module per feature page. 50 are
shipped; the ones relevant to a Modern-series laptop:

```
UC_NB_User Scenario      performance profiles / fan mode      <- core
UC_NB_Monitor            temperatures, fan rpm, load         <- core
UC_NB_System Info        model, BIOS, hardware summary
UC_NB_System Diagnosis   health checks
UC_NB_General Settings   battery charge limit, keyboard backlight, boot options
UC_Mystic Light          lighting (Modern = white backlight only)
UC_NB_Live Update        updater (we will not reimplement)
UC_NB_AI Engine          ML fan/tuning (skip)
UC_NB_Burn Recovery, UC_NB_System Restore, UC_NB_Creator Mode   (skip)
```

Desktop-only modules that are irrelevant to a laptop: `UC_Cooling Wizard`,
`UC_Cube Fan`, `UC_Graphics Fan Tool`, `UC_Coreliquid`, `UC_Power Supply Unit`,
`UC_MSI Router`, `UC_Duet`, `UC_Liberator`, `UC_Analysis`.

`UC_NB_Base%20Module.dll` contains the series table: `Modern 14/15`,
`Prestige 14/15`.

## How feature gating actually works

`DCv2.PlatformRules` is a pure data holder:

```
Date : i8          Data : string        IsCase : List<u4>
DefaultType : u4   TargetType : List<u4>  TargetCPU : List<u4>
```

There is no hardcoded model→feature table in the binary: the rules arrive as a
`Data` string (cached JSON, updated from MSI's service). So "which features does
this laptop get" is decided at runtime, not statically. The authoritative source
is therefore the machine itself — see `re/probe/probe.ps1`.

## Target machine

MSI Modern 15 B7M: Ryzen U-series, integrated graphics, white (not per-key RGB)
keyboard backlight. Expected surviving surface: monitoring, user scenario /
fan mode, battery charge limit, keyboard backlight on/off, system info.

## Scope decision (taken after this analysis)

Windows is not a target and no MSI code is reused. The result is a Linux app
with the same visual language, driving the in-tree `msi-ec` driver:

| MSI Center concept | Implementation |
| --- | --- |
| User Scenario (eco/comfort/sport) | `msi-ec/shift_mode` |
| Fan mode (auto/silent/advanced) | `msi-ec/fan_mode` |
| Cooler Boost | `msi-ec/cooler_boost` |
| CPU temperature | `msi-ec/cpu/realtime_temperature` |
| Fan RPM | EC ticks `0xCC/0xCD`, `480000 / ticks` |
| Battery charge limit | `BAT1/charge_control_end_threshold` |
| Keyboard backlight | `leds/msiacpi::kbd_backlight/brightness` |
| Webcam block | `msi-ec/webcam_block` |

`re/probe/probe.ps1` and the Ghidra track were dropped with the Windows target;
the surviving notes above are kept as provenance for the feature list.

## Open questions

1. ~~Exact WMI classes / ACPI method IDs~~ — resolved: the kernel driver abstracts the EC.
2. Mystic Light / per-key RGB — not present on this model.
3. Which privileged operations go through the named-pipe service — irrelevant on Linux; udev grants group access instead.
4. Whether the fan-curve feature is EC-side or service-side — EC-side; tables at `0x6A`/`0x72` and `0x82`/`0x8A` (MControlCenter layout). Not yet exposed in the UI.
