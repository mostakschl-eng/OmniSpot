# UI conversion status

The C# UI has 16 XAML views/controls and 59 UI files including code-behind and view models. Rust UI files mirror the C# names under `rust/OmniSpot/UI/{Controls,Pages,ViewModels,Windows}`. The C# sources remain the visual and behavior reference until each Rust view is checked side by side.

| C# XAML | Rust UI | Status |
|---|---|---|
| `Windows/Settings.xaml` | `Windows/Settings.rs` | Shell, navigation and footer present; focus, discard and Save & Apply pending |
| `Pages/General.xaml` | `Pages/General.rs` | Cards and key selector present; layout and saved settings pending |
| `Controls/ModifierKeySelector.xaml` | `Controls/ModifierKeySelector.rs` | Three keyboard rows and selection present; visual check pending |
| `Pages/Hotkeys.xaml` | `Pages/Hotkeys.rs` | Empty state, configured row layout and Add flyout visible; picker, drag/reorder and editing pending |
| `Controls/KeyAssignmentButton.xaml` | `Controls/KeyAssignmentButton.rs` | Badge present; assignment input pending |
| `Controls/CycleModeSelector.xaml` | `Controls/CycleModeSelector.rs` | Three mode buttons select the Rust page state; saved settings pending |
| `Controls/AddApplicationFlyout.xaml` | `Controls/AddApplicationFlyout.rs` | Flyout layout present; search, browse and selection pending |
| `Pages/Overlay.xaml` | `Pages/Overlay.rs` | Cards and bounded 100 ms delay stepper present; C# numeric text input and live preview pending |
| `Pages/SpotlightSearch.xaml` | `Pages/SpotlightSearch.rs` | Toggle and key selector present; saved settings pending |
| `Pages/Stats.xaml` | `Pages/Stats.rs` | Privacy shield, enable and disable controls present; reset action and full dashboard pending |
| `Controls/BentoDashboard.xaml` | `Controls/BentoDashboard.rs` | Metric groups present; charts, data and layout pending |
| `Pages/About.xaml` | `Pages/About.rs` | Identity card and Support rows present; icon and actions pending |
| `Windows/MainWindow.xaml` | — | Tray owner pending |
| `Windows/AppOverlayWindow.xaml` | `Windows/AppOverlayWindow.rs` | Running/launchable/dynamic row sections bound to UI state; native placement, focus and actions pending |
| `Windows/WarningOverlayWindow.xaml` | `Windows/WarningOverlayWindow.rs` | 300 px warning card, title, message, dismiss and caution bar present; native placement, countdown and link pending |
| `Windows/SpotlightWindow.xaml` | `Windows/SpotlightWindow.rs` | Search box, result list and footer layout bound to UI state; text entry, selection and actions pending |

UI state is local to the Rust process. It does not read the active C# databases, install startup shortcuts, register hooks or collect statistics. Displaying a control is not evidence that its C# action has been ported. Complete each pending row before claiming full UI parity.

## File-stem audit

The C# `UI` tree has 59 physical files. XAML and its code-behind share one Rust `.rs` stem. The five `ViewModels/DesignTime` classes exist only for the WPF designer; `rust/OmniSpot/UI/ViewModels/DesignTime/README.md` records why they have no Rust runtime modules. The Rust tree still lacks runtime equivalents for `Pages/PageProviderService`, `Windows/MainWindow`, and 15 view-model stems (including Settings, Hotkeys, General, Overlay, Stats, and the shared state helpers). These are outstanding conversions, not completed files.
