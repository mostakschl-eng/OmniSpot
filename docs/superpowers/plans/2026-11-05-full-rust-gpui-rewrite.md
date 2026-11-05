# OmniSpot Full Rust and GPUI Rewrite Implementation Plan

> **For agentic workers:** Use `superpowers:executing-plans` to implement this plan task by task. Use subagents only when explicitly authorized by the user or applicable instructions. Track implementation with the checkboxes below. Repository Git restrictions override any skill examples that stage or commit changes.

**Goal:** Replace OmniSpot's C# application with a Rust Windows desktop application, including its UI, Spotlight search, hotkeys, app switching, overlays, settings, tray, statistics, and distribution, while preserving existing user data and expected behavior.

**Architecture:** One Rust process with a GPUI presentation layer, platform-independent application logic, and isolated Windows API adapters. Background work communicates through bounded messages; neither rendering nor the keyboard callback performs search, database, or icon work. The old C# source becomes a buildable reference under `legacy/csharp/`, not a production runtime dependency.

**Tech stack:** Rust MSVC toolchain; pinned GPUI and its required platform crates; Microsoft `windows` bindings; `serde`/`serde_json`; transactional `redb` storage; a Windows-compatible tray integration; existing Inno Setup packaging adapted to Rust. Resolve exact mutually compatible dependency versions during Task 1 and lock them; this document does not invent an untested version combination.

**Spec:** Sections 1–10 of this document are the migration design and acceptance specification. Sections 11–15 are the execution plan and release checklist.

**Date:** 2026-11-05.

**Status:** Planning artifact. A dependency-free Rust package now exists under `rust/`; no C# source, installed app, or user database has moved.

**Layout update:** The three-crate layout and root `Cargo.toml` below are superseded by [`docs/migration/workspace-setup.md`](../../migration/workspace-setup.md). Use one Cargo package under `rust/OmniSpot/`, mirroring the C# source folders and file names. `rust/Cargo.toml` points directly to `OmniSpot/App.rs`, so no `src/` folder is required. Split modules file by file and add separate crates only if a tested boundary requires them. Task paths below name responsibilities, not files to create up front. The C# source remains in place during the Rust proof.

**Supersedes:** The earlier proposal to retain a C# UI with a Rust Spotlight backend. The user's final decision is a full Rust application, including the UI.

## Global constraints

- Follow root `AGENTS.md`. Never stage, commit, push, change remotes, or modify Git tracking. Do not use `git mv`, `git rm`, `git clean`, or history rewrites.
- Preserve other working changes. Take a fresh inventory before implementing; observations below are not proof of later repository state.
- Application code, UI, and ongoing production runtime must be Rust. No WPF/.NET process, WebView, Electron, HTML/JS frontend, localhost server, or C#–Rust bridge in the finished application.
- Windows API FFI is expected and does not require writing an application backend in C or C++.
- Keep existing features; do not use the rewrite to add AI, web search, plugins, sync, accounts, telemetry, or a whole-drive crawler.
- Keep offline operation, user-space operation by default, portable mode, and the tray-first workflow.
- Preserve the compact dark Spotlight list. The user rejected large tiles, white flashes, pale gray surfaces, and paths repeated throughout the result list.
- An OFF floating-overlay switch must actually prevent the overlay from appearing, including when a delayed show operation is already queued.
- Never overwrite, delete, or silently reset existing settings, usage history, or statistics during migration or recovery. Existing user-directed statistics reset/delete flows remain supported and require explicit selection of their data scope.
- Read reference projects for ideas and honor their licenses. Do not copy their entire architectures or assume their benchmarks apply to OmniSpot.
- A claim of improved speed requires before/after measurements. Passing unit tests does not establish UI correctness or GPU compatibility.

## Review focus

1. Keyboard suppression must remain paired across repeat, configuration changes, injected events, and shutdown; unrelated application shortcuts must remain usable.
2. An overlay disabled or hidden during pending work must never be resurrected by an old timer, catalog completion, or render callback.
3. Search results and icons from an older request must never overwrite a newer query or a later show/hide session.
4. Migration must preserve numeric WPF key mappings, app order, local dates, usage counters, and Unicode paths without duplicate imports.
5. A tray-first process must remain responsive with every window hidden, without rendering continuously or polling windows on a short timer.

Tests for these conditions belong to Tasks 4–10 and the final acceptance matrix, rather than being deferred to a generic final review.

## 1. Decisions and limits

### 1.1 Confirmed by the user

- Rewrite the complete app in Rust, one subsystem at a time.
- Use a Rust UI; GPUI is the selected framework direction.
- Retain the previous implementation under a clearly named C# reference folder.
- Optimize a normal Windows background utility, not a graphics-heavy application.
- Preserve the existing feature set and improve reliability and responsiveness.

### 1.2 Engineering decisions proposed by this plan

| Decision | Implementation choice | Reason and check |
|---|---|---|
| Initial production target | Windows 10 build 19041+ and Windows 11, x64 MSVC | Matches the existing OS floor; prove GPUI compatibility on the stated floor before advertising it. |
| Other CPU architectures | Assess x86 and ARM64 explicitly in Task 1 | Current C# profiles declare x86/x64; GPUI support must not be inferred from Rust target availability. No silent claim of x86 parity. |
| Rust workspace | `crates/omnispot-core`, `crates/omnispot-windows`, `crates/omnispot-app` | Three clear ownership boundaries without a crate for every feature. |
| Storage | `redb` transactions, versioned JSON-encoded domain records | Pure Rust storage, explicit schema evolution, one application-owned writer. Validate its pinned Windows build. |
| Legacy data | One-time versioned export from LiteDB; Rust imports that export | Avoid writing an untested LiteDB file parser or discarding history. See Section 7 for the temporary exporter exception. |
| GPU fallback | Test the pinned GPUI backend; never assume an automatic CPU fallback | WARP exists in Windows, but its existence does not prove GPUI uses it correctly. |
| Search scope | Existing app sources plus Windows Search indexed files | No custom filesystem index or Everything dependency in this release. |
| UI components | Start with GPUI primitives; add a component library only for a demonstrated need | Avoid importing a large editor-style component stack for a small launcher. |
| Background execution | GPUI background facilities plus dedicated Win32/COM workers where required | Avoid adding a second async runtime unless an actual dependency requires it. |

These are implementation defaults, not claims that architecture or hardware validation has already passed. A failed gate must produce a specific report and a Rust-only corrective proposal; it must not silently change scope or restore C# as the runtime.

### 1.3 Approaches considered

1. **Full Rust + GPUI, incremental port:** chosen. It supports the desired custom interface while retaining a coherent Rust codebase. Windows window behavior and renderer support require an early proof.
2. **Full Rust with another native Rust UI:** reserve only if the GPUI compatibility gate exposes an essential requirement that cannot be met with a small supported integration. Do not build two UI frameworks in parallel.
3. **Rust backend + existing WPF UI:** superseded by the user's final decision; not an implementation path for this plan.

The choice is based on OmniSpot's requirements. Discord's announced work does not establish that OmniSpot's window styles, hardware support, or performance will automatically work.

## 2. Current implementation and parity inventory

Paths in this table are current repository paths. After Task 2, their reference equivalents are under `legacy/csharp/`.

