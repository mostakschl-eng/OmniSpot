# OmniSpot

**Stop hunting. Start teleporting.**

![Windows Support](https://img.shields.io/badge/Windows-10%2020H1+%20/%2011-blue)

I spend my workdays on macOS and my evenings on Windows. After using [rCmd](https://lowtechguys.com/rcmd/) on Mac, I stopped searching for windows and started using muscle memory to jump to them.

Back on Windows, Alt+Tab felt slow and unpredictable for the same workflow, so I built OmniSpot.

[**Download Latest Release**](https://app-switcher.com/) | [**Portable (.zip)**](https://github.com/ipatalas/app-switcher/releases)

<details>
<summary>▶ See it in action</summary>
<video src="https://github.com/user-attachments/assets/5f5708a0-c384-4bd3-894e-73f334be37f6" controls width="100%"></video>
</details>

---

## 🚀 Why use this?

Alt+Tab is MRU (Most Recently Used), so the order keeps shifting. You have to look, scan, and tap.

OmniSpot uses static hotkeys:

- `Apps+C` -> always Chrome
- `Apps+V` -> always VS Code
- `Apps+T` -> always Terminal

You stop thinking about where a window is and just jump there.

## ✨ Key features

- [**Three Intelligent Cycle Modes:**](https://app-switcher.com/docs/configuration/cycle-modes/)
    * `NextApp`: Cycle between different apps assigned to the same key.
    * `NextWindow`: Cycle through all open windows of a *single* app (great for multi-instance browsers).
    * `Hide`: Minimize the app if you press the hotkey while it's already focused (the ultimate "toggle").
- **Start if not running:** optional per-app setting to start the app when no matching process is running.
- **Packaged app support:** works with modern Windows packaged apps (for example Windows Terminal) in addition to classic desktop apps.
- [**Peek mode:**](https://app-switcher.com/docs/configuration/peek-mode/) hold the hotkey to peek at the target app and release to return to the original app.
- **Lightweight desktop app:** native C#/.NET app with a tray-first workflow.

## 🔒 Power user ethics

OmniSpot uses a system-wide keyboard hook to detect your hotkeys. That is exactly why transparency matters.

- **Open source:** review the code and verify behavior yourself.
- **100% offline:** no telemetry, no auto-updates, no backend service.
- **No admin required by default:** runs in user space.
- **Portable option:** use the ZIP build if you want a no-installer workflow.

> **Windows SmartScreen may appear on first launch.**
> As an independent release, OmniSpot has not built strong SmartScreen reputation yet. If prompted, click **More info** then **Run anyway**.

## 🗺️ Roadmap

- ~~**Peek mode:** show target app as long as hotkey is held down and then bring back the original app when released.~~
- **Custom key combinations:** allow more complex hotkeys, for example `Ctrl+Shift+T`.
- ~~**Dynamic bindings:** automatically assign letters based on app name~~
- **More modifiers:** add ~~Caps Lock~~ and function keys as optional modifiers.

---

## Slint migration

The Rust desktop preview lives in `desktop/`. Open Settings from the repository root with:

```powershell
cargo run --locked --manifest-path desktop/Cargo.toml --target-dir target/desktop -- --settings-preview
```

The preview uses an isolated `slint-preview` folder beside its executable. Use `-- --profile-dir PATH` to open a separate SQLite test profile. Use `-- --about`, `-- --spotlight`, `-- --overlay`, `-- --warning`, or `-- --tray` to exercise those surfaces independently.

Launching the executable normally starts the native background host. The tray, global hotkeys, and stats collection follow the profile’s settings.

```powershell
cargo run --locked --manifest-path desktop/Cargo.toml --target-dir target/desktop -- --run
```

The host uses `%APPDATA%\OmniSpot` or the directory beside the executable when there is a `.portable` marker. Use `--run --profile-dir PATH` for an isolated host profile. The native host imports legacy LiteDB settings and usage history on first use and leaves the legacy files in place. Use `--migrate-profile PATH` to perform the import without starting the tray or keyboard hook. Exit other OmniSpot instances before running either command.

Build the standalone Windows installer with `powershell -ExecutionPolicy Bypass -File .\build-installer.ps1 -Slint -Version 1.0.2`. It packages the Rust executable and requires no .NET runtime.

Use the current OmniSpot WPF app as the visual and behavioral reference. Measure the existing screens at the same Windows theme, accent, DPI, and window size. Reuse the theme colors, typography, icons, spacing, and control states across Slint screens.

| Step | WPF files | Slint and Rust work |
|---|---|---|
| 1 | `App.xaml`, `About.xaml` | Shared theme and card styling, app identity, support links, and diagnostics. |
| 2 | `Settings.xaml`, `General.xaml`, `ModifierKeySelector.xaml` | Settings navigation, Windows backdrop, theme, modifiers, startup, and behavior toggles. Wire Reset and Save & Apply to Rust using a temporary SQLite profile for testing. |
| 3 | `Hotkeys.xaml`, `AddApplicationFlyout.xaml`, `KeyAssignmentButton.xaml`, `CycleModeSelector.xaml` | App lists, key capture, conflict indicators, cycle modes, and running or installed app selection. Check keyboard-only use and save/reopen behavior. |
| 4 | `Overlay.xaml`, `AppOverlayWindow.xaml`, `WarningOverlayWindow.xaml` | Preview, timing, app groups, numbered windows, and activation. Verify cursor-monitor placement, transparency, topmost state, and focus behavior. |
| 5 | `Stats.xaml`, `BentoDashboard.xaml` | Port the dashboard, empty and warmup states, filters, and reset dialog. Verify live counts and history against fixture databases. |
| 6 | `SpotlightSearch.xaml`, `SpotlightWindow.xaml` | Port search settings, the floating search window, results and icons. Verify focus, keyboard navigation, Escape, and launch. |
| 7 | `MainWindow.xaml` | Port the tray menu and Windows host. Exercise the global hook, startup, shutdown, portable mode, database upgrades, and installer before removing WPF and .NET. |

Use one shared implementation for repeated controls. Compare screenshots side by side in light and dark at 100% and 150% DPI, including hover, focus, selected, and disabled states. Test settings with isolated data first. Keep WPF available until the Rust executable passes these checks and can upgrade existing installations.

## 💽 Installation and requirements

### Requirements

- Windows 10 version 20H1 (build 19041) or newer, or Windows 11
- [.NET 8 Desktop Runtime](https://dotnet.microsoft.com/en-us/download/dotnet/8.0) (or higher) for non-self-contained builds

### Choose a release

| Release | Requires .NET installed | Installer |
|---|---|---|
| **Installer** | Yes | Yes |
| **Installer self-contained** | No | Yes |
| **Portable** | Yes | No |
| **Portable self-contained** | No | No |

- **Installer** - recommended for most users. Runs setup and adds OmniSpot to Programs.
- **Installer self-contained** - bundles .NET runtime, larger download.
- **Portable** - ZIP only, no setup. Extract and run `OmniSpot.exe`.
- **Portable self-contained** - ZIP with bundled runtime, no separate dependency.

### Steps

1. Download from [app-switcher.com](https://app-switcher.com/) or [GitHub releases](https://github.com/ipatalas/app-switcher/releases).
2. Run the installer, or extract the ZIP archive.
3. Start `OmniSpot.exe`.

The app starts in the background. Open Settings from the system tray icon to configure hotkeys.

## ⌨️ Set up your first hotkey

1. Right-click the tray icon and open **Settings**.
2. Add a hotkey and choose the target app.
3. Pick a modifier and a letter key.
4. Optional: enable **Start if not running** for that app.

Tip: the `Menu/Apps` key is often an excellent modifier because it is usually unused as a shortcut modifier.

## ⌘ CLI commands

Run:

```shell
OmniSpot.exe <command>
```

Available commands:

| Command               | Description                                                            |
|-----------------------|------------------------------------------------------------------------|
| `--log-all-windows`   | Log all windows to the log file, useful when troubleshooting           |
| `--enable-auto-start` | Enable application auto start on system boot                           |
| `--debug`             | Enable debug logging, useful for troubleshooting, do not use otherwise |
| `--trace`             | Enable trace logging, useful for troubleshooting, do not use otherwise |
| `--help`              | Show the help message with available commands                          |

## 🛠 Technical notes and known issues

- Configured modifier down/up events are suppressed to avoid side effects in foreground apps.
- Pressing modifier alone (without letter key) still works as usual. For example, `Apps` alone can still open context menu.
- If you assign common shortcuts (for example `Ctrl+V`) as OmniSpot hotkeys, those combinations will be intercepted system-wide and will not reach the foreground app.
- Complex shortcuts may conflict. Example: if `Ctrl` is your configured modifier, an app shortcut like `Ctrl+Shift+T` may fail because `Ctrl` is suppressed before the full combo reaches the app.
- Elevated windows limitation (UIPI): when an administrator app is focused, OmniSpot hotkeys will not be intercepted unless OmniSpot is also run as Administrator.

Inspired by [rCmd](https://apps.apple.com/app/rcmd-app-switcher/id1596483192) for macOS.
