# Spotlight-style Search Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a separate Spotlight-style search panel and an explicit “open beside” action, without changing the existing AppSwitcher overlay or normal app-switch shortcuts.

**Architecture:** Add a small search window with its own configurable global shortcut. Search app entries and Windows Search indexed files as the user types; show app suggestions before a query. Separately, route `configured modifier + Alt + app key` to an “open beside” action (for example, `Shift+Alt+S`) while leaving `configured modifier + app key` as normal switching. Arrange only windows tracked in an AppSwitcher-managed group, up to four. Use [Flow Launcher](https://github.com/Flow-Launcher/Flow.Launcher) as the primary search behavior reference and [PowerToys](https://github.com/microsoft/PowerToys) Command Palette as a secondary UI reference; adapt the ideas to AppSwitcher rather than copying their architecture.

**Tech Stack:** Existing .NET 8 WPF app, keyboard hook, configuration store, and Windows Search index. No new plugin system or search dependency for the first version.

**Spec:** User requirements are captured in the Scope section of this plan; a separate spec was not requested.

## Global Constraints

- Keep the existing app-switcher overlay, appearance, and app hotkeys unchanged.
- Open search in its own panel using a separately configurable shortcut.
- Keep `configured modifier + app key` unchanged; `configured modifier + Alt + app key` opens that app beside the active/managed layout.
- For the first beside action, arrange the active window and chosen app as two halves. Add a third and fourth app only to an AppSwitcher-managed group, using three-zone and 2×2 layouts; do not rearrange unrelated windows.
- Search installed/known apps and files available through the Windows Search index; do not recursively crawl the whole disk on each query.
- Show app suggestions when the query is empty; open a selected result by click or Enter and close with Escape.
- Keep the panel responsive while typing and retain local-only behavior.

## Scope

- The panel opens with its search field focused and a short list of app suggestions.
- Typing filters app and indexed-file results together, with result type and useful path/context visible.
- Up/Down changes selection; Enter or click opens the selected app or file; Escape closes the panel.
- A separate shortcut setting can be changed without changing the existing app modifier or its A–Z bindings.
- If Windows Search is unavailable or a location is not indexed, app search still works and the file-search limitation is shown clearly; no full-disk fallback scan is added.

## Approaches

- **Separate panel and shortcut — chosen.** Keeps app switching predictable and gives search its own growing result list.
- **Add search to the existing overlay — not chosen.** Mixes two workflows and changes an overlay the user wants to keep as-is.
- **Expand app shortcuts into arbitrary modifier chords — not chosen.** The plan adds only one explicit alternate action (`configured modifier + Alt + app key`), rather than an open-ended set of combinations.
- **Add beside behavior to the normal app-switch chord — not chosen.** It would change the established meaning of existing shortcuts; the extra Alt modifier makes the action explicit.

## Review Focus

- Search must not block the UI while the user types; verify provider work runs off the UI thread and stale query results are discarded.
- A missing or disabled Windows Search index must not prevent app search or crash the panel; test this path.
- Duplicate names and paths must remain distinguishable in results; test selection opens the chosen path.
- The search shortcut must not alter or trigger an existing app binding; test both shortcut paths and conflict validation.
- Closing the panel during an in-flight search must not reopen it or update a closed window; test cancellation/stale-result handling.
- `configured modifier + Alt + app key` must not be mistaken for a normal app switch, while the unmodified app shortcut remains unchanged; test both paths.
- Failed or delayed app launch must not cause AppSwitcher to move an unrelated window; test timeout and stale-handle handling.
- A fifth app must not silently displace a window from a four-window group; test the full-group behavior.

---

### Task 1: Search result model and app/file providers

**Files:**
- Create: `AppSwitcher/Search/SearchResult.cs`
- Create: `AppSwitcher/Search/LauncherSearchService.cs`
- Create: `AppSwitcher.Tests/Search/LauncherSearchServiceTests.cs`

**Interfaces:**
- `SearchResult` contains display name, result type, open target, and optional path/context.
- `LauncherSearchService` accepts a query and returns ranked app and indexed-file results; an empty query returns app suggestions only.

- [ ] **Step 1: Write failing tests** for app-name matching, indexed-file matching, empty-query suggestions, no results, and duplicate display names.
- [ ] **Step 2: Run those tests and confirm the expected failures.**
- [ ] **Step 3: Implement app lookup using the app discovery/configuration sources already in the project and file lookup through the Windows Search index.** Keep file-query work off the UI thread and cap results to a small list suitable for the panel.
- [ ] **Step 4: Run `dotnet test AppSwitcher.Tests --filter FullyQualifiedName~LauncherSearchServiceTests` and confirm the tests pass.**

### Task 2: Independent launcher shortcut

**Files:**
- Modify: `AppSwitcher/Configuration/Configuration.cs`
- Create: the next `AppSwitcher/Configuration/Migrations/*` migration
- Modify: `AppSwitcher/Configuration/ConfigurationValidator.cs`
- Modify: `AppSwitcher/Input/KeyStateMachine.cs` and `AppSwitcher/Input/Hook.cs`
- Modify: the existing settings page/view model and key-capture control
- Test: configuration migration, shortcut validation, and key-state handling under `AppSwitcher.Tests/`

**Interfaces:**
- Persist one launcher shortcut separately from the existing shared modifier and app keys.
- Reuse the current keyboard hook; do not register a second system-wide hook.

- [ ] **Step 1: Write failing tests** for default configuration, invalid/conflicting shortcut rejection, launcher shortcut dispatch, and unchanged existing app-key dispatch.
- [ ] **Step 2: Run the focused tests and confirm the expected failures.**
- [ ] **Step 3: Add the persisted setting, migration, validation, and settings control.** Pick a safe default during implementation and allow the user to change it.
- [ ] **Step 4: Extend the shared hook to open the launcher on that shortcut while preserving the current app modifier behavior.**
- [ ] **Step 5: Run the focused configuration and input tests.**

### Task 3: Search panel and result interaction

**Files:**
- Create: `AppSwitcher/UI/Windows/LauncherWindow.xaml`
- Create: `AppSwitcher/UI/Windows/LauncherWindow.xaml.cs`
- Create: `AppSwitcher/UI/ViewModels/LauncherViewModel.cs`
- Modify: `AppSwitcher/ServicesConfiguration.cs`
- Test: `AppSwitcher.Tests/UI/ViewModels/LauncherViewModelTests.cs`

**Interfaces:**
- The view model consumes `LauncherSearchService` and exposes query, suggestions/results, selected result, and open/close actions.
- The window is independent of `AppOverlayWindow` and receives the launcher shortcut event from the existing hook.

- [ ] **Step 1: Write failing view-model tests** for suggestion display, query updates, arrow selection, Enter/open, and Escape/close.
- [ ] **Step 2: Run the focused tests and confirm the expected failures.**
- [ ] **Step 3: Implement a compact search window with a focused search box, suggestions, grouped app/file results, and keyboard navigation.**
- [ ] **Step 4: Open selected apps/files with the existing Windows launch mechanisms; log launch errors without crashing the app.**
- [ ] **Step 5: Run view-model and search-service tests, then `dotnet build -c Release`.**

### Task 4: Search panel documentation

**Files:**
- Modify: `README.md` or `docs/src/content/docs/configuration/` launcher documentation

- [ ] **Step 1: Document the search shortcut, app/file search scope, and the Windows Search indexing limitation.**
- [ ] **Step 2: Manually verify opening/closing, app suggestions, app launch, indexed-file launch, keyboard navigation, and coexistence with existing app-switcher shortcuts.**

### Task 5: Explicit “open beside” shortcut routing

**Files:**
- Modify: `AppSwitcher/Input/KeyStateMachine.cs` and `AppSwitcher/Input/Hook.cs`
- Modify: `AppSwitcher/Configuration/ConfigurationValidator.cs` if the alternate chord needs persisted conflict validation
- Test: `AppSwitcher.Tests/Input/KeyStateMachineTests.cs` and relevant configuration tests

**Interfaces:**
- Normal app switch: `configured modifier + app key` (example: `Shift+S`).
- Beside action: `configured modifier + Alt + app key` (example: `Shift+Alt+S`).

- [ ] **Step 1: Write failing tests** showing that `Shift+S` stays a normal switch and `Shift+Alt+S` selects the same app for beside mode.
- [ ] **Step 2: Run the focused tests and confirm the expected failures.**
- [ ] **Step 3: Add explicit alternate-chord routing before extra-modifier rejection.** Do not reinterpret arbitrary extra modifiers as app shortcuts; handle only the defined beside chord.
- [ ] **Step 4: Run the key-state and hook tests, including unbound keys and key release order.**

### Task 6: App-managed 2–4 window layouts

**Files:**
- Create: `AppSwitcher/WindowDiscovery/WindowLayoutService.cs`
- Modify: `AppSwitcher/Switcher.cs` and `AppSwitcher/ServicesConfiguration.cs`
- Test: `AppSwitcher.Tests/WindowDiscovery/WindowLayoutServiceTests.cs`

**Interfaces:**
- The service receives the active window and target app/window, and returns the handles and bounds for an AppSwitcher-managed group.
- Track only windows placed by this feature; do not infer membership from arbitrary Windows Snap or manually positioned windows.

- [ ] **Step 1: Write failing tests** for two-window halves, adding a third, adding a fourth in a 2×2 layout, stale/closed windows, and the full-group limit.
- [ ] **Step 2: Run those tests and confirm the expected failures.**
- [ ] **Step 3: Implement monitor-work-area layout calculation and apply bounds with the Windows window-positioning API.** Restore minimized windows before placement and wait for newly launched targets with a bounded timeout.
- [ ] **Step 4: If a target cannot be launched or found, leave the existing group unchanged and use the normal switch/open behavior.** When four slots are occupied, keep the group intact and use normal switch/open behavior instead of replacing a member.
- [ ] **Step 5: Run layout and input tests, then manually verify 2-, 3-, and 4-window arrangements on the same monitor.**

### Task 7: Final build and installer

**Files:**
- Modify: `README.md` or `docs/src/content/docs/configuration/` to document the beside shortcut and its four-window limit
- Build: Release executable and Inno Setup installer

- [ ] **Step 1: Run the complete test suite and Release build.**
- [ ] **Step 2: Compile the Inno Setup installer and verify it contains the new search and layout features.**
- [ ] **Step 3: Manually verify normal switching, `Shift+Alt+S` beside behavior, the search panel, and that unrelated windows stay put.**
