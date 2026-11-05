# Rust source layout

- Keep Rust application source under `OmniSpot/` with the same folders and file stems as the C# `OmniSpot/` project. Use `#[path = "..."]` in `mod.rs` when a capitalized C# filename needs a Rust module name.
- Convert one existing C# file or related view group at a time. Preserve its UI states and actions; record incomplete parity in `docs/migration/ui-parity.md`.
- Match each C# source file's folder and stem when a Rust runtime equivalent exists. A XAML view and its `.xaml.cs` code-behind become one matching `.rs` view module. WPF designer-only files are documented under `UI/ViewModels/DesignTime/README.md` and are not copied as empty Rust modules.
- Keep Rust build outputs under `target/`. Do not read or write the C# app's databases or binaries while working on UI.
- Root `AGENTS.md` Git rules apply: never stage, commit, push, or change Git tracking.
