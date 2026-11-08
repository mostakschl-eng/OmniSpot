using OmniSpot.WindowDiscovery;
using System.Collections.Concurrent;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace OmniSpot.Spotlight;

internal sealed class SpotlightIconService(IconExtractor iconExtractor)
{
    private static readonly string SettingsAppPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.Windows),
        "ImmersiveControlPanel",
        "SystemSettings.exe");

    private static readonly Guid ShellItemImageFactoryGuid = new("bcc18b79-ba16-442f-80c4-8a59c30c463b");

    private readonly ConcurrentDictionary<string, ImageSource> _extensionCache = new(StringComparer.OrdinalIgnoreCase);
    private readonly ConcurrentDictionary<string, ImageSource> _targetCache = new(StringComparer.OrdinalIgnoreCase);

    public ImageSource GetIcon(string target, SearchResultType type)
    {
        if (string.IsNullOrWhiteSpace(target))
        {
            return iconExtractor.GetDefaultIcon();
        }

        // 1. Settings deep-links
        if (target.StartsWith("ms-settings:", StringComparison.OrdinalIgnoreCase))
        {
            return _targetCache.GetOrAdd("ms-settings:", _ =>
                ExtractShellItemIcon(SettingsAppPath, 48)
                ?? iconExtractor.GetByProcessPath(SettingsAppPath)
                ?? iconExtractor.GetDefaultIcon());
        }

        // 2. Folders
        if (type == SearchResultType.Folder || Directory.Exists(target))
        {
            return _targetCache.GetOrAdd("::folder::", _ =>
                ExtractShellItemIcon(Environment.GetFolderPath(Environment.SpecialFolder.Windows), 48)
                ?? iconExtractor.GetDefaultIcon());
        }

        // 3. Apps: packaged apps (AUMID / shell:AppsFolder) or regular executable paths
        if (type == SearchResultType.App)
        {
            return _targetCache.GetOrAdd(target, ResolveAppIcon);
        }

        // 3. Files: cache by file extension to avoid repeated shell lookups
        var extension = Path.GetExtension(target);
        if (!string.IsNullOrEmpty(extension))
        {
            if (_extensionCache.TryGetValue(extension, out var cached))
            {
                return cached;
            }

            if (File.Exists(target))
            {
                var icon = ExtractShellItemIcon(target, 48) ?? iconExtractor.GetByProcessPath(target);
                if (icon is not null)
                {
                    _extensionCache[extension] = icon;
                    return icon;
                }
            }
        }

        // Direct file lookup fallback
        if (File.Exists(target))
        {
            var icon = ExtractShellItemIcon(target, 48) ?? iconExtractor.GetByProcessPath(target);
            if (icon is not null)
            {
                return icon;
            }
        }

        return iconExtractor.GetDefaultIcon();
    }

    private ImageSource ResolveAppIcon(string path)
    {
        // AUMID (e.g. OpenAI.Codex_2p2nqsd0c76g0!App) or shell:AppsFolder item
        if (path.Contains('!') || path.StartsWith("shell:AppsFolder\\", StringComparison.OrdinalIgnoreCase) || !File.Exists(path))
        {
            var parsingName = path.StartsWith("shell:AppsFolder\\", StringComparison.OrdinalIgnoreCase)
                ? path
                : $"shell:AppsFolder\\{path}";

            var shellIcon = ExtractShellItemIcon(parsingName, 48);
            if (shellIcon is not null)
            {
                return shellIcon;
            }
        }

        // Direct executable or file path
        if (File.Exists(path))
        {
            var shellIcon = ExtractShellItemIcon(path, 48);
            if (shellIcon is not null)
            {
                return shellIcon;
            }

            var exeIcon = iconExtractor.GetByProcessPath(path);
            if (exeIcon is not null)
            {
                return exeIcon;
            }
        }

        return iconExtractor.GetDefaultIcon();
    }

    private static ImageSource? ExtractShellItemIcon(string parsingName, int iconSize)
    {
        try
        {
            var hr = SHCreateItemFromParsingName(parsingName, IntPtr.Zero, ShellItemImageFactoryGuid, out var factory);
            if (hr != 0 || factory is null)
            {
                return null;
            }

            hr = factory.GetImage(new SIZE(iconSize, iconSize), 0x0, out var hbitmap);
            if (hr != 0 || hbitmap == IntPtr.Zero)
            {
                return null;
            }

            try
            {
                var bitmapSource = Imaging.CreateBitmapSourceFromHBitmap(
                    hbitmap,
                    IntPtr.Zero,
                    Int32Rect.Empty,
                    BitmapSizeOptions.FromEmptyOptions());
                bitmapSource.Freeze();
                return bitmapSource;
            }
            finally
            {
                DeleteObject(hbitmap);
            }
        }
        catch
        {
            return null;
        }
    }

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = true)]
    private static extern int SHCreateItemFromParsingName(
        [MarshalAs(UnmanagedType.LPWStr)] string pszPath,
        IntPtr pbc,
        [MarshalAs(UnmanagedType.LPStruct)] Guid riid,
        out IShellItemImageFactory ppv);

    [DllImport("gdi32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DeleteObject(IntPtr hObject);

    [ComImport]
    [Guid("bcc18b79-ba16-442f-80c4-8a59c30c463b")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IShellItemImageFactory
    {
        [PreserveSig]
        int GetImage(
            [In, MarshalAs(UnmanagedType.Struct)] SIZE size,
            [In] int flags,
            out IntPtr phbm);
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct SIZE(int cx, int cy)
    {
        public int cx = cx;
        public int cy = cy;
    }
}