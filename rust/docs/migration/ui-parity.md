# OmniSpot Rust migration parity

This ledger records behavior that is intentionally tracked while the Rust runtime replaces the C# runtime.

## Implemented

- Configuration JSON load, validation, atomic save, live Save & Apply updates, and packaged-app migration.
- Atomic replacement for settings, stats, Spotlight usage, and app-registry JSON; a failure leaves the prior destination intact.
- Separate read-only LiteDB exporter plus `--import-legacy` validation and profile fallback. Cross-language synthetic export/import, data-reader fallback, integrity checks, and idempotent reimport pass; the user's real databases have not been read or changed.
- Global keyboard hook, modifier state machine, dynamic mode, synthetic passthrough, overlay timer, peek restore, and clean hook shutdown.
- Win32 window enumeration, placement/style/cloaking filtering, process path and friendly-name inspection, elevation detection, packaged-app discovery, switching, and launch.
- Overlay grouping, window-number actions, static/dynamic launch actions, native tray menu, and application lifecycle.
- Tray-first startup and explicit Exit behavior; closing a Rust window keeps the resident app running.
- Spotlight app/file search, usage ranking, keyboard navigation, mouse activation, launch, and packaged-app catalog.
- Stats collection, dashboard metrics, JSON persistence, historical bucket retention, immediate enable/disable, and confirmed reset.
- Stats JSON is persisted as schema v2 under the Storage module. Existing unversioned Rust stats are treated as v1 and upgraded; the C# v2 default-date cleanup also runs when importing legacy data. Newer unknown schemas are left untouched and stats collection stays disabled to avoid overwriting them.
- Stats use local `yyyy-MM-dd` dates, the C# rolling 30-day metric window, lifetime saved-time totals, productivity streak threshold, saved-time formatting, percentage rounding, latency empty handling, and Alt+Tab sober-streak threshold.
- Rust General settings disables the pulse-border toggle unless Windows reports build 22000 or later, matching the C# capability gate.
- Rust release build and 59 passing unit tests; the C# Release suite passes 436 tests. The GNU Rust target lacks `gcc.exe`/`dlltool.exe`, but the installed MSVC Rust toolchain builds and tests successfully. Strict Clippy passes with `-D warnings`; Cargo reports a future-compatibility notice in third-party `proc-macro-error2`.

## Remaining parity work

- Settings visuals and interactions are not yet an exact match. Theme selection, page details, warning behavior, and spotlight window lifecycle need a full side-by-side review.
- Statistics calculations now include C# maintenance/session warmup thresholds, most-peeked app, friendly configured names, and stale-shortcut selection. Selecting the stale-shortcut action removes the app, saves, and dispatches updated configuration. The dashboard still lacks the complete C# card styling, progress-ring/tooltip presentation, and several detailed metric cards.
- Windows shell icon extraction/rasterization for Win32 executables and file results is not yet equivalent to WPF `IconExtractor`.
- `SystemIndex` query shape, exclusion rules, ordering, 15-result limit, and local file URL decoding are ported. The COM/OLE DB provider and asynchronous query path compile and are unit-tested, but have not been validated against a populated Windows Search index on this machine.
- Warning overlay dismissal, focus/deactivation behavior, and the real tray-first app flow still need interactive validation on Windows.
- Inno Setup compilation requires an installed `ISCC.exe`; the Rust installer script and resource wiring are present but not locally compiled here.
- Current Rust verification: MSVC `cargo fmt --check`, 59 unit tests, strict Clippy (`-D warnings`), and Release build pass. Cargo's only remaining warning is the transitive `proc-macro-error2` future-incompatibility notice through GPUI.

The ledger must be updated as each item is completed; it is not a claim that the migration is finished.