| Subsystem | Source of truth to inspect | Required Rust behavior |
|---|---|---|
| Startup and composition | `OmniSpot/App.xaml.cs`, `ServicesConfiguration.cs`, `UI/Windows/MainWindow.*` | One resident process; tray entry; startup errors; graceful shutdown; deliberate window lifecycle. |
| Configuration | `Configuration/Configuration.cs`, `ConfigurationManager.cs`, `ConfigurationService.cs`, `Storage/*`, `Migrations/*` | Same fields, app ordering, defaults, validation, packaged-app path repair, persistence, and live application of saved settings. |
| Keyboard transitions | `Input/KeyStateMachine.cs`, `Input/Hook.cs`, existing input tests | Configurable modifier; exact modifier conditions; paired key suppression; repeat handling; injected-event rules; unbound-key passthrough; Alt+Tab statistics. |
| App switching | `Switcher.cs`, `WindowDiscovery/*` | `NextApp`, `NextWindow`, `Hide`; shared-letter app groups; optional launch; minimized-window restore; packaged apps; indexed digit selection; activation and pulse-border behavior. |
| Peek | `Input/Peeker.cs`, tests | Current 400 ms arming threshold, previous-window return, restore target's minimized state, cancellation and elevation handling. |
| Dynamic bindings | `Input/DynamicModeService.cs`, tests | Existing automatic letter assignment and configured-app exclusions; stable behavior for ambiguous names. |
| Floating overlay | `Overlay/AppOverlayService.cs`, `OverlayShowTimer.cs`, `UI/Windows/AppOverlayWindow.*` | Delay, running/launchable/dynamic sections, active state, click actions, up to ten eligible window choices, cursor-relative positioning, keep-open behavior. |
| Warnings | `Overlay/WarningOverlayService.cs`, `WarningContent.cs`, `UI/Windows/WarningOverlayWindow.*` | Elevation and key-stealing warnings without stealing focus or retry loops. |
| Spotlight catalog | `Spotlight/AppSearchProvider.cs`, `WindowDiscovery/PackagedAppsService.cs` | Curated tools, configured/running apps, Start Menu shortcuts, launchable packaged apps, useful titles, cached metadata and refresh. |
| Spotlight queries | `Spotlight/SpotlightSearchService.cs`, `WindowsSearchFileProvider.cs` | App-first updates; later indexed files; matching, sorting, deduplication, result limits, cancellation, binary-noise exclusions. |
| Spotlight usage | `Spotlight/SpotlightUsageStore.cs`, `Stats/SessionStats.cs` | Existing successful-launch counts, capped ranking boost, frequent/recent suggestions and historical usage inputs. |
| Spotlight presentation | `UI/ViewModels/SpotlightViewModel.cs`, `UI/Windows/SpotlightWindow.*` | Compact dark list, footer path, keyboard and pointer navigation, open, close, focus, empty state, foreground behavior. |
| Settings UI | `UI/Pages/*`, `UI/ViewModels/Common/*`, `UI/Windows/Settings.*` | General, applications, overlay, Spotlight, statistics, About; add/remove/reorder apps, validation, Save & Apply, Reset, discard prompt. |
| Overlay preview | `Input/Hook.cs`, `App.xaml.cs`, `Settings.xaml.cs` | Toggle previews immediately; save persists; discard/reset restores saved state; no delayed resurrection. |
| Statistics | `Stats/*`, `Stats/Storage/*`, `Stats/Migrations/*`, `StatsSettingsViewModel.cs` | Switch/peek durations, static/dynamic usage, daily buckets, Alt+Tab counters, streaks/efficiency/records and existing reset/delete controls. |
| Startup shortcut | `Startup/AutoStart.cs` | Per-user Startup `.lnk`, correct target and working directory, no automatic elevation. |
| CLI | `CLI/*`, `resume.ps1`, `suspend.ps1` | Preserve actual registered commands and flags after inspection, including help/debug/trace, startup and window diagnostics; verify scripts rather than assuming their behavior. |
| Release | `build-installer.ps1`, `OmniSpot/installer.iss`, publish profiles | Installer, portable ZIP, app icon/version, mutex, upgrade/uninstall, user data exclusions, correct architecture. |

The current app version read during planning is 1.0.2. The project targets .NET 8 / Windows 19041 and declares x86/x64 runtime identifiers. Earlier work reported 436 passing tests; establish a fresh baseline rather than copying that count into a verification report.

Current CLI registrations are `--log-all-windows`, `--enable-auto-start`, `--debug`, `--trace`, and the handler's `--help`. Root `suspend.ps1`/`resume.ps1` use an externally installed Sysinternals `PsSuspend.exe` against the oldest OmniSpot process; they are developer helpers, not evidence of built-in pause/resume commands. Preserve/document the helpers without making PsSuspend a dependency of the Rust application.

### 2.1 Configuration fields to preserve

`Modifier`, ordered `Applications`, `PulseBorderEnabled`, `Theme`, `OverlayEnabled`, `OverlayShowDelayMs`, `OverlayKeepOpenWhileModifierHeld`, `PeekEnabled`, `DynamicModeEnabled`, `StatsEnabled`, `SpotlightEnabled`, `SpotlightModifier`.

Each configured application includes `Key`, `ProcessPath`, `CycleMode`, `StartIfNotRunning`, `Type`, and optional `Aumid`. Preserve existing constructor/storage defaults, including missing-field behavior; do not assume an absent bool always means false.

### 2.2 Search values to capture and port

- Current app limit: 25; indexed file request limit: 15; suggestions capped at 10.
- Current file search starts at query length 2; current managed query delay is 25 ms. Treat changes to these as measured design changes, not incidental consequences of porting.
- Current app catalog refresh threshold: 15 seconds with reuse of the existing completed snapshot while refreshing.
- Current usage boost: `min(successful_launch_count, 6) * 2`.
- Inspect and capture exact normalization, prefix/token/initials/subsequence scores and source boosts in golden fixtures before replacing them.
- `control` must show launchable Control Panel without package-family identifiers or a list of assembly DLLs. A single `c` must be able to find Control Panel.
- Keep Windows Search failure isolated from application results. Empty results and an unavailable index are different states in diagnostics.

## 3. GPUI and Windows compatibility gate

GPUI is a custom-rendered UI toolkit; it does not automatically provide the Windows widgets or WPF behavior being replaced. Its published README describes pre-1.0 API changes and Windows Win32/DirectWrite integration. Zed's Windows guidance lists DirectX 11 and vendor graphics drivers; that is useful evidence, not a certification of our pinned application build. [S1–S3]

Before relocating the source or implementing the whole product, build a disposable GPUI executable that proves:

- Hidden startup with a tray entry and working Windows message processing.
- A normal focusable settings window and a compact, borderless Spotlight window.
- A topmost overlay that can show without activating or taking keyboard focus.
- Keyboard/IME input, Unicode text, text selection, Enter/Escape, and repeat handling.
- Minimize/hide/reopen without terminating the process or leaving a visible taskbar item for overlays.
- Per-monitor DPI, negative monitor coordinates, work-area clamping, monitor removal and changes.
- Dark background from first creation through resize/show/hide; no transient white frame.
- A dependable HWND/lifecycle integration for native attributes; no reliance on undocumented object-layout casts.
- Rounded edges and optional Windows backdrop effects with a solid dark fallback when composition/blur is unavailable or disabled.
- Hidden windows cause no app-driven render loop, animation loop, or polling loop.
- Failure before UI creation can produce a Win32 error message and write a useful log without depending on GPUI.

Record results on Windows 10/11, an Intel/AMD integrated GPU, and a discrete GPU when available. Also test remote desktop, a VM, and Microsoft Basic Display Adapter/software-rendering scenarios when available. Distinguish **passed**, **failed**, and **not tested**.

WARP is a Windows CPU rasterizer, not proof of automatic GPUI fallback. Investigate supported selection on the pinned backend. A CPU fallback may be claimed only after testing both rendering correctness and resource usage. Do not maintain a large custom renderer fork as an unplanned side project. [S4]

**Gate outcome:** a dated `docs/migration/compatibility.md` with exact GPU/driver/OS/framework/architecture data and a supported target matrix. Continue on the proved target set. An essential failure that prevents the requested app requires a concrete Rust-only decision before the rest of the rewrite proceeds. Unavailable test hardware remains a documented release limitation; never label it tested.

## 4. Target source and build layout

