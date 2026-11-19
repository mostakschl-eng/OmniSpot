using OmniSpot.Configuration;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.IO;
using OmniSpot.WindowDiscovery;

namespace OmniSpot;

/// <summary>Runs configuration validation in the native Rust configuration module.</summary>
internal static class RustConfigurationBackend
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public static bool MigrationsRun => NativeMigrationsRun() != 0;

    public static void MarkMigrationsRun()
    {
        if (NativeMarkMigrationsRun() == 0) throw new InvalidOperationException("Rust configuration runtime could not record migration completion.");
    }

    public static OmniSpot.Configuration.Configuration? GetCurrent()
    {
        var pointer = NativeGetCurrent(out var length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust configuration runtime could not read current configuration.");
        try
        {
            var bytes = new byte[checked((int)length)];
            Marshal.Copy(pointer, bytes, 0, bytes.Length);
            var native = JsonSerializer.Deserialize<NativeConfiguration?>(bytes, JsonOptions);
            return native is null ? null : FromNative(native);
        }
        finally { NativeFree(pointer, length); }
    }

    public static void SetCurrent(OmniSpot.Configuration.Configuration config)
    {
        var native = new NativeConfiguration((uint)config.Modifier, config.Applications.Select(ToNative).ToArray(),
            config.PulseBorderEnabled, config.Theme.ToString(), config.OverlayEnabled, config.OverlayShowDelayMs,
            config.OverlayKeepOpenWhileModifierHeld, config.PeekEnabled, config.DynamicModeEnabled,
            config.StatsEnabled, config.SpotlightEnabled, (uint)config.SpotlightModifier,
            config.SpotlightCustomDirectories?.ToArray());
        var bytes = JsonSerializer.SerializeToUtf8Bytes(native, JsonOptions);
        if (NativeSetCurrent(bytes, (nuint)bytes.Length) == 0)
            throw new InvalidOperationException("Rust configuration runtime could not cache the configuration.");
    }

    public static OmniSpot.Configuration.Configuration GetDefaults()
    {
        UIntPtr length = UIntPtr.Zero;
        var pointer = NativeDefaults(ref length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust configuration defaults returned no response.");
        try
        {
            var output = new byte[checked((int)length.ToUInt64())];
            Marshal.Copy(pointer, output, 0, output.Length);
            return FromNative(JsonSerializer.Deserialize<NativeConfiguration>(output, JsonOptions)
                ?? throw new InvalidOperationException("Rust configuration defaults returned invalid data."));
        }
        finally { NativeFree(pointer, length); }
    }

    public static IReadOnlyList<ApplicationConfiguration> MigratePackagedApps(
        IReadOnlyList<ApplicationConfiguration> applications, IReadOnlyList<PackagedPath> packagedPaths)
    {
        var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(new ConfigurationMigrationRequest(
            applications.Select(ToNative).ToArray(), packagedPaths.ToArray()), JsonOptions));
        UIntPtr length = UIntPtr.Zero;
        var pointer = NativeMigratePackagedApps(bytes, (UIntPtr)bytes.Length, ref length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust configuration migration returned no response.");
        try
        {
            var output = new byte[checked((int)length.ToUInt64())];
            Marshal.Copy(pointer, output, 0, output.Length);
            var apps = JsonSerializer.Deserialize<NativeApplication[]>(output, JsonOptions)
                ?? throw new InvalidOperationException("Rust configuration migration returned invalid data.");
            return apps.Select(app => new ApplicationConfiguration((System.Windows.Input.Key)app.Key,
                app.ProcessPath, Enum.Parse<CycleMode>(app.CycleMode), app.StartIfNotRunning,
                Enum.Parse<ApplicationType>(app.ApplicationType), app.Aumid)).ToArray();
        }
        finally { NativeFree(pointer, length); }
    }

    public static ValidationResult Validate(OmniSpot.Configuration.Configuration config) => Call<ValidationResult>(new ConfigurationRequest(
        (uint)config.Modifier, config.Applications.Select(ToNative).ToArray(), config.PulseBorderEnabled,
        config.Theme.ToString(), config.OverlayEnabled, config.OverlayShowDelayMs,
        config.OverlayKeepOpenWhileModifierHeld, config.PeekEnabled, config.DynamicModeEnabled,
        config.StatsEnabled, config.SpotlightEnabled, (uint)config.SpotlightModifier,
        config.SpotlightCustomDirectories?.ToArray()));

    public static OmniSpot.Configuration.Configuration? Sanitize(OmniSpot.Configuration.Configuration config, IPackagedAppsService packagedAppsService)
    {
        var request = new ConfigurationRequest((uint)config.Modifier, config.Applications.Select(ToNative).ToArray(),
            config.PulseBorderEnabled, config.Theme.ToString(), config.OverlayEnabled, config.OverlayShowDelayMs,
            config.OverlayKeepOpenWhileModifierHeld, config.PeekEnabled, config.DynamicModeEnabled,
            config.StatsEnabled, config.SpotlightEnabled, (uint)config.SpotlightModifier,
            config.SpotlightCustomDirectories?.ToArray());
        var familyMatches = new List<FamilyMatch>();
        var installedPaths = new List<AumidPath>();
        foreach (var app in config.Applications)
        {
            if (app.Type == ApplicationType.Win32 && GetPackageFamilyName(app.ProcessPath) is { } family &&
                packagedAppsService.GetByPackageFamilyName(family) is { } package)
            {
                familyMatches.Add(new FamilyMatch(family, package.Aumid, package.IconPath));
                if (packagedAppsService.GetInstalledPathByAumid(package.Aumid) is { } installedPath)
                    installedPaths.Add(new AumidPath(package.Aumid, installedPath));
            }
            if (app.Type == ApplicationType.Packaged && !File.Exists(app.ProcessPath) &&
                app.Aumid is { Length: > 0 } aumid && packagedAppsService.GetInstalledPathByAumid(aumid) is { } path)
            {
                installedPaths.Add(new AumidPath(aumid, path));
            }
        }
        var sanitizeRequest = new SanitizeRequest(request, familyMatches.ToArray(), installedPaths.ToArray());
        var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(sanitizeRequest, JsonOptions));
        UIntPtr length = UIntPtr.Zero;
        var pointer = NativeSanitize(bytes, (UIntPtr)bytes.Length, ref length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust configuration sanitizer returned no response.");
        try
        {
            var output = new byte[checked((int)length.ToUInt64())];
            Marshal.Copy(pointer, output, 0, output.Length);
            var result = JsonSerializer.Deserialize<NativeConfiguration?>(output, JsonOptions);
            return result is null ? null : FromNative(result);
        }
        finally { NativeFree(pointer, length); }
    }

    public static ValidationResult ValidateApplication(IApplicationConfiguration app) => Call<ValidationResult>(ToNative(app));

    private static NativeApplication ToNative(IApplicationConfiguration app) => new(
        (uint)app.Key, app.ProcessPath, app.CycleMode.ToString(), app.StartIfNotRunning,
        app.Type.ToString(), app.Aumid);

    private static OmniSpot.Configuration.Configuration FromNative(NativeConfiguration result) => new(
        (System.Windows.Input.Key)result.Modifier,
        result.Applications.Select(app => new ApplicationConfiguration((System.Windows.Input.Key)app.Key,
            app.ProcessPath, Enum.Parse<CycleMode>(app.CycleMode), app.StartIfNotRunning,
            Enum.Parse<ApplicationType>(app.ApplicationType), app.Aumid)).ToList(),
        result.PulseBorderEnabled, Enum.Parse<AppThemeSetting>(result.Theme), result.OverlayEnabled,
        result.OverlayShowDelayMs, result.OverlayKeepOpenWhileModifierHeld, result.PeekEnabled,
        result.DynamicModeEnabled, result.StatsEnabled, result.SpotlightEnabled,
        (System.Windows.Input.Key)result.SpotlightModifier, result.SpotlightCustomDirectories);

    private static T Call<T>(object request)
    {
        var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(request, JsonOptions));
        UIntPtr length = UIntPtr.Zero;
        var pointer = request is ConfigurationRequest
            ? NativeValidate(bytes, (UIntPtr)bytes.Length, ref length)
            : NativeValidateApplication(bytes, (UIntPtr)bytes.Length, ref length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust configuration validator returned no response.");
        try
        {
            var output = new byte[checked((int)length.ToUInt64())];
            Marshal.Copy(pointer, output, 0, output.Length);
            var response = JsonSerializer.Deserialize<NativeValidationResult>(output, JsonOptions)
                ?? throw new InvalidOperationException("Rust configuration validator returned invalid data.");
            if (response.Status == "success") return (T)(object)ValidationResult.Success;
            return (T)(object)(response.Process is { } process
                ? ValidationResult.Error(process, response.Message ?? "Configuration is invalid.")
                : ValidationResult.Error(response.Message ?? "Configuration is invalid."));
        }
        finally { NativeFree(pointer, length); }
    }

    private static string? GetPackageFamilyName(string path)
    {
        var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(path, JsonOptions));
        UIntPtr length = UIntPtr.Zero;
        var pointer = NativePackageFamily(bytes, (UIntPtr)bytes.Length, ref length);
        if (pointer == IntPtr.Zero) return null;
        try
        {
            var output = new byte[checked((int)length.ToUInt64())];
            Marshal.Copy(pointer, output, 0, output.Length);
            return JsonSerializer.Deserialize<string?>(output, JsonOptions);
        }
        finally { NativeFree(pointer, length); }
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_configuration_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeValidate(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_configuration_validate_application", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeValidateApplication(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_configuration_sanitize", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeSanitize(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_configuration_package_family", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativePackageFamily(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, UIntPtr length);

    private sealed record ConfigurationRequest(uint Modifier, NativeApplication[] Applications,
        bool PulseBorderEnabled, string Theme, bool OverlayEnabled, int OverlayShowDelayMs,
        bool OverlayKeepOpenWhileModifierHeld, bool PeekEnabled, bool DynamicModeEnabled, bool StatsEnabled,
        bool SpotlightEnabled, uint SpotlightModifier, string[]? SpotlightCustomDirectories);
    private sealed record NativeApplication(uint Key, string ProcessPath, string CycleMode,
        bool StartIfNotRunning, string ApplicationType, string? Aumid);
    private sealed record NativeValidationResult(string Status, string? Process, string? Message);
    private sealed record NativeConfiguration(uint Modifier, NativeApplication[] Applications, bool PulseBorderEnabled,
        string Theme, bool OverlayEnabled, int OverlayShowDelayMs, bool OverlayKeepOpenWhileModifierHeld,
        bool PeekEnabled, bool DynamicModeEnabled, bool StatsEnabled, bool SpotlightEnabled,
        uint SpotlightModifier, string[]? SpotlightCustomDirectories);
    private sealed record SanitizeRequest(ConfigurationRequest Configuration, FamilyMatch[] FamilyMatches, AumidPath[] InstalledPaths);
    private sealed record FamilyMatch(string FamilyName, string Aumid, string? IconPath);
    private sealed record AumidPath(string Aumid, string Path);
    internal sealed record PackagedPath(string ProcessPath, string Aumid);
    private sealed record ConfigurationMigrationRequest(NativeApplication[] Applications, PackagedPath[] PackagedPaths);
    [DllImport(LibraryName, EntryPoint = "omnispot_configuration_defaults", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeDefaults(ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_configuration_manager_get", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeGetCurrent(out UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_configuration_manager_set", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeSetCurrent(byte[] request, UIntPtr requestLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_configuration_manager_migrations_run", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeMigrationsRun();
    [DllImport(LibraryName, EntryPoint = "omnispot_configuration_manager_mark_migrations_run", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeMarkMigrationsRun();
    [DllImport(LibraryName, EntryPoint = "omnispot_configuration_migrate_packaged_apps", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeMigratePackagedApps(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);
}
