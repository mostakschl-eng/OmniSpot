using System.Diagnostics.CodeAnalysis;
using System.IO;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace OmniSpot.WindowDiscovery;

public class IconExtractor
{
    private readonly Dictionary<string, ImageSource> _images = new();
    private readonly object _lock = new();
    private ImageSource? _defaultIcon;

    public ImageSource? GetByProcessPath(string processPath)
    {
        if (string.IsNullOrWhiteSpace(processPath))
        {
            return null;
        }

        lock (_lock)
        {
            if (_images.TryGetValue(processPath, out var imageSource))
            {
                return imageSource;
            }

            var icon = LoadIconFromExe(processPath);
            if (icon is not null)
            {
                _images[processPath] = icon;
                return icon;
            }
        }

        return null;
    }

    public ImageSource? GetByIconPath(string? iconPath)
    {
        if (string.IsNullOrWhiteSpace(iconPath))
        {
            return null;
        }

        lock (_lock)
        {
            if (_images.TryGetValue(iconPath, out var imageSource))
            {
                return imageSource;
            }

            try
            {
                using var image = Image.FromFile(iconPath);
                var extractedImage = ConvertToImageSource(image);
                _images[iconPath] = extractedImage;
                return extractedImage;
            }
            catch
            {
                return null;
            }
        }
    }

    public ImageSource GetDefaultIcon()
    {
        lock (_lock)
        {
            _defaultIcon ??= GetByProcessPath(
                Environment.GetFolderPath(Environment.SpecialFolder.System) + "\\shell32.dll")!;
        }

        return _defaultIcon;
    }

    private static BitmapImage? LoadIconFromExe(string path)
    {
        try
        {
            using var icon = Icon.ExtractAssociatedIcon(path);
            return icon != null ? ConvertToImageSource(icon.ToBitmap()) : null;
        }
        catch
        {
            return null;
        }
    }

    private static BitmapImage ConvertToImageSource(Image image)
    {
        using var stream = new MemoryStream();
        image.Save(stream, System.Drawing.Imaging.ImageFormat.Png);
        stream.Seek(0, SeekOrigin.Begin);

        var bitmap = new BitmapImage();
        bitmap.BeginInit();
        bitmap.StreamSource = stream;
        bitmap.CacheOption = BitmapCacheOption.OnLoad;
        bitmap.EndInit();
        bitmap.Freeze();
        return bitmap;
    }
}