```text
D:\OmniSpot\
  AGENTS.md                         # Rust entry points + unchanged Git restrictions
  Cargo.toml                       # Workspace, profiles, shared dependency policy
  Cargo.lock                       # Application dependency lock
  rust-toolchain.toml               # Exact working toolchain established by spike
  crates/
    AGENTS.md                      # Rust rules covering all workspace crates
    omnispot-core/
      src/{lib,model,config,input,switching,peek,overlay,search,stats,storage,migration}.rs
      tests/                       # Pure behavior, fixtures and storage integration
      benches/                     # Reproducible search/cancellation workload
    omnispot-windows/
      src/{lib,hook,windows,activation,packages,shortcuts,search,icons,tray,startup,instance,theme}.rs
      tests/                       # Opt-in OS integration tests; no surprise hooks
    omnispot-app/
      src/{main,bootstrap,controller,commands,workers}.rs
      src/ui/{mod,theme,spotlight,settings,applications,overlay,warnings,stats}.rs
      tests/                       # Controller lifecycle and stale-update tests
      resources/                   # Licensed icons and application manifest
  tests/fixtures/                  # Synthetic parity traces and migration bundles
  tools/legacy-export/              # Documented one-time LiteDB export workflow
  scripts/{build-rust,package-rust,verify-rust}.ps1
  packaging/{installer.iss,omnispot.manifest}
  build-installer.ps1               # Small Rust packaging entry point at cutover
  legacy/csharp/
    README.md                      # Original build commands and source mapping
    OmniSpot/
    OmniSpot.Tests/
    OmniSpot.sln
    OmniSpot.sln.DotSettings
    build-installer.ps1             # Legacy packaging script, relocated-path aware
    ...                            # Only inventoried supporting source/config files
  docs/migration/
    {parity,baseline,compatibility,ffi-and-threading,data-format,validation,performance,release}.md
```

Names above are ownership decisions, not a requirement to create empty files. Create modules as their implementation tasks need them; split larger modules along the same responsibility when warranted.

### 4.1 Relocation rules

- Task 0 records every relevant source file, existing change, resource, relative reference, and root helper script.
- Use filesystem operations for the approved source relocation; do not manipulate the Git index. The user reviews and stages the resulting working-tree changes.
- Move only the inventoried source/config/resource files. Do not recursively move the existing `OmniSpot` directory: its `bin` folders may contain running executables, live portable databases, and logs.
- Keep all data, existing output directories, and running binary paths intact. Leave `.git`, `.reference`, root rules, and unrelated work at the repository root.
- Preserve C# project/test relative references under the shared legacy parent. Verify resource paths and helper script working directories.
- Keep the old C# guidance available to legacy builds. Add Rust guidance without weakening any root Git rule.
- Do not delete the legacy source after the Rust release. It remains the rollback and behavior reference until the user separately requests removal.

## 5. Runtime boundaries and interfaces

### 5.1 Dependency direction

`omnispot-core` knows domain types and storage behavior; it does not import GPUI or Windows APIs. `omnispot-windows` translates OS objects to owned domain data. `omnispot-app` owns composition, message routing, and GPUI views. The core does not call UI functions or keep HWNDs alive by accident.

Use ordinary typed Rust calls and channels within the process. There is no C ABI between the app's Rust modules. Unsafe FFI is limited to OS/library boundaries.

### 5.2 Shared model contract

Define these names in `omnispot-core/src/model.rs` and use them consistently:

```rust
pub struct QueryId(pub u64);
pub struct SessionId(pub u64);
pub struct ConfigRevision(pub u64);
pub struct WindowId(pub isize); // opaque value, revalidated before an OS action
pub struct AppId(pub String);  // stable app identity; not a visible label
pub struct ResultId(pub String);
pub enum ResultKind { App, File }
pub enum LaunchTarget {
    Executable(std::path::PathBuf),
    Shortcut(std::path::PathBuf),
    PackagedApp { aumid: String },
    File(std::path::PathBuf),
    SettingsUri(String),
}
```

`SearchResult` contains a stable `ResultId`, title, display path, kind, typed launch target, icon identity/location and integer score. No GPU texture, borrowed COM pointer, or GPUI entity crosses this model boundary. Icon updates carry both result identity and session/query identity.

App identity includes target kind plus its stable target value: executable path, shortcut path, AUMID, or settings URI. Store verified legacy target aliases separately. Only consolidate shortcut/executable identities when target resolution proves equivalence; display-name equality alone is not proof. Preserve exact imported aliases rather than assuming Rust and .NET Unicode uppercasing produce identical keys.

`WindowId` is only an address-like handle value. Actions use `WindowIdentity` containing that value, owning PID/thread ID, process creation identity where obtainable, and a local window-generation counter invalidated by observed create/destroy events. Delayed activation, minimizing and peek restoration revalidate identity; if identity is unavailable or changed, cancel the delayed effect. Include same-handle reuse in another process and in the same process in tests; `IsWindow` alone is insufficient.

Represent modifiers and keys explicitly in core input types, including left/right variants, scan code and extended-key identity where needed. Legacy WPF enum integers are never cast to Windows virtual-key values.

### 5.3 Engine contract

Establish the exact owned DTO fields in Task 0's parity fixture/schema, then implement these interfaces in the named modules:

```rust
// core/search.rs: fast application search against an immutable catalog snapshot
SearchEngine::replace_catalog(&mut self, catalog: CatalogSnapshot);
SearchEngine::search_apps(&self, query: &str, limit: usize) -> Vec<SearchResult>;
SearchEngine::suggestions(&self, limit: usize) -> Vec<SearchResult>;
SearchEngine::record_successful_launch(&mut self, app: &AppId);

// core/input.rs: deterministic, clock-injected transition decisions
InputMachine::process(&mut self, event: KeyEvent, now: std::time::Instant) -> InputDecision;
InputMachine::reset(&mut self, reason: ResetReason) -> Vec<InputEffect>;

// core/overlay.rs: effective preview/saved state and delayed-show token checks
OverlayState::apply_saved(&mut self, config: OverlayConfig, revision: ConfigRevision);
OverlayState::preview(&mut self, enabled: bool);
OverlayState::discard_preview(&mut self);
OverlayState::may_show(&self, ticket: OverlayTicket, modifier_held: bool) -> bool;

// core/migration.rs: validate completely before beginning a transaction
validate_bundle(bytes: &[u8]) -> Result<MigrationBundle, MigrationError>;
import_bundle(store: &mut Store, bundle: MigrationBundle) -> Result<ImportReport, MigrationError>;
```

These are planning signatures; `CatalogSnapshot`, `InputDecision`, config DTOs and errors are defined in the owner tasks before their callers. They must not become a generic plugin framework.

`AppCommand` includes query submission, launch, apply/save/reset configuration, preview overlay, switch, peek lifecycle, show/hide settings, and exit. `AppEvent` includes app/file results tagged with `SessionId` + `QueryId`, icon readiness, config applied, operation failure, and worker stopped. Do not expose a queue for arbitrary closures.

### 5.4 Threads and cancellation

| Owner | Responsibilities | Prohibited work |
|---|---|---|
| GPUI thread | Visible state, focus, input editing, texture upload, paint | Blocking OS discovery, database calls, file queries, image decode |
| Keyboard thread with Win32 message loop | `WH_KEYBOARD_LL`, bounded transition/suppression decisions, injected-event tagging | COM discovery, window enumeration, disk I/O, waiting for the UI, synchronous logging |
| Action worker | Activation, switch and peek effects using owned commands | Owning GUI entities or waiting on search completion |
| Shell worker with explicit apartment | Shortcuts, packaged metadata and shell icon requests | Moving live COM objects to other threads |
| Search worker with explicit apartment | Windows Search COM/OLE DB query lifecycle, latest pending query | Unbounded query/thread spawning or UI mutation |
| Storage worker | Transactions, batched usage/statistics, flush/close | Doing storage inside the hook callback |

Choose and document the COM apartment required by each actual API; use RAII initialization/uninitialization on its owning worker. Do not assume all Shell/WinRT/OLE DB APIs have interchangeable threading rules.

Use bounded queues: action/control 64 commands, storage/statistics 1000 events, icon jobs 128, and one replaceable pending file-search request. Coalesce catalog/icon refresh work. Limits are starting engineering budgets and must be exercised under load.

The hook makes suppression decisions synchronously using its own state and a validated config snapshot. If an action cannot be queued, fail open before consuming its keydown. Once a keydown was suppressed, always handle its matching keyup coherently. Never drop a consumed keyboard action silently. Isolate any callback panic at the FFI boundary and continue the hook chain safely.

