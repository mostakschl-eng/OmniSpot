# Rust workspace setup and C# reference map

This is an ongoing full C# to Rust application rewrite. The Rust GPUI app now includes its runtime, tray, settings, hooks, switching, overlays, Spotlight, statistics, JSON storage, CLI, and legacy-profile importer. Build and test from `D:\OmniSpot\rust` with the MSVC Rust toolchain; output stays in `rust/target/`. The C# solution remains at `D:\OmniSpot\OmniSpot.sln` as the behavior reference. The inventory below records the original destination mapping; current implementation and remaining parity are tracked in [`ui-parity.md`](ui-parity.md). `.reference` remains excluded.

## Rust package layout

Cargo allows an explicit binary path. This package uses `OmniSpot/App.rs` so the Rust source can mirror the C# `OmniSpot/` project. Module declarations use `#[path]` to keep the matching C# folder and file names, including their capitals. Rust modules may be grouped where that fits the language better:

```text
rust/
  Cargo.toml                 package manifest
  Cargo.lock                 locked application dependencies
  OmniSpot/
    App.rs                   process entry; Cargo.toml points here
    CLI/                    argument registration, parsing, help and dispatch
    Configuration/          configuration model, JSON storage, validation and migrations
    Extensions/             add only if actual shared methods need it
    Input/                  global keyboard hook, state machine and dynamic bindings
    Overlay/                warning and application overlay services
    Properties/             future Windows manifest/version resources
    Resources/              UI images and icons as ported
    Spotlight/              application/file search, usage ranking and persistence
    Startup/                per-user Startup shortcut creation and removal
    Stats/                  collection, JSON storage, v2 migration and dashboard metrics
    UI/
      Controls/             modifier selector, app controls, dashboard
      Pages/                General, Hotkeys, Overlay, Stats, About, SpotlightSearch
      ViewModels/           display state for settings, apps, overlay and search
      Windows/              Settings, AppOverlayWindow, WarningOverlayWindow, SpotlightWindow
    WindowDiscovery/        window enumeration, process/package discovery and app registry
  tests/                     cross-module behavior and migration checks
  target/                    ignored Cargo output
```

The C# `bin/`, `obj/`, and `Inno_Files/` are generated output, not source modules. Rust keeps generated build files under `rust/target/`; installer output will be mapped when packaging is ported. No C# output or portable data is copied into Rust.

For each source folder, preserve its name and each runtime file's stem in Rust. Combine a WPF `.xaml` file and its `.xaml.cs` code-behind into one `.rs` file with that stem. Do not make empty Rust copies of WPF-only designer files; `UI/ViewModels/DesignTime/README.md` explains that exception. Leave all original C# files in place until the replacement is complete and verified.

Keep unit tests next to their modules; use `tests/` for behavior crossing module boundaries. The Rust application uses its own JSON data files. C# LiteDB data is accessed only by the separate read-only exporter; ordinary Rust startup does not open the C# databases or invoke a managed process. The explicit binary path is a supported [Cargo target setting](https://doc.rust-lang.org/cargo/reference/cargo-targets.html); Rust's [module path attribute](https://doc.rust-lang.org/reference/items/modules.html#the-path-attribute) lets these files keep the C# names. Separate workspace crates can be added later if a real boundary calls for them.

## Current app audit

| Current owner | Behavior to preserve | Rust destination when implemented |
|---|---|---|
| `OmniSpot/App.xaml.cs`, `ServicesConfiguration.cs` | Startup, portable selection, database setup, service lifetime, hook start, shutdown | `App.rs`, configuration, stats, tray and lifecycle modules |
| `UI/Windows/MainWindow.*`, `UI/ViewModels/MainWindowViewModel.cs` | Hidden owner window, tray Settings/Exit and double-click | Rust tray and GPUI Settings lifecycle; live behavior still needs validation |
| `UI/Windows/Settings.*`, `UI/ViewModels/Common/SettingsState.cs` | One Settings shell, shared editable state, validation, reset and Save & Apply | `UI/Windows/Settings.rs`, `UI/ViewModels/Common/SettingsState.rs`; visual and interaction parity remains under review |
| `UI/Pages/General.*`, `Hotkeys.*`, `Overlay.*`, `Stats.*`, `About.*`, `SpotlightSearch.*` | Current navigation and page behavior | `OmniSpot/UI/Pages/` |
| `UI/Controls/*` | Modifier selector, app picker, cycle selector and dashboard | `UI/Controls/` |
| `Configuration/*` | Ordered app bindings, defaults, validation, LiteDB persistence and migrations | `Configuration/` JSON persistence and versioned settings migration |
| `Input/*`, `Switcher.cs`, `WindowDiscovery/*`, `Startup/*` | Hotkeys, peek, window actions and startup shortcut | `Input/`, `Switcher.rs`, `WindowDiscovery/`, and `Startup/AutoStart.rs`; behavior and live Windows checks remain |
| `Overlay/*`, `UI/Windows/*Overlay*` | Delayed app and warning overlays | `Overlay/` and `UI/Windows/`; focus, dismissal and timer behavior still need live validation |
| `Stats/*`, `Spotlight/*`, `CLI/*` | Stats, search, usage and command line | `Stats/` versioned JSON storage/migration, `Spotlight/`, and `CLI/`; exact UI and indexed-search validation remain |

The screenshots show the Settings shell, General page and Applications page as visual references. They are not executable UI specifications; behavior and data rules come from the current source and tests.

## Safe source and data placement

Do not relocate `OmniSpot/` or `OmniSpot.Tests/` during this setup. `OmniSpot/bin/Debug` and `OmniSpot/bin/Release` currently contain `settings.db` and `stats.db`. The project's `.portable` marker is copied beside built executables, so those files may be live user data. Installed data is stored under `%APPDATA%\OmniSpot`; logs use `%LOCALAPPDATA%\OmniSpot\logs` or the portable executable directory. Neither Rust builds nor future archives should write there.

After the Rust startup/tray and Settings framework are proven, inventory source-only files and move them to `legacy/csharp/` without touching `bin`, `obj`, databases, journals, logs, `.git`, `.reference`, or the running executable. Verify the relocated C# solution still builds and tests. Keep a dated recovery copy outside the repository before any data migration. No Git staging, commits or pushes are part of this work.

## Current status

- The Rust app is wired as the active implementation path; `.reference` remains excluded and C# sources remain available for comparison.
- MSVC `cargo fmt --check`, 59 Rust tests, strict Clippy with `-D warnings`, and a Release build have passed in the latest verification cycle. GPUI's transitive `proc-macro-error2` dependency still emits a future-compatibility notice.
- Remaining work includes exact settings/dashboard visual parity, shell icon parity, live Windows validation, installer/package validation, and review of remaining C# logic against Rust. See [`ui-parity.md`](ui-parity.md) for the current detailed ledger.

### First GPUI check (2026-11-05)

- Pinned `gpui = 0.2.2` with Windows manifest and MSVC Rust toolchain. `cargo check --locked`, `cargo build --locked`, `cargo test --locked` and format check passed. The single test verifies that only the explicit preview flag opens the UI.
- The user confirmed the dark GPUI window appears on screen. The computer capture tool showed unrelated desktop content, so automated visual comparison remains unavailable. Do not claim pixel parity without user screenshots or another reliable capture.
- At the initial UI preview stage, interactions were local draft state and the tray, settings storage, hooks and window switching were not connected. That historical status was superseded by the Rust runtime implementation; current gaps are listed above and in `ui-parity.md`.
