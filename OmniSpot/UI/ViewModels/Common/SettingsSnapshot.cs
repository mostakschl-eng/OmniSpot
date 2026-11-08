using OmniSpot.Configuration;
using System.Windows.Input;

namespace OmniSpot.UI.ViewModels.Common;

internal sealed record SettingsSnapshot(
    Key ModifierKey,
    IReadOnlyList<ApplicationShortcutSnapshot> Applications,
    bool PulseBorderEnabled,
    AppThemeSetting Theme,
    bool OverlayEnabled,
    int OverlayShowDelayMs,
    bool OverlayKeepOpenWhileModifierHeld,
    bool PeekEnabled,
    bool DynamicModeEnabled,
    bool StatsEnabled,
    bool SpotlightEnabled = true,
    Key SpotlightModifierKey = Key.LeftAlt,
    IReadOnlyList<string>? SpotlightCustomDirectories = null)
{
    // This is needed because regular record equality would do only reference comparison for Applications collection
    public bool Equals(SettingsSnapshot? other)
    {
        if (other is null)
        {
            return false;
        }

        var customDirsEqual = (SpotlightCustomDirectories is null && other.SpotlightCustomDirectories is null) ||
                              (SpotlightCustomDirectories is not null && other.SpotlightCustomDirectories is not null &&
                               SpotlightCustomDirectories.SequenceEqual(other.SpotlightCustomDirectories, StringComparer.OrdinalIgnoreCase));

        return ModifierKey == other.ModifierKey &&
               PulseBorderEnabled == other.PulseBorderEnabled &&
               Theme == other.Theme &&
               OverlayEnabled == other.OverlayEnabled &&
               OverlayShowDelayMs == other.OverlayShowDelayMs &&
               OverlayKeepOpenWhileModifierHeld == other.OverlayKeepOpenWhileModifierHeld &&
               PeekEnabled == other.PeekEnabled &&
               DynamicModeEnabled == other.DynamicModeEnabled &&
               StatsEnabled == other.StatsEnabled &&
               SpotlightEnabled == other.SpotlightEnabled &&
               SpotlightModifierKey == other.SpotlightModifierKey &&
               customDirsEqual &&
               Applications.SequenceEqual(other.Applications);
    }

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(ModifierKey);
        hash.Add(PulseBorderEnabled);
        hash.Add(Theme);
        hash.Add(OverlayEnabled);
        hash.Add(OverlayShowDelayMs);
        hash.Add(OverlayKeepOpenWhileModifierHeld);
        hash.Add(PeekEnabled);
        hash.Add(DynamicModeEnabled);
        hash.Add(StatsEnabled);
        hash.Add(SpotlightEnabled);
        hash.Add(SpotlightModifierKey);
        return hash.ToHashCode();
    }
}

internal sealed record ApplicationShortcutSnapshot(
    Key Key,
    string ProcessPath,
    bool StartIfNotRunning,
    CycleMode CycleMode,
    ApplicationType Type = ApplicationType.Win32,
    string? Aumid = null);