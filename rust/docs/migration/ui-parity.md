# OmniSpot Rust migration parity

This ledger records behavior that is intentionally tracked while the Rust runtime replaces the C# runtime.

## Implemented

- Configuration JSON load, validation, atomic save, live Save & Apply updates, and packaged-app migration.
- Global keyboard hook, modifier state machine, dynamic mode, synthetic passthrough, overlay timer, peek restore, and clean hook shutdown.
- Win32 window enumeration, placement/style/cloaking filtering, process path and friendly-name inspection, elevation detection, packaged-app discovery, switching, and launch.
- Overlay grouping, window-number actions, static/dynamic launch actions, native tray menu, and application lifecycle.
- Spotlight app/file search, usage ranking, keyboard navigation, mouse activation, launch, and packaged-app catalog.
- Stats collection, dashboard metrics, JSON persistence, historical bucket retention, and configuration enable/disable behavior.
- Rust release build and unit-test suite.

## Remaining parity work

- Windows shell icon extraction/rasterization for Win32 executables and file results is not yet equivalent to WPF `IconExtractor`.
- Persistent Rust app-name registry and Windows friendly process-name resolution are now used by dynamic mode and overlay discovery.
- Windows Search/`SystemIndex` file search is represented by a bounded filesystem provider; provider-level search parity is still pending.
- Warning overlay dismissal, focus/deactivation behavior, and several WPF visual interactions need runtime validation on Windows.
- Stats app-registry persistence and legacy LiteDB statistics migrations need import compatibility if existing C# databases are to be retained.
- Inno Setup compilation requires an installed `ISCC.exe`; the Rust installer script and resource wiring are present but not locally compiled here.

The ledger must be updated as each item is completed; it is not a claim that the migration is finished.
