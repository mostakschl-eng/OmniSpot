# C# designer-only files

The five matching C# classes in `OmniSpot/UI/ViewModels/DesignTime/` provide sample data for the WPF XAML designer. GPUI has no WPF designer, so these classes have no Rust runtime equivalent. Keep this folder as an explicit migration marker; do not create empty `.rs` copies. The C# files remain untouched while the current app is in use.