Use monotonically increasing query/session/config/timer generations. UI accepts an event only when both the query and show-session match. Cancelling a blocking COM query is best effort where the provider allows it; stale-result rejection and bounded pending work are mandatory regardless.

### 5.5 Background behavior

- On hidden idle, no active animation, continuous redraw request, 100 ms focus timer, or catalog rebuild loop.
- Refresh the catalog on warm-up, relevant configuration changes, detectable app changes, and a stale check on demand. A stale search keeps using the last complete snapshot while one refresh runs.
- Use appropriate Win32 events for foreground/window changes where needed; keep event callbacks lightweight.
- Remove timers, unhook keyboard events, release native handles and COM objects, stop workers and flush storage on exit. UI closure alone must not leak a resident process.

## 6. User experience specification

### 6.1 Spotlight

- Initial reference geometry: 540 × 380 logical pixels, fixed during query changes; adapt only to available work area and DPI. A smaller monitor must not put the footer or controls offscreen.
- Near-black base (`#191D24`), dark search field (`#242A33`), readable light text (`#F4F7FB`), subtle stroke, restrained accent selection.
- Compact rows: approximately 42 logical pixels, 24-pixel icons, app title left, short result kind right. Use the same vertical-list behavior for suggestions and results.
- The selected path lives in the footer and can be revealed/copied accessibly; long paths never force the window wider. Tooltips must not cover the main results continuously.
- Background is dark from native window creation, before the first GPUI frame. An opaque dark surface is the reliable base; optional DWM transparency/blur follows system settings and must not make text unreadable.
- Up/Down changes selection and scrolls it into view. Left/Right edits query text. Enter opens; Escape closes. IME composition Enter must not launch an item prematurely.
- Initial selection is the best result for a new query. Appending files preserves a still-existing selected result by `ResultId`, rather than trusting an old numeric index.
- Empty text shows suggestions. Pending work must not flash a false empty state. No matches, missing index and failed launch get clear, appropriately scoped feedback.
- Cache icons by stable app/path identity with invalidation; decode and extract off the UI thread; do not block first results on missing icons.
- Reopen on the cursor's monitor, clamp to the work area, acquire focus deliberately, and close on actual foreground loss without fighting the user's focus.

### 6.2 Floating app overlay and warnings

The floating switcher overlay is separate from Spotlight. Its effective enabled state is saved configuration plus a settings-window preview override. Preview OFF immediately cancels pending show tickets and hides the window. Reset/discard clears the override; Save & Apply persists it.

Check enablement and token validity at scheduling, effect delivery, and the final show. Test OFF before holding a modifier, OFF while held, OFF during a pending timer, release-before-expiry, and close/reopen settings after discard. Do not rely on a disabled settings control to enforce runtime behavior.

Keep no-activation overlays, cursor placement, clickable entries, correct digit labels, warning lifetimes and existing app sections. A closed or destroyed HWND must not be acted on solely because it was present in a previous snapshot.

### 6.3 Settings, tray and accessibility

Port every existing setting and validator before considering cosmetic expansion. Keep app ordering and add/remove/reassign controls; missing/invalid targets need actionable errors. Distinguish previewed from saved state. Closing with dirty edits offers save/discard/cancel with correct rollback behavior.

Keep tray menu Open Settings and Exit, double-click behavior, startup setting, About/version, and current statistics pages. Use the existing visual style as the content reference while implementing normal GPUI controls.

All main flows must work with a keyboard. Validate focus indicators, labels, selection announcements, high contrast/reduced motion, Unicode/Bengali names, and IME input on the pinned GPUI stack. Record screen-reader support explicitly; custom drawing does not imply accessibility support. Essential input/focus failures block release.

## 7. Data preservation and migration

### 7.1 New data paths

