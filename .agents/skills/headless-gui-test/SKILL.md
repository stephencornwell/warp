---
name: headless-gui-test
description: Run and drive the Warp GUI client (warp-oss) end-to-end on a headless Linux box with Xvfb + xdotool, including the focus/menu gotchas that make UI actions silently no-op. Use when manually verifying runtime GUI behavior of a Warp build without a physical display.
---

# Headless GUI testing of the Warp client

## Build & launch

```bash
cargo build --bin warp-oss --features gui
```

Xvfb (reuse an existing one if present — `ps aux | grep Xvfb`):

```bash
nohup Xvfb :99 -screen 0 1920x1080x24 -ac >/tmp/xvfb.log 2>&1 &
```

Launch with an isolated profile so you always test a fresh first-run state:

```bash
mkdir -p ~/wtest/{config,share,state}
DISPLAY=:99 XDG_RUNTIME_DIR=/run/user/$(id -u) \
LIBGL_ALWAYS_SOFTWARE=1 WGPU_BACKEND=gl \
XDG_CONFIG_HOME=~/wtest/config XDG_DATA_HOME=~/wtest/share XDG_STATE_HOME=~/wtest/state \
nohup ./target/debug/warp-oss >/tmp/warp.log 2>&1 &
```

The app log (much richer than stdout) lands at `$XDG_STATE_HOME/warp-oss/warp-oss.log`,
rotating to `warp-oss.log.old.0`. Every UI action logs a line like
`dispatching typed action: warp::...::SomeAction`. This is the single best oracle for
"did my click actually do anything" — if no action is logged, the input never reached
the app; if an action IS logged but the UI doesn't change, that's a real product bug.

## Critical gotcha: no window manager → keyboard input is dropped

There is usually no WM on these boxes (`wmctrl` fails with
"Cannot get window manager info properties", and `xdotool windowactivate` reports
`_NET_ACTIVE_WINDOW` unsupported). Without a WM nothing gives the window X input focus,
so **all keyboard shortcuts silently do nothing** while mouse clicks still work.
This produces very convincing false-negative bug reports (e.g. "Ctrl+Shift+D doesn't
split panes").

Always focus the window explicitly before any keystrokes, and re-focus after every
app restart (the window id changes):

```bash
WIN=$(DISPLAY=:99 xdotool search --class WarpOss | head -1)
DISPLAY=:99 xdotool windowfocus $WIN
```

Verify with `grep -c UserInsert $XDG_STATE_HOME/warp-oss/warp-oss.log` after typing.

Resize/move with `xdotool windowsize $WIN 1000 700` / `windowmove` (wmctrl won't work).

## Gotcha: screenshots are downscaled — never click on raw screenshot coordinates

Screenshots of a 1920x1080 screen are commonly delivered downscaled (e.g. 1568px wide,
factor ~0.8167). Coordinates read off the image are therefore NOT screen coordinates:

```
screen_x = image_x / 0.8167   screen_y = image_y / 0.8167
```

Skipping this conversion makes clicks land ~20% too high, which looks exactly like a
"menu hit-testing offset" or a dead menu item. Always convert, or avoid the problem
entirely (preferred, see below). Confirm the factor per run with
`identify shot.png` vs the real screen size.

## Prefer keyboard navigation for popup menus

The most reliable way to exercise a context menu item is to open the menu with a
right-click and then drive it with `Down` xN + `Return`, screenshotting after the
Downs to confirm which row is highlighted. This sidesteps all coordinate scaling.
Note menus are context-sensitive: the input menu shows Cut/Copy only when text is
selected, so the item index shifts — always verify the highlight before pressing Enter.

## Screenshots

`DISPLAY=:99 import -window root /path/shot.png` (ImageMagick) captures the whole
screen including in-app popups. Warp renders its menus inside its own window, so
`-window root` is enough; there is no separate popup X window.

## Verifying copy/paste for real (install xclip)

`sudo apt-get install -y xclip` works on these boxes and makes clipboard assertions
trustworthy instead of guesswork:

```bash
printf 'SENTINEL' | DISPLAY=:99 xclip -selection clipboard -i   # seed before a paste test
DISPLAY=:99 xclip -o -selection clipboard                        # read back after a copy
```

Seed a sentinel before every copy test so an empty/unchanged clipboard is
distinguishable from a successful copy. The X clipboard works fine under Xvfb, so a
failed paste is a real product bug, not an environment artifact. Test app→X (app Copy,
read with xclip) and X→app (seed with xclip, paste in app) separately — they can fail
independently.

## Useful keybindings (app/src/util/bindings.rs)

new tab `ctrl-shift-t`, command palette `ctrl-shift-p`, settings `ctrl-,`,
split right `ctrl-shift-d`, split down `ctrl-shift-e`, find `ctrl-shift-f`,
close session/pane `ctrl-shift-w`, copy `ctrl-shift-c`, paste `ctrl-shift-v`.

## Offline / no-network launch

```bash
nohup unshare -rn ./target/debug/warp-oss ... &
```
Works and is the cleanest way to prove the app needs no network. Note the process runs
in its own netns, and shader cache warnings about `/root/.cache` are harmless.

## Things that may be broken (re-check rather than assume)

Observed historically on the "plain local terminal" branch. Most were later fixed, but
they are the failure modes worth probing first on any build; verify against the current
code before blaming the environment:
- Closing a pane panicking with `circular view reference for view type warp::pane_group::PaneGroup`
  (root cause was a quit-scope helper re-entering the PaneGroup being closed).
- Terminal block/input right-click menus dispatching `BlockListContextMenu` yet rendering nothing.
- Tab "Close tab" / ✕ / `ctrl-shift-w` being no-ops.
- Command palette "actions" category returning `No Results Found` for every query
  (palette binding-source setup not wired up).
- A keyboard shortcut dispatching its action but having no effect while the equivalent
  context-menu item works — e.g. `ctrl-shift-v` logging `EditorAction::Paste` and
  inserting nothing while menu Paste inserts the same clipboard correctly. Always test
  both the keyboard and menu path for copy/paste; they go through different code.
- Command palette still listing surfaces that were supposedly deleted (notebooks, team
  workflows, Sentry panic triggers). Search the palette for `notebook`, `workflow`,
  `drive`, `ai`, `account`, `sentry` and invoke any hit to see whether it no-ops.
  Palette entries and the Settings → Keyboard shortcuts list are both fed by the
  `EditableBinding::new(...)` registry in `app/src/terminal/view/init.rs`, so when a
  removed surface still shows up there, grep that file for the human-readable label
  (note the label casing differs from the palette's title-cased rendering, e.g.
  "Toggle team workflows modal" in source vs "Toggle Team Workflows Modal" in the UI —
  grep case-insensitively or you will wrongly conclude the string is already gone).
- `Open Settings: <Section>` palette deep-links surviving for deleted settings pages.
  These come from the `SettingsNavItem::Page(SettingsSection::...)` list in
  `app/src/settings_view/mod.rs` (~line 999), which is separate from the
  `settings_pages` vec that builds the visible sidebar — so a section can be removed
  from the sidebar yet still be deep-linkable, opening a blank content pane. Sweep by
  typing `open settings` in the palette and comparing the results against the sidebar;
  a working deep-link highlights its sidebar row, a dead one renders an empty pane.
- Settings keybinding search uses fuzzy subsequence matching rather than literal
  substring matching. A query such as `team` can match unrelated labels, so inspect
  the actual row labels before treating a search hit as a deleted-surface leftover.
Confirm each by pairing a screenshot with the dispatched-action log line.

## Devin Secrets Needed

None — the OSS local-terminal build requires no login.
