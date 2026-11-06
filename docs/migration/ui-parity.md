# UI conversion status

The C# UI has 16 XAML views/controls and 59 UI files including code-behind and view models. Rust uses fewer, sometimes combined modules; the C# sources remain the behavior and visual reference. A matching file count is not the parity gate.

| C# XAML | Rust UI | Status |
|---|---|---|
| `Windows/Settings.xaml` | `Windows/Settings.rs` | Shell, navigation, settings state, Save & Apply, reset and discard prompt are wired; visual and focus parity still needs review |
| `Pages/General.xaml` | `Pages/General.rs` | Theme, modifier, startup, dynamic, pulse-border and peek controls are wired; visual parity remains |
| `Controls/ModifierKeySelector.xaml` | `Controls/ModifierKeySelector.rs` | Modifier selection is wired; visual and keyboard-access parity remains |
| `Pages/Hotkeys.xaml` | `Pages/Hotkeys.rs` | App add/remove/reorder, key assignment, cycle mode, picker and save workflow are wired; drag-reorder parity remains |
| `Controls/KeyAssignmentButton.xaml` | `Controls/KeyAssignmentButton.rs` | Key capture and assignment are wired; visual parity remains |
| `Controls/CycleModeSelector.xaml` | `Controls/CycleModeSelector.rs` | All three cycle modes update the shared settings state; visual parity remains |
| `Controls/AddApplicationFlyout.xaml` | `Controls/AddApplicationFlyout.rs` | Running-app selection and executable browsing are wired; visual and search behavior parity remains |
| `Pages/Overlay.xaml` | `Pages/Overlay.rs` | Delay and toggles update shared settings; overlay preview is wired; exact numeric input/layout parity remains |
| `Pages/SpotlightSearch.xaml` | `Pages/SpotlightSearch.rs` | Spotlight enable and modifier settings are wired; visual parity remains |
| `Pages/Stats.xaml` | `Pages/Stats.rs` | Privacy, enable/disable, confirmed reset and maintenance action are wired; dashboard parity remains incomplete |
| `Controls/BentoDashboard.xaml` | `Controls/BentoDashboard.rs` | Live metrics and warmup/maintenance cards are wired; complete C# card layout, progress indicators and tooltips remain |
| `Pages/About.xaml` | `Pages/About.rs` | App icon/version and support actions are wired, including window diagnostics to clipboard; visual parity remains |
| `Windows/MainWindow.xaml` | `UI/Tray.rs`, `App.rs` | Tray-first Settings/Exit lifecycle is implemented; live lifecycle parity still needs validation |
| `Windows/AppOverlayWindow.xaml` | `Windows/AppOverlayWindow.rs` | Running/launchable/dynamic rows and actions are wired; native placement/focus behavior needs live validation |
| `Windows/WarningOverlayWindow.xaml` | `Windows/WarningOverlayWindow.rs` | Warning display and dismiss are wired; native placement/focus behavior needs live validation |
| `Windows/SpotlightWindow.xaml` | `Windows/SpotlightWindow.rs` | Search input, result selection, keyboard navigation and launch are wired; live focus/IME and exact visual parity remain |

Settings use Rust JSON data; the active C# LiteDB files are only accessed through the separate read-only migration exporter. Controls above are connected to Rust state/actions, while each listed visual or live behavior gap remains unverified.

## File-stem audit

The C# `UI` tree has 59 physical files. XAML and code-behind behavior is combined in Rust modules, and shared WPF view-model responsibilities are folded into Rust page/window state. The five `ViewModels/DesignTime` classes are designer-only. `PageProviderService` is represented by the Rust Settings page enum/render switch; `MainWindow` lifecycle is represented by the tray and app composition. Review remaining behavior and visuals by the table above, not by one-to-one view-model file counts.