| Mode | State | Logs |
|---|---|---|
| Installed | `%APPDATA%\OmniSpot\rust\state.redb` | `%LOCALAPPDATA%\OmniSpot\rust\logs\` |
| Portable | `<exe-dir>\data\rust\state.redb` when `.portable` exists | `<exe-dir>\logs\rust\` |

Resolve paths through Windows known-folder APIs, not an assumed current working directory. Preserve `.portable` detection semantics. Reject an unwritable portable data location with a useful message rather than silently switching profiles.

The old `settings.db`, `stats.db`, associated log/journal files and legacy paths remain untouched. Never merge installed and portable profiles automatically.

### 7.2 Storage contract

Use one `redb` database with tables `meta`, `config`, `spotlight_usage`, `daily_stats`, and `app_metadata`. Store schema version in `meta`; define serialized record versions separately from crate versions. Config is one versioned record, daily statistics use ISO local-date keys, usage uses canonical app identity. Cache tables are reconstructible and cannot overwrite real settings/history.

Transactions atomically persist related changes. Failed settings writes leave both saved configuration and runtime settings in the previous known state; an explicit preview remains distinguishable. A corrupt/unsupported database is reported and preserved for recovery, not replaced with defaults.

Bound/batch normal usage writes outside the search path. Settings commits, explicit statistics deletion, migration commit and orderly shutdown flush must report failures. Define the short unflushed usage window in documentation; do not claim crash-proof delivery of events that have never been committed.

### 7.3 LiteDB migration bridge

Rust does not need to understand the proprietary details of the current LiteDB files. Build a temporary exporter using the existing LiteDB dependency under `tools/legacy-export/` or the archived legacy solution. This is a one-time migration tool, not part of the Rust backend, not launched on ordinary app startup, and not a .NET dependency of the new application.

The final app, UI, hooks, search and storage remain Rust. The exporter can be distributed separately as a self-contained migration utility for old installations; do not bundle the .NET runtime into the new Rust installer. Document this exception transparently. If a strict prohibition on even a temporary exporter is later required, resolve a separately verified migration approach before changing any existing data.

Export only after the old app has flushed and exited. Use a supported LiteDB read API, never a raw copy of a live database without its journal. Do not print private app paths or history in build logs. Preserve raw source databases and make a dated backup before any legacy migration that writes data.

`omnispot-migration-v1.json` contains:

- Format version, source app/storage versions, source profile identifier, export timestamp and integrity hash/provenance.
- Every configuration field and ordered app entry.
- Named WPF key values plus original integers, mapped through an explicit tested lookup to new key types.
- Spotlight launch counters and historical daily statistics, including static/dynamic per-app counters, duration totals, Alt+Tab totals, fastest-switch records and original local dates.
- Missing/unknown optional fields reported explicitly; unknown required fields or keys reject the import rather than dropping assignments.

The Rust `--import-legacy <file>` operation validates size limits, schema, enum values, totals and mappings before writing. Use a single transaction for config/history and the import marker. Mark imports by source identity + content digest, not merely a file path. Reimport is idempotent; conflicting imports into an initialized profile require an explicit user-selected replacement/export workflow, never additive counter duplication.

Define the digest over a canonical payload that excludes the digest field itself. Retain legacy usage target identities alongside the mapped `AppId` until mapping is validated, so changing identity normalization does not disconnect existing counts from their apps. Do not merge apps solely because they have the same display title.

A **pristine** Rust profile may contain seeded factory defaults but has no user-saved config revision, successful-launch history, statistics, or prior import marker. Initial import may replace those seeded defaults. An existing edited/used profile requires a separate backed-up replacement workflow; it is never treated as pristine merely because its configuration happens to equal defaults. Acquire exclusive profile ownership for import and reject a concurrently running Rust instance using the same profile. Do not start normal workers or seed usage before the first-run migration choice is resolved.

Before commit, compare record counts and counter totals. After commit, reopen the store and compare again. Rollback restores the entire prior transaction on failure. Keep the export bundle and original databases available for the user; do not delete them automatically.

## 8. Windows integration rules

- Use Microsoft's maintained Rust Windows bindings, enabling only needed API groups. Keep native resource ownership explicit through RAII wrappers. [S5]
- Hook callback handles `nCode < 0` by forwarding; do not query async key state inside the callback as a replacement for processing the event stream. A hook thread needs a message loop and can lose its hook if it blocks. [S6]
- Preserve injected input tagging and paired suppression. Avoid blindly releasing real physical modifier keys or replaying sequences recursively.
- Keep monotonic durations separate from local-calendar statistics. Reset pending peek/overlay work on config changes, session transitions and shutdown; recover safely from missed releases.
- Enumerate windows using the existing visibility/style/cloaking/process criteria; revalidate `WindowIdentity`, not only handle existence, before delayed effects. Cache process metadata without retaining invalid handles.
- Document normal Windows foreground restrictions and UIPI/elevated-window limitations. Provide current warnings; do not promise cross-elevation control without elevation.
- Launch through typed targets using Shell APIs. A search query or title must never become an interpolated command line. Correctly handle spaces, Unicode, `.lnk`, AUMIDs and settings URIs.
- Preserve system theme/accent detection and react to relevant Windows notifications without polling constantly. Enforce contrast for the user's dark launcher surface.
- Keep startup changes user-directed; a debug/preview run cannot modify the production Startup shortcut.
- Preserve `OmniSpotMutex` at production cutover so old/new production binaries cannot both install hooks. Development previews use a distinct instance/profile and hooks disabled until a deliberate smoke run with the other app exited.
- Process CLI diagnostics/import/help before graphics initialization when possible, so supported maintenance actions work on systems where a renderer cannot start.

## 9. Performance plan

Capture Release baselines before moving implementation. Use the same machine, data snapshots, power mode, visible UI geometry and query sequences for C# and Rust. Measure cold and warm paths separately. Include end-to-end paint, not just engine timing.

| Metric | Method | Proposed investigation target, not an unmeasured guarantee |
|---|---|---|
| Warm app matching/ranking | Fixed synthetic catalogs of 100, 1,000 and 5,000 entries; at least 500 sampled queries after warm-up | p95 <= 5 ms at 5,000 entries on the recorded development machine |
| Key input to visible app results | Timestamp input, accepted result update and actual presented frame | Warm p95 <= 50 ms; investigate any regression from baseline |
| Hotkey callback duration | Timed low-overhead instrumentation, isolated from logger writes | p99 < 1 ms on recorded machine; never wait for slow effects |
| Switch activation | Input accepted to foreground event, excluding app cold launch | No material regression versus baseline; report OS-denied cases separately |
| File results | Same Windows Search state, query set and indexing scope | Report separately from app results; no UI stall while waiting |
| Hidden idle CPU | 60-second settled run, process CPU time / wall time / logical CPU count | Average below 0.5% total-machine CPU and no avoidable periodic work |
| Hidden GPU rendering | Present/frame activity after hide; ETW/PresentMon when available | No application-driven repeated presents while all windows are hidden |
| Resource retention | Private bytes/handles/threads before and after 1,000 show/query/hide cycles | No sustained growth after caches settle; report GPU memory separately |

Use `c`, `control`, `control panel`, `chrome`, `vs`, `code`, Unicode/Bengali text, initialisms, multiword, no-match and backspace sequences. Include `c -> control -> c -> empty`, repeated identical queries, and a slow file provider completing out of order.

Remove or tune the current 25 ms query delay only with measured responsiveness and worker-load evidence. Keep app matching cheap; coalesce expensive file searches. Avoid prematurely adding a whole-drive index, lock-free data structures, a large runtime or micro-optimizations.

For comparison against the three references, use installed builds only when available and record their versions/settings/index providers. Never call OmniSpot universally fastest based on different scopes or a single query. Framework choice and background residency alone do not prove lower resource usage.

## 10. Rust rules to add during implementation

Create `crates/AGENTS.md`, root build instructions and a short architecture note with these policies:

- Exact tested Rust toolchain and GPUI revision/version; checked-in application `Cargo.lock`; reproducible `--locked` builds.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and meaningful unit/integration tests.
- No `unwrap`/`expect` for OS, persisted, user, or FFI inputs. Use typed errors and user-visible recovery where appropriate; tests may assert.
- No unsafe in domain/UI code unless an unavoidable boundary is demonstrated. Every unsafe block states validity, lifetime, ownership and thread assumptions.
- No panic/unwind across OS callbacks; no borrowed pointers surviving an API-defined lifetime; no COM object transferred across apartments accidentally.
- Dependency additions need a concrete use, license review, minimal features and Windows support; no wildcard dependencies or floating Git branch in production.
- No secret/key-event logging, telemetry, network fetching at app runtime, or private user-data fixtures.
- Pure decision logic receives injected clocks/events; OS tests opt in and do not install global hooks or change startup configuration during ordinary `cargo test`.
- Stop background owners predictably; bounded queues and explicit overflow semantics; no silent data loss or swallowed launch failure.
- Keep root Git prohibitions verbatim. Reviewable checkpoints replace any template instruction to make commits.

## 11. Ordered implementation tasks

Each task follows: add the named failing checks, observe the intended failure, implement its narrow deliverable, run the named checks, then update the parity/status record. No task includes a Git commit. Do not create every future module up front.

### Task 0 — Inventory, behavior fixtures and baseline

**Files:** create `docs/migration/parity.md`, `baseline.md`, `tests/fixtures/input-traces.json`, `search-catalog.json`, `search-queries.json`, `config-cases.json`, `stats-cases.json`; inspect sources in Section 2.

**Consumes:** current C# source, current user-approved UI, synthetic test data.

**Produces:** traceable feature checklist, explicit expected search orders/scores and input effects, baseline commands/results, source-relocation manifest. Fixtures include version and stable IDs; no real user paths/history.

- [ ] Record working tree state without changing it; identify live build/data/output directories and any unrelated work.
- [ ] Run the current Release build/tests and capture exact commands, exit status, toolchain and test count.
- [ ] Capture defaults, matching scores, duplicate rules, display paths and all CLI registrations directly from current source/tests.
- [ ] Add traces for left/right modifiers, unexpected modifier, repeat, synthetic input, no binding, digit switching and modifier release during a pending action.
- [ ] Add stats fixtures with expected totals and date boundaries; record old/new enum mapping requirements.
- [ ] Record visible UI and background baseline where automation permits; mark blocked observations explicitly.

**Acceptance:** every row of Section 2 links to an implementation task and at least one check. Expected outputs are recorded before being implemented in Rust.

### Task 1 — GPUI proof, toolchain and platform decision

**Files:** create root workspace/toolchain/lockfile, `crates/AGENTS.md`, minimal `omnispot-core`, `omnispot-windows`, `omnispot-app`, and `docs/migration/compatibility.md`. Spike views stay identifiable until replaced.

**Consumes:** Sections 3 and 6; target hardware matrix.

**Produces:** a working pinned GPUI build, verified window/handle strategy, proposed architecture support recorded as tested or untested.

- [ ] Verify MSVC tools, SDK, Cargo/Rust and candidate upstream GPUI packaging against official upstream documentation. Record exact dependency origin, license and required native DLLs.
- [ ] Add the smallest tray/hidden-window and focusable/no-activate window proof. Run it before adding business logic.
- [ ] Exercise all Section 3 checks, including overlay focus, Unicode input and dark first frame.
- [ ] Test x64 and inspect real x86/ARM64 dependency support; explicitly document any reduction from old profiles before release work proceeds.
- [ ] Measure hidden idle presentation activity and capture actual renderer/adapter details.
- [ ] Pin the working toolchain/dependencies and write the gate outcome. Do not proceed past an unresolved essential focus/window/renderer failure.

**Checks:** `cargo check --workspace --locked`; `cargo test --workspace --locked`; manual spike matrix. A successful desktop build alone is insufficient.

### Task 2 — Archive C# source without moving live data

**Files:** create `legacy/csharp/README.md`; relocate only the Task 0 source manifest; update relocated script working directories; retain root guidance and references.

**Consumes:** compatibility pass and source manifest.

**Produces:** buildable C# reference, unchanged live data and binary locations, clear root Rust entry points.

- [ ] Verify each resolved source/destination stays inside the intended repository paths and no source path is an output/database directory.
- [ ] Relocate inventoried source files using native filesystem operations. Preserve unrelated edits exactly; do not use Git tracking commands.
- [ ] Repair solution/test/project/resource relative paths and run the archived Release suite from its new location.
- [ ] Compare source inventory and data/output file inventory before/after. There must be no accidental data migration in this task.
- [ ] Document legacy build and rollback usage; retain licenses and attribution.

**Checks:** `dotnet build legacy/csharp/OmniSpot.sln -c Release`; execute the archived test runner with its actual generated path; review the source-only file map.

### Task 3 — Domain configuration and transactional Rust storage

**Files:** `omnispot-core/src/{model,config,storage}.rs`, `tests/config_storage.rs`, `omnispot-app/src/bootstrap.rs` for profile paths only.

**Consumes:** config/default/validation fixtures; typed model names in Section 5.

**Produces:** `AppConfig`, key/cycle/theme enums, `Store`, validated config revisions and versioned storage schema.

- [ ] Write `config_preserves_order_and_defaults`, `duplicate_bindings_follow_existing_rules`, `key_mapping_is_explicit`, `failed_save_keeps_previous_config`, `portable_and_installed_profiles_are_separate` and `unsupported_schema_is_preserved`.
- [ ] Implement validation and record serialization; no GPUI/Win32 objects in saved records.
- [ ] Implement `Store::open(path)`, `load_config`, transactional `save_config`, schema migration and orderly close with typed errors.
- [ ] Use temporary stores to test reopen, transaction rollback, corrupt input, failed write and missing optional fields.

**Checks:** `cargo test -p omnispot-core --locked --test config_storage`.

### Task 4 — Input state machine and hook adapter

**Files:** core `input.rs`, Windows `hook.rs`, core `tests/input_parity.rs`, opt-in Windows hook smoke tests.

**Consumes:** `AppConfig`, key DTOs, captured traces.

**Produces:** `InputMachine`, `InputDecision`, `InputEffect`, `ResetReason`; RAII `KeyboardHook` owner; typed switch/Spotlight commands.

- [ ] Port trace assertions for exact modifiers, repeat, keyup pairing, injected events, unbound replay and legacy Alt+Tab tracking.
- [ ] Add `reset_does_not_leave_phantom_modifiers`, `queue_full_fails_open_before_suppression`, `spotlight_space_repeat_toggles_once`, `shutdown_releases_hook_ownership` and key-release-after-config-change tests.
- [ ] Implement the pure transition machine, then the dedicated hook thread/message loop using a cheap immutable config snapshot.
- [ ] Keep suppression decisions in the callback and expensive effects outside it. Handle callback failure/overflow without trapping normal typing.
- [ ] Smoke-test native typing, Ctrl/Shift/Alt/Win combinations, Caps Lock/Apps modifiers, layout differences and elevated-app limits in an isolated run.

**Checks:** `cargo test -p omnispot-core --locked --test input_parity`; deliberate OS smoke with legacy hooks stopped. Test processes must not hook the user's keyboard by default.

### Task 5 — Discovery, launch, window activation and dynamic bindings

**Files:** Windows `windows.rs`, `activation.rs`, `packages.rs`, `shortcuts.rs`; core `switching.rs`; tests `switching_parity.rs`, Windows `launch_targets.rs`.

**Consumes:** input commands, current config, platform snapshots.

**Produces:** `WindowSnapshot`, `SwitchPlanner::plan`, typed `SwitchEffect`, native `WindowService`/`Launcher` and catalog app candidates.

- [ ] Write fake-window tests for all three cycle modes, app groups, current process/window selection, minimized windows, no running app, optional launch and digit labels/indexing.
- [ ] Port dynamic key assignment and exclusions from existing tests.
- [ ] Implement window filtering, process identity, AUMID and shortcut metadata with explicit lifetime/apartment ownership.
- [ ] Implement typed launch actions; test paths with spaces/Unicode, stale HWND, disappeared process, packaged activation failure and no target app.
- [ ] Add `reused_hwnd_does_not_activate_unrelated_window`, including reuse within one process; use process identity and observed window generation for delayed effects.
- [ ] Implement foreground restoration/pulse behavior using measured OS outcomes, not forced-focus loops. Surface UIPI restrictions.

**Checks:** pure switching suite; opt-in discovery/launch smoke across classic, packaged, minimized and multi-window apps. Failure to activate must not be counted as a successful switch.

### Task 6 — Spotlight catalog, ranking, suggestions and indexed files

**Files:** core `search.rs`, `tests/search_parity.rs`, `benches/search.rs`; Windows `search.rs`, `icons.rs`; app worker routes in `workers.rs`.

**Consumes:** captured search fixtures, catalog candidates, usage snapshot, typed launch targets.

**Produces:** `SearchEngine` contract in Section 5, `CatalogSnapshot`, app/file result messages and stable identities.

- [ ] Write golden ranking tests, single-letter/Control Panel tests, raw-package/no-AUMID exclusions, duplicate identity and title cases, short-fuzzy rejection and usage-boost caps.
- [ ] Implement immutable cached catalog snapshots, one refresh at a time, last-good results while refreshing, and invalidation on config/app changes.
- [ ] Implement current matching semantics and suggestion ordering before tuning. Match Unicode intentionally rather than accidentally applying ASCII-only lowercasing.
- [ ] Implement Windows Search on its owning COM worker. Bound provider results and query length; correctly escape/parameterize syntax so quotes/operators cannot change query intent.
- [ ] Preserve file-type exclusions and query-length rules; handle disabled index, unavailable provider, malformed URI and cancellation separately.
- [ ] Deliver app results first. Add out-of-order, repeated-query and hide/reopen session tests; reject stale icon results too.
- [ ] Add successful-launch usage updates through storage events; no database call in the matching loop.

**Checks:** `cargo test -p omnispot-core --locked --test search_parity`; app worker/controller tests; opt-in real index smoke; reproducible Release benchmark against Task 0.

### Task 7 — GPUI Spotlight UI and application controller

**Files:** app `controller.rs`, `commands.rs`, `ui/{mod,theme,spotlight}.rs`, `tests/spotlight_lifecycle.rs`; Windows `theme.rs` and window attributes as required.

**Consumes:** search events, icon payloads, session IDs, typed launch commands.

**Produces:** working Spotlight integrated with Rust workers and the input machine, following Section 6.

- [ ] Write controller checks `old_query_cannot_replace_new`, `old_session_cannot_reopen_hidden_window`, `files_preserve_selected_result_id`, `launch_failure_keeps_recoverable_feedback` and `empty_state_waits_for_current_completion`.
- [ ] Build the compact dark vertical list and footer; fixed logical geometry with work-area clamping; asynchronous cached icons.
- [ ] Connect search editing and navigation; handle IME composition and normal text editing before Enter/shortcut dispatch.
- [ ] Connect launch acknowledgement to usage recording and close-on-success behavior. Retain a visible actionable error when launching fails.
- [ ] Verify rapid type/erase/retype, 1,000 result changes, keyboard selection, scrolling, focus loss and reopen across monitors.
- [ ] Capture first-frame and query-change behavior. No repeated white frame or hidden continuous rendering is accepted.

**Checks:** `cargo test -p omnispot-app --locked --test spotlight_lifecycle`; live keyboard/UI checks and screen capture where available.

### Task 8 — Overlay, warnings and peek lifecycle

**Files:** core `overlay.rs`, `peek.rs`, `tests/overlay_peek.rs`; app `ui/{overlay,warnings}.rs`; Windows activation/window attributes.

**Consumes:** input transitions, current window snapshots, config revision, injectable clock.

**Produces:** `OverlayState`, `OverlayTicket`, `PeekState` and no-activate GPUI windows.

- [ ] Write `off_never_shows_when_modifier_held`, `off_invalidates_queued_show`, `release_before_delay_never_shows`, `discard_restores_saved_overlay`, `stale_refresh_cannot_reopen_overlay`.
- [ ] Write peek threshold/short-tap/return/minimized-state tests, target-window-closed tests, and cancel-on-config-change/elevation tests.
- [ ] Test that an old peek snapshot cannot restore/minimize a new window that reused the old handle; cancel when identity cannot be verified.
- [ ] Implement generation-based delay/peek effects with no blocked worker or stale show callbacks.
- [ ] Port active/running/launchable/dynamic sections, eligible window grouping, digit selection, clickable rows and cursor/work-area positioning.
- [ ] Verify overlays and warnings do not steal focus; test the OFF setting before hold, while held and during pending work.

**Checks:** `cargo test -p omnispot-core --locked --test overlay_peek`; live modifier and focus smoke.

### Task 9 — Tray, settings UI and configuration application

**Files:** Windows `tray.rs`, `startup.rs`, `instance.rs`; app `ui/{settings,applications}.rs`, `bootstrap.rs`; tests `settings_lifecycle.rs`.

**Consumes:** store, config revision, platform app picker, overlay preview commands.

**Produces:** full settings/tray flows and atomically applied configuration events.

- [ ] Write tests for invalid saves, add/reorder/remove, reset, save/discard/cancel, preview OFF while a timer is pending, and failed persistence without partial application.
- [ ] Implement all General/Applications/Overlay/Spotlight/About controls and existing validation; preserve app order and configured groups.
- [ ] Implement tray open/double-click/exit, correct background residency and native instance ownership.
- [ ] Add Startup shortcut enable/disable with correct executable path; tests use an injected shortcut directory. No preview build edits the real Startup folder.
- [ ] Connect theme/accent/contrast and keyboard accessibility. Handle IME, focus movement, high contrast and reduced motion explicitly.

**Checks:** `cargo test -p omnispot-app --locked --test settings_lifecycle`; deliberate tray/startup smoke with original state recorded and restored.

### Task 10 — Statistics and usage history parity

**Files:** core `stats.rs`, `tests/stats_parity.rs`, app `ui/stats.rs`; storage event handling.

**Consumes:** switch/peek/Alt+Tab events, source stats fixtures and local-date clock.

**Produces:** equivalent daily aggregates, metrics, charts and enabled/disabled behavior.

- [ ] Port existing calculator, streak, efficiency, fastest-switch, static/dynamic aggregation and duration tests as fixtures/assertions.
- [ ] Test day rollover, restart on same day, historical dates, stats disabled, flush-on-exit and overflow accounting.
- [ ] Implement bounded batched event consumption, documented overflow behavior and final flush; never block hotkeys for stats.
- [ ] Port the dashboard and existing data reset/delete controls; ensure a delete targets the selected data scope and preserves configuration/other records.
- [ ] Verify fresh installations with statistics disabled do not start a collector or manufacture historical buckets.

**Checks:** `cargo test -p omnispot-core --locked --test stats_parity`; storage reopen and UI metrics comparisons.

### Task 11 — Legacy exporter and atomic import

**Files:** `tools/legacy-export/`, core `migration.rs`, `tests/migration.rs`, synthetic migration fixtures, `docs/migration/data-format.md`.

**Consumes:** Sections 7 and current legacy storage documents/migrations; initialized Rust store.

**Produces:** versioned migration bundle, import report, preserved old files and recoverable new state.

- [ ] Write fixtures for all settings/app types/key mappings, usage counts and complete daily buckets; include missing optional fields, unknown enum, corrupted/truncated input and mismatched source identity.
- [ ] Implement and verify the read/export utility using synthetic LiteDB databases. Snapshot only quiescent, flushed sources.
- [ ] Implement `validate_bundle` and transactional `import_bundle`; reject partial/conflicting imports and verify idempotence.
- [ ] Test initial import over seeded defaults, rejection of edited/used profiles without replacement consent, and rejection while another instance owns the profile.
- [ ] Test imported usage affecting real search order, including Control Panel, `.lnk`/executable aliases, AUMIDs and settings URIs; same-title apps must not inherit each other's counters.
- [ ] Simulate failure before commit, after import marker staging, and during validation; reopen and compare source/export/import totals.
- [ ] Document portable/installed profile selection and the optional separate exporter distribution. Ordinary Rust operation never invokes a managed process.
- [ ] Run an actual user-data migration only with a quiescent source and preserved backup; do not use real personal data as a committed test fixture.

**Checks:** synthetic exporter self-check plus `cargo test -p omnispot-core --locked --test migration`; produce a count/totals-only validation report.

### Task 12 — CLI, logs, startup and shutdown hardening

**Files:** app `main.rs`, `bootstrap.rs`, `commands.rs`, Windows instance/startup modules; `docs/migration/validation.md`.

**Consumes:** actual legacy CLI inventory, controller/worker owners, storage flush interface.

**Produces:** preserved CLI flags/commands, Rust-only startup/shutdown, safe diagnostic and recovery behavior.

- [ ] Test help and import/diagnostics before GPU initialization, unknown flags and missing arguments, second-instance handling, read-only directory and unsupported storage schema.
- [ ] Implement bounded rotating logs with debug/trace opt-in and no raw keystrokes/search history logged by default.
- [ ] Test exit while query/icon/COM/timer work is pending. Stop hook intake first, cancel work, drain accepted actions/storage with bounded shutdown and useful failure reporting.
- [ ] Revalidate sleep/resume, screen lock/unlock, missed modifier release, display changes and device-renderer failure behavior.
- [ ] Verify no leaked hooks, timers, thread handles or hidden windows after repeated restart.

**Checks:** controller/CLI integration tests; 1,000-cycle resource run; documented OS smoke scenarios.

### Task 13 — Rust build, portable package and installer

**Files:** `scripts/{build-rust,package-rust,verify-rust}.ps1`, `packaging/{installer.iss,omnispot.manifest}`, root `build-installer.ps1`, Rust resources and root README.

**Consumes:** validated x64 binary, pinned licenses/dependencies, compatibility matrix and current installer identity.

**Produces:** versioned native installer, portable ZIP, checksums, dependency/license manifest and reproducible commands.

- [ ] Test packaging against a staging fixture containing fake settings/log files; assert none are included and that required app resources/native libraries are included.
- [ ] Build `x86_64-pc-windows-msvc` Release with `--locked`; embed version/icon, GUI subsystem and per-monitor DPI/asInvoker manifest. Verify actual runtime DLL requirements on a clean machine.
- [ ] Stage to a versioned clean directory; never publish over a running binary or use a directory containing portable user data as the package source.
- [ ] Keep installer AppId `7954d19d-b9c7-42b4-882b-0c03e632e75f` and production mutex `OmniSpotMutex` for upgrade continuity; verify path/architecture interactions rather than assuming x86-to-x64 upgrades work automatically.
- [ ] Remove .NET runtime checks from the Rust installer. Preserve per-user operation and existing default install choices unless the architecture migration requires an explicitly documented change.
- [ ] Build one native installer and one portable ZIP per supported architecture; the old framework-dependent/self-contained distinction does not apply to normal Rust delivery.
- [ ] Exclude `.portable` from installers; include it deliberately in portable ZIPs. Exclude all databases, journals, logs, private exports, debug data and build caches.
- [ ] Test clean install, upgrade from the old app, correct Startup link, uninstall/reinstall, portable folder with spaces/Unicode and no administrator rights. Uninstall preserves user data by default.

**Checks:** clean Windows smoke without .NET/Rust/Cargo installed; package manifest self-check, installer compile, portable run, artifact SHA-256 and version verification.

### Task 14 — Final regression, performance and production cutover

**Files:** update `docs/migration/{parity,compatibility,performance,validation,release}.md`, root README and migration instructions.

**Consumes:** completed implementation tasks and candidate artifacts.

**Produces:** evidence-backed release report and user-reviewable deliverables.

- [ ] Run the full Rust verification commands in Section 12 and fresh archived C# parity checks where needed.
- [ ] Complete Section 13's acceptance matrix, preserving exact failed/not-tested labels.
- [ ] Repeat Task 0 benchmarks with the same workload and report medians/p95/p99, cold/warm paths, hidden idle and long-run resources.
- [ ] Investigate each regression and each dropped parity feature. A failing core feature blocks production cutover.
- [ ] Run upgrade/data import and rollback rehearsal with synthetic profiles. Confirm the original app can still use its untouched databases after a failed Rust upgrade.
- [ ] Ensure production startup runs only Rust; stop the old app before switching hook ownership; verify only one production process/tray entry.
- [ ] Produce final installer/ZIP/checksums, known compatibility limits, migration instructions and a concise completion report. Leave all Git staging/committing/pushing to the user.

## 12. Verification commands and evidence

Execute commands from the repository root after the relevant files exist. MSVC tools and the validated target must be available.

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build -p omnispot-app --release --target x86_64-pc-windows-msvc --locked
```

The release workflow introduced by Task 13 must expose:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\verify-rust.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\build-rust.ps1 -Configuration Release -Target x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File .\scripts\package-rust.ps1 -Version <release-version> -Target x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File .\build-installer.ps1 -Version <release-version>
```

These future scripts are deliverables, not commands claimed to work today. `verify-rust.ps1` runs deterministic checks by default; live hook/startup/renderer scenarios require an explicit integration switch and isolation so they do not alter the developer's desktop unexpectedly.

For each gate record: command, date, build identifier/source state, OS/architecture, dependency/toolchain versions, exit status, results and artifact location. Avoid committing private machine names/user paths. A compiler success, a unit-test pass and a visual inspection are separate evidence categories.

## 13. Final acceptance matrix

| Area | Required cases | Evidence owner |
|---|---|---|
| Keys | Left/right modifiers, extra modifiers, autorepeat, injected input, unbound combination, digits, reconfigure while held, missed release | Tasks 4/12; replay traces + OS smoke |
| Switching | All cycle modes, app groups, minimized/closed/elevated windows, optional launch, classic/packaged apps, dynamic mode | Tasks 5/8; snapshots + OS smoke |
| Peek | Short tap, >=400 ms hold, previous window closed, target initially minimized, config/elevation cancellation | Task 8; fake clock + OS smoke |
| Overlay OFF | Before hold, while held, pending timer, pending refresh, discard/reset/save/restart | Tasks 8/9; deterministic race tests + live focus check |
| Spotlight | Single-letter/control query, ranking, duplicates, binary exclusions, large catalog, suggestions, launch learning, indexed files unavailable | Tasks 6/7; golden fixture + real index smoke |
| UI races | Rapid query changes, repeated query, out-of-order files/icons, hide/reopen, IME Enter, selected-item removal | Tasks 6/7; controller tests + UI check |
| Visuals | Dark first frame, no white flash, compact rows, footer paths, scroll, no huge tiles, 100/125/150/200% DPI | Tasks 1/7; capture + manual inspection |
| Hardware | Win10/11, integrated/discrete GPU, RDP, VM/software adapter, sleep/resume/device change | Tasks 1/12; passed/failed/not-tested matrix |
| Accessibility | Complete keyboard flows, clear focus, high contrast, reduced motion, labels/screen reader, Bengali/IME | Tasks 1/7/9; explicit results and limitations |
| Data | Settings, ordered apps/keys, all usage/stat totals, defaults, Unicode, idempotence, corrupt/unsupported input, rollback | Tasks 3/10/11; synthetic exporter/import fixtures |
| Background | Hidden app resident, tray works, no continuous redraw, no polling loop, bounded workers, resource stability | Tasks 1/12/14; 60-second idle + 1,000-cycle run |
| Distribution | Clean machine without .NET, upgrade/rollback, native dependencies, portable/install data paths, user-data exclusions | Tasks 13/14; package manifest + clean VM smoke |

## 14. Risks and responses

| Risk | Early signal | Response |
|---|---|---|
| GPUI window features need unsupported internals | Cannot show a no-activate overlay or retrieve a stable native handle | Resolve in Task 1; use supported Win32 adapter where possible; no widespread internal patches. |
| GPU/driver incompatibility | Blank/failing window on target hardware | Verify pinned backend and any supported WARP path; native diagnostic; honest support matrix; Rust-only framework decision if essential target fails. |
| x86 compatibility loss | GPUI dependencies do not build for legacy x86 target | Record before port; do not ship an x86 label on an x64-only build. Architecture support is a cutover decision. |
| Lost legacy data | Export omits enum/date/stat fields or import partially commits | Complete synthetic fixtures, quiescent export, validation + single transaction, untouched old data and rollback rehearsal. |
| Hook causes typing failures | Suppression/keyups unpaired or callback blocks | Pure replay tests, cheap hook callback, bounded admission, panic containment and isolated live smoke. |
| Focus/overlay flicker | Delayed show or foreground polling fights user input | Session/config/timer generations, event-driven foreground tracking and final-show enablement checks. |
| Rust port slows perceived search | Serialization/locks/icons dominate the match loop | No inter-language bridge; immutable snapshots; icons off critical path; compare end-to-end baseline. |
| Background utility uses too much GPU/CPU | Hidden frames or periodic work persist | Explicit hide suspension, no repeating catalog/focus timers, idle instrumentation before release. |
| GPUI upgrade breaks builds | Floating dependencies or toolchain drift | Pin exact tested inputs and lockfile; upgrades require compatibility/regression checks. |
| Installer ships private data | Staging glob catches portable databases/logs | Clean staging, explicit manifest/exclusion check, no source from live output. |

## 15. Definition of done and handoff

- [ ] User-facing app, all application logic and ongoing runtime are Rust; GPUI is the UI on the proved target platform.
- [ ] All current features in Section 2 are ported or an explicitly accepted release limitation is recorded; no quiet feature removal.
- [ ] C# source is preserved buildably under `legacy/csharp/`; no original user data or live outputs were moved by the archive task.
- [ ] The migration utility is separate and temporary; normal installation/startup/search/switching never requires .NET or invokes C#.
- [ ] User settings, app order/key identities, usage and statistics import correctly with an atomic/idempotent procedure and verified rollback.
- [ ] Overlay OFF, stale query/session, keyboard pairing and hidden-idle regressions have direct tests and available live checks.
- [ ] Rust checks and Release packaging pass; compatibility/performance claims match recorded evidence.
- [ ] Native installer, portable ZIP, checksums, license manifest and upgrade instructions are produced.
- [ ] No Git staging, committing, pushing, tracking changes or remote changes were performed by the agent.

Recommended handoff: give an implementation agent this document and root `AGENTS.md`; ask it to complete Task 0 and Task 1 first, then continue through the ordered tasks while reporting gate outcomes. This document is an end-to-end migration plan, not evidence that the Rust app already exists.

## Sources checked during planning

- **S1:** [Upstream GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md): framework lifecycle, Windows backend/text integration and pre-1.0 compatibility warning. Pin and recheck upstream APIs during Task 1.
- **S2:** [Zed installation requirements](https://zed.dev/docs/installation): Windows graphics/driver and architecture guidance. These are Zed's published requirements, not automatic OmniSpot certification.
- **S3:** [Zed Windows troubleshooting](https://zed.dev/docs/windows): graphics initialization and VM considerations.
- **S4:** [Microsoft WARP guide](https://learn.microsoft.com/en-us/windows/win32/direct3darticles/directx-warp): software rendering capability in Windows; does not establish GPUI integration.
- **S5:** [Microsoft Rust Windows API documentation](https://microsoft.github.io/windows-docs-rs/doc/windows/): Windows/WinRT binding surface.
- **S6:** [Microsoft LowLevelKeyboardProc documentation](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc): callback message-loop, forwarding, state timing and timeout constraints.
- **S7:** [redb documentation](https://docs.rs/redb/latest/redb/): pure Rust embedded transactional storage. Use a tested pinned version, not an assumed latest API.
- **Repository evidence:** root `AGENTS.md`, `.agents/architecture.md`, current README, `OmniSpot/OmniSpot.csproj`, and source files enumerated in Section 2, inspected on the planning date. Future execution must revalidate them.
