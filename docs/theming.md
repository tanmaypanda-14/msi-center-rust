# Theming

The window follows the active desktop theme. On Omarchy (or any desktop that
keeps Omarchy's theme layout) it reads the system palette at startup and maps
it onto a small set of semantic design tokens; otherwise it draws with the
built-in palette. There is no theme editor in the app and no theme file of its
own.

## Where the theme comes from

Resolution order, first match wins:

1. `MSI_CENTER_THEME` — a theme **name** (e.g. `flexoki-light`), looked up in
   `~/.config/omarchy/themes/<name>` and then `$OMARCHY_PATH/themes/<name>`.
2. `MSI_CENTER_THEME_DIR` — a **directory** containing `colors.toml`, for other
   desktops and for tests.
3. `$HOME/.local/state/omarchy/current/theme` — where Omarchy keeps the applied
   theme (`theme.name` sits beside it). Omarchy hardcodes this path and ignores
   `XDG_STATE_HOME`, so the app does too.
4. The built-in palette — used when nothing above can be read. A missing
   directory is silent (the machine just is not running Omarchy); an unreadable
   `colors.toml` prints one line on stderr.

The theme's display name comes from `theme.name` beside `current/theme`,
falling back to the directory name.

To see what the app resolved:

```sh
msi-center --theme
```

It prints the theme name and mode, where it came from, the font, and the final
token values — which makes it the first thing to check when a colour looks
wrong.

## What is read

| Input | Use |
| --- | --- |
| `colors.toml` | the palette; `background` is the only required key |
| `shell.toml` | `[font] base-size`, the root of the type scale |
| `theme.name` | the name shown in the UI and by `--theme` |

`colors.toml` is a flat `key = "#rrggbb"` document. The app parses the subset it
needs instead of linking a TOML implementation, and keeps unknown keys so a
future Omarchy release does not break it. Only `background` is mandatory; every
other token has a derivation. `shell.toml` is read for `[font] base-size`
(default `12`, clamped to `8`–`28`) and then ignored.

## Tokens

Slint pages never see a palette key. They read tokens from
`crates/app/ui/theme.slint`, and `crates/app/src/theme.rs` fills those from
`crates/theme`.

| Token | Palette source | Used for |
| --- | --- | --- |
| `bg` | `background` | window |
| `rail` | `dark_background`, else a darker `background` | navigation rail |
| `surface` | `lighter_background` (dark themes), else a lift towards `foreground` | cards |
| `surface_alt` | `lighter_background`, else a larger lift | inputs, segmented controls, hover |
| `border` / `border-strong` | mixed from `background` towards `foreground` | hairlines, selected controls |
| `text` | `foreground` | body text |
| `text_bright` | `bright_foreground`, else `foreground` towards white | headings, big values |
| `muted` | `muted`, else `foreground` towards `background` | secondary text |
| `faint` | same source as `muted`, contrast-checked against `bg` | labels, chart axes |
| `accent` | `accent`, then `blue`, then `green` | selection, chart line, icons |
| `accent_hover` | `accent` towards `foreground` | hover |
| `accent_soft` | `selection` towards `background`, else `accent` mixed in | selected backgrounds |
| `on_accent` | whichever of `bg`/`text_bright` reads better on `accent` | glyphs on accent |
| `ok` / `warn` / `danger` / `info` | `green` / `orange` then `yellow` / `red` / `blue` then `cyan` | status |

Two rules matter here:

* **Elevation is derived, not hardcoded.** Surfaces are mixed from
  `background` towards `foreground`, so the same ramp works for a near-black
  theme and for a light one. Light themes keep the declared
  `lighter_background` out of the surface mix, because on a light palette
  "lighter" is the wrong direction.
* **Readability is enforced.** `muted`, `faint`, `ok` and `warn` are pushed
  towards the readable foreground until they reach a minimum contrast against
  the surface they sit on. Themes with low-contrast comments still produce a
  usable UI.

Typography follows `[font] base-size`: the type scale is scaled by
`base_size / 12` and clamped to `0.9`–`1.4`, so an oversized desktop font grows
the window's text without reflowing the layout into nonsense. The family
defaults to `Adwaita Sans`; `MSI_CENTER_FONT` overrides it (for example
`MSI_CENTER_FONT="JetBrainsMono Nerd Font"`).

## Environment

| Variable | Effect |
| --- | --- |
| `MSI_CENTER_THEME` | draw with this named theme instead of the active one |
| `MSI_CENTER_THEME_DIR` | read `colors.toml` from this directory |
| `MSI_CENTER_FONT` | override the font family |
| `OMARCHY_PATH` | where stock themes live (default `/usr/share/omarchy`) |

```sh
msi-center --theme                                  # what would be used now
MSI_CENTER_THEME=flexoki-light msi-center           # preview a light theme
MSI_CENTER_THEME_DIR=/tmp/palette msi-center        # bring your own colors.toml
```

## Startup only

The theme is read once, before the window is shown. Changing the theme while
the app is running does not repaint it: restart the app to pick up the new
palette. This keeps the window free of file watchers and inotify handles; the
app polls the EC once a second and does nothing else in the background.

If you want theme switches to apply without relaunching by hand, Omarchy's
theme hook can restart the app for you (this is an Omarchy-side hook, the app
does not install it). Drop an executable script into the hook directory:

```sh
#!/bin/bash
# ~/.config/omarchy/hooks/theme-set.d/msi-center.sh
pkill -x msi-center || true
```

The hook receives the new theme name as `$1`; Omarchy re-applies the theme
before running it, so the restarted app reads the new palette.

## Window opacity

Omarchy tags every window with its default opacity and blur rules. The app
paints an opaque background and does not override them, so on such a setup the
window is slightly translucent by design. To make it fully solid, add a rule in
`~/.config/hypr/hyprland.lua` that drops the opacity tag, using the app's
Wayland `app_id`, `msi-center`:

```lua
o.window("msi-center", { tag = "-default-opacity", opacity = "1 1" })
```

## Fallback palette

Without a readable theme the app uses the palette it shipped with before it
learned to follow the desktop: near-black background, white text, a red accent.
This is what `--theme` reports as `source built-in`, and it is covered by tests
so a broken theme cannot make the window unusable.
