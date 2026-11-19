using Microsoft.Extensions.Logging;
using System.Runtime.InteropServices;

namespace OmniSpot.Startup;

/// <summary>Thin UI-facing adapter for native Rust Startup shortcut operations.</summary>
internal sealed class AutoStart(ILogger<AutoStart> logger)
{
    private const string LibraryName = "omnispot_spotlight_native";

    public bool IsEnabled() => NativeIsEnabled() != 0;

    public bool RemoveShortcut()
    {
        var succeeded = NativeRemove() != 0;
        if (succeeded)
        {
            logger.LogInformation("Startup shortcut removed");
        }
        else
        {
            logger.LogError("Native Rust code failed to remove the startup shortcut");
        }
        return succeeded;
    }

    public bool CreateShortcut()
    {
        var succeeded = NativeCreate() != 0;
        if (succeeded)
        {
            logger.LogInformation("Startup shortcut created");
        }
        else
        {
            logger.LogError("Native Rust code failed to create the startup shortcut");
        }
        return succeeded;
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_autostart_is_enabled", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeIsEnabled();

    [DllImport(LibraryName, EntryPoint = "omnispot_autostart_remove", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeRemove();

    [DllImport(LibraryName, EntryPoint = "omnispot_autostart_create", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeCreate();
}
