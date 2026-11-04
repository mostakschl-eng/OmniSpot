# Spotlight Search Launcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Follow strict TDD conventions in `.agents/testing-conventions.md`, architecture rules in `.agents/architecture.md`, and C# 13 guidelines in `.agents/csharp-conventions.md`. Never run `git add`, `git commit`, or `git push`. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a modern, lightning-fast Spotlight-style search panel to OmniSpot, featuring empty-query app suggestions (most-used / recent apps with icons), live app & Windows Search indexed file matching, keyboard navigation, and an independent configurable shortcut configured via a new Settings tab following the existing keyboard layout design.

**Architecture Reference:**
- **[Flow Launcher](https://github.com/Flow-Launcher/Flow.Launcher)**: OLE DB `Search.CollatorDSO` Windows Search catalog querying, fuzzy ranking, non-blocking asynchronous pipeline with `CancellationToken`.
- **[PowerToys Run](https://github.com/microsoft/PowerToys)**: Modern WPF floating search palette with backdrop, autofocus search box, instant keyboard navigation (`Up`/`Down`/`Enter`/`Escape`), and empty-query quick suggestions.

**Tech Stack:** .NET 8 WPF, `WPF-UI`, `System.Data.OleDb` for Windows Search OLE DB, existing `KeyboardHookLite` keyboard hook in `Hook.cs`, LiteDB settings persistence, and OmniSpot's existing app & usage statistics services.

---

## Global Constraints & Principles

- **No regression:** Keep existing app-switching overlay, hotkeys, cycle modes, and dynamic mode completely unchanged.
- **Single Hook:** Reuse the existing low-level keyboard hook (`KeyboardHookLite` in `Hook.cs`); do not register multiple system hooks.
- **Instantaneous Activation:** Default to `Alt + Space` (0ms delay, no menu side effects). Allow user-configured modifier choice (`Alt`, `Ctrl`, `Win`) via the visual keyboard layout control.
- **Non-blocking Search:** All file & app queries run on background threads with `CancellationToken`; typing debounced (~60ms); UI remains 60fps responsive.
- **Resilient Fallback:** If Windows Search service (`WSearch`) is disabled or stopped, app search remains 100% functional, and file search limitations are handled gracefully without crashing.
- **Pro Standards:** Strict AAA unit tests with xUnit v3 and AwesomeAssertions before implementation; file-scoped namespaces; Allman braces; no git operations.

---

## Settings & UI Architecture

### 1. Settings Navigation: New "Spotlight Search" Tab
- In `OmniSpot/UI/Windows/Settings.xaml`, directly after the `Overlay` tab, add:
  `<ui:NavigationViewItem Content="Spotlight Search" TargetPageType="{x:Type pages:SpotlightSearch}">` with icon `Search24`.
- Create `OmniSpot/UI/Pages/SpotlightSearch.xaml` and `SpotlightSearchViewModel.cs`:
  - Card 1: **Enable Spotlight Search** toggle switch.
  - Card 2: **Activation Shortcut** — replicates the visual keyboard layout control (`controls:ModifierKeySelector`) allowing the user to select their desired modifier key (pairing with Space).
  - Card 3: **Search Options** — toggles for including Windows Search indexed files, max results count.

### 2. Spotlight Search Panel (`LauncherWindow.xaml`)
- Borderless, rounded Fluent window centered horizontally in the upper third of the active monitor.
- Top: Search `TextBox` with search icon, auto-focused on opening.
- Bottom (on empty query): Suggested / Most Used / Recent apps grid with sharp icons and names, sourced from OmniSpot's `SessionStats` / `DailyBucketDocument` and configured applications.
- Bottom (on query): Filtered list of apps and indexed files with badge ("App" / "File"), path/context subtitle, and smooth `Up`/`Down` keyboard selection.
- `Enter` executes the item via shell (`Process.Start` / `shell:AppsFolder`); `Escape` or clicking outside (`Deactivated`) closes the window.

---

### Task 1: Search Result Models & Search Providers

**Files:**
- Create: `OmniSpot/Search/SearchResult.cs`
- Create: `OmniSpot/Search/IAppSearchProvider.cs` & `OmniSpot/Search/AppSearchProvider.cs`
- Create: `OmniSpot/Search/IFileSearchProvider.cs` & `OmniSpot/Search/WindowsSearchFileProvider.cs`
- Create: `OmniSpot/Search/LauncherSearchService.cs`
- Create: `OmniSpot.Tests/Search/LauncherSearchServiceTests.cs`
- Create: `OmniSpot.Tests/Search/AppSearchProviderTests.cs`

**Steps:**
- [ ] **Step 1: Write failing tests** for `AppSearchProvider` (matching configured apps, running apps, packaged apps, start menu shortcuts) and `LauncherSearchService` (query ranking, empty-query app suggestions from stats, duplicate deduplication, cancellation token support).
- [ ] **Step 2: Run tests and confirm expected failures.**
- [ ] **Step 3: Implement `SearchResult`, `AppSearchProvider`, `WindowsSearchFileProvider` (via `Search.CollatorDSO` OLE DB with error handling), and `LauncherSearchService`.**
- [ ] **Step 4: Run `dotnet test --filter FullyQualifiedName~Search` and verify all tests pass.**

---

### Task 2: Configuration & Global Shortcut Handling

**Files:**
- Modify: `OmniSpot/Configuration/Configuration.cs`
- Modify: `OmniSpot/Configuration/Storage/SettingsDocument.cs`
- Modify: `OmniSpot/Configuration/ConfigurationValidator.cs`
- Modify: `OmniSpot/Input/KeyStateMachine.cs` & `OmniSpot/Input/Hook.cs`
- Create: `OmniSpot.Tests/Configuration/LauncherConfigurationTests.cs`
- Modify: `OmniSpot.Tests/Input/KeyStateMachineTests.cs` & `OmniSpot.Tests/Input/HookTests.cs`

**Steps:**
- [ ] **Step 1: Write failing tests** for launcher configuration defaults, validation (no conflict with regular app modifier), key state machine detecting `Modifier + Space`, and hook triggering launcher toggle without interfering with regular app switching.
- [ ] **Step 2: Run tests and confirm expected failures.**
- [ ] **Step 3: Update `Configuration`, `SettingsDocument`, and `ConfigurationValidator`.**
- [ ] **Step 4: Update `KeyStateMachine` and `Hook` to dispatch launcher shortcut event cleanly.**
- [ ] **Step 5: Run tests and verify all pass.**

---

### Task 3: Settings Page with Visual Key Layout Selector

**Files:**
- Create: `OmniSpot/UI/Pages/SpotlightSearch.xaml`
- Create: `OmniSpot/UI/Pages/SpotlightSearch.xaml.cs`
- Create: `OmniSpot/UI/ViewModels/SpotlightSearchSettingsViewModel.cs`
- Modify: `OmniSpot/UI/Windows/Settings.xaml` (add navigation item directly after `Overlay`)
- Modify: `OmniSpot/UI/ViewModels/Common/SettingsState.cs`
- Modify: `OmniSpot/ServicesConfiguration.cs`
- Create: `OmniSpot.Tests/UI/ViewModels/SpotlightSearchSettingsViewModelTests.cs`

**Steps:**
- [ ] **Step 1: Write failing tests** for `SpotlightSearchSettingsViewModel` (reading/updating modifier key, enabling/disabling launcher, state synchronization).
- [ ] **Step 2: Run tests and confirm expected failures.**
- [ ] **Step 3: Implement `SpotlightSearchSettingsViewModel`, `SpotlightSearch.xaml` using the exact card style and `controls:ModifierKeySelector` layout, and link in `Settings.xaml`.**
- [ ] **Step 4: Run tests and verify all pass.**

---

### Task 4: Spotlight Search Window & Result Interaction

**Files:**
- Create: `OmniSpot/UI/Windows/LauncherWindow.xaml`
- Create: `OmniSpot/UI/Windows/LauncherWindow.xaml.cs`
- Create: `OmniSpot/UI/ViewModels/LauncherViewModel.cs`
- Modify: `OmniSpot/ServicesConfiguration.cs`
- Create: `OmniSpot.Tests/UI/ViewModels/LauncherViewModelTests.cs`

**Steps:**
- [ ] **Step 1: Write failing tests** for `LauncherViewModel` (loading suggestions on open, debounced query search, selection index tracking with Up/Down, Enter launching target, Escape resetting).
- [ ] **Step 2: Run tests and confirm expected failures.**
- [ ] **Step 3: Build `LauncherWindow.xaml` with modern WPF-UI styling, auto-centering on active monitor, search box with magnifying glass icon, suggested apps panel with icons when empty, and results list when searching.**
- [ ] **Step 4: Wire shell execution for Win32 executables, files, and UWP/packaged apps, closing launcher upon launch or deactivation.**
- [ ] **Step 5: Run tests and verify all pass.**

---

### Task 5: Integration & Verification

**Files:**
- Update `README.md`
- End-to-end verification

**Steps:**
- [ ] **Step 1: Run full test suite (`dotnet test`) and code formatting (`dotnet format`).**
- [ ] **Step 2: Verify Release build compiles cleanly (`dotnet build -c Release`).**
- [ ] **Step 3: Verify seamless coexistence between Spotlight Search and OmniSpot normal app switching.**
