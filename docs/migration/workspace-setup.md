# Rust workspace setup and C# reference map

This is UI conversion work, not a replacement app. Build Rust from `D:\OmniSpot\rust` with `cargo +stable-x86_64-pc-windows-msvc build --locked`. Output stays in `rust/target/`. The current C# solution remains at `D:\OmniSpot\OmniSpot.sln` and builds independently. Run the Settings UI with `cargo +stable-x86_64-pc-windows-msvc run --locked`. Use `--ui-spotlight`, `--ui-overlay`, or `--ui-warning` after `--` to inspect those separate views. None registers hooks, creates a tray icon, writes settings or reads the C# databases yet.

## Rust package layout

Cargo allows an explicit binary path. This package uses `OmniSpot/App.rs` so the Rust source can mirror the C# `OmniSpot/` project. Module declarations use `#[path]` to keep the matching C# folder and file names, including their capitals. This is the destination map; untouched subsystem folders contain only `.gitkeep` until their first Rust source file is converted:

```text
rust/
  Cargo.toml                 package manifest
  Cargo.lock                 locked application dependencies
  OmniSpot/
    App.rs                   process entry; Cargo.toml points here
    CLI/                    argument registration, parsing and help ported; WindowDiscovery/logging integration pending
    Configuration/          future configuration and storage port
    Extensions/             add only if actual shared methods need it
    Input/                  future hook and key-state port
    Overlay/                five matching service/parser/content files ported; native app wiring pending
    Properties/             future Windows manifest/version resources
    Resources/              UI images and icons as ported
    Spotlight/              future search behavior port
    Startup/                AutoStart.rs shortcut path, create, remove and enable check ported; app wiring pending
    Stats/                  future statistics behavior port
    UI/
      Controls/             modifier selector, app controls, dashboard
      Pages/                General, Hotkeys, Overlay, Stats, About, SpotlightSearch
      ViewModels/           display state for settings, apps, overlay and search
      Windows/              Settings, AppOverlayWindow, WarningOverlayWindow, SpotlightWindow
    WindowDiscovery/        future Windows enumeration and activation port
  tests/                     cross-module behavior and migration checks
  target/                    ignored Cargo output
```

The C# `bin/`, `obj/`, and `Inno_Files/` are generated output, not source modules. Rust keeps generated build files under `rust/target/`; installer output will be mapped when packaging is ported. No C# output or portable data is copied into Rust.

For each source folder, preserve its name and each runtime file's stem in Rust. Combine a WPF `.xaml` file and its `.xaml.cs` code-behind into one `.rs` file with that stem. Do not make empty Rust copies of WPF-only designer files; `UI/ViewModels/DesignTime/README.md` explains that exception. Leave all original C# files in place until the replacement is complete and verified.

Keep unit tests next to their modules; use `tests/` for behavior crossing module boundaries. The Rust package must use its own data paths and must not read or create C# databases during early UI work. The explicit binary path is a supported [Cargo target setting](https://doc.rust-lang.org/cargo/reference/cargo-targets.html); Rust's [module path attribute](https://doc.rust-lang.org/reference/items/modules.html#the-path-attribute) lets these files keep the C# names. Separate workspace crates can be added later if a real boundary calls for them.

## Current app audit

| Current owner | Behavior to preserve | Rust destination when implemented |
|---|---|---|
| `OmniSpot/App.xaml.cs`, `ServicesConfiguration.cs` | Startup, portable selection, database setup, service lifetime, hook start, shutdown | `App.rs`, future `ServicesConfiguration.rs` and matching modules |
| `UI/Windows/MainWindow.*`, `UI/ViewModels/MainWindowViewModel.cs` | Hidden owner window, tray Settings/Exit and double-click | `UI/Windows/MainWindow.rs`, `UI/ViewModels/MainWindowViewModel.rs` |
| `UI/Windows/Settings.*`, `UI/ViewModels/Common/SettingsState.cs` | One Settings shell, shared editable state, validation, reset and Save & Apply | `UI/Windows/Settings.rs`, `UI/ViewModels/Common/SettingsState.rs` |
| `UI/Pages/General.*`, `Hotkeys.*`, `Overlay.*`, `Stats.*`, `About.*`, `SpotlightSearch.*` | Current navigation and page behavior | `OmniSpot/UI/Pages/` |
| `UI/Controls/*` | Modifier selector, app picker, cycle selector and dashboard | `UI/Controls/` |
| `Configuration/*` | Ordered app bindings, defaults, validation, LiteDB persistence and migrations | `Configuration/` |
| `Input/*`, `Switcher.cs`, `WindowDiscovery/*`, `Startup/*` | Hotkeys, peek, window actions and startup shortcut | `Input/`, `Switcher.rs`, `WindowDiscovery/`; `Startup/AutoStart.rs` ported but not wired |
| `Overlay/*`, `UI/Windows/*Overlay*` | Delayed app and warning overlays | Five matching `OmniSpot/Overlay/` file stems ported; `OmniSpot/UI/Windows/` and native event wiring pending |
| `Stats/*`, `Spotlight/*`, `CLI/*` | Stats, search, usage and command line | `Stats/`, `Spotlight/`; all four `CLI/` file stems ported, with WindowDiscovery and logging integration pending |

The screenshots show the Settings shell, General page and Applications page as visual references. They are not executable UI specifications; behavior and data rules come from the current source and tests.

## Safe source and data placement

Do not relocate `OmniSpot/` or `OmniSpot.Tests/` during this setup. `OmniSpot/bin/Debug` and `OmniSpot/bin/Release` currently contain `settings.db` and `stats.db`. The project's `.portable` marker is copied beside built executables, so those files may be live user data. Installed data is stored under `%APPDATA%\OmniSpot`; logs use `%LOCALAPPDATA%\OmniSpot\logs` or the portable executable directory. Neither Rust builds nor future archives should write there.

After the Rust startup/tray and Settings framework are proven, inventory source-only files and move them to `legacy/csharp/` without touching `bin`, `obj`, databases, journals, logs, `.git`, `.reference`, or the running executable. Verify the relocated C# solution still builds and tests. Keep a dated recovery copy outside the repository before any data migration. No Git staging, commits or pushes are part of this work.

## Next implementation slice

Confirm that the pinned GPUI Settings preview opens and closes reliably on this machine. Then add a tray icon and Settings reopen/Exit lifecycle, without registering global hotkeys or reading the C# databases. Port General settings after that gate passes.

### First GPUI check (2026-11-05)

- Pinned `gpui = 0.2.2` with Windows manifest and MSVC Rust toolchain. `cargo check --locked`, `cargo build --locked`, `cargo test --locked` and format check passed. The single test verifies that only the explicit preview flag opens the UI.
- The user confirmed the dark GPUI window appears on screen. The computer capture tool showed unrelated desktop content, so automated visual comparison remains unavailable. Do not claim pixel parity without user screenshots or another reliable capture.
- The UI conversion status for each C# XAML file is tracked in `ui-parity.md`. Interactions are local draft UI state only; no tray, settings storage, hooks, or window switching is connected. The old app and its data were not touched.
