using OmniSpot.UI.ViewModels;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Effects;
using Wpf.Ui.Appearance;
using Forms = System.Windows.Forms;

namespace OmniSpot.UI.Windows;

public partial class AppOverlayWindow : Window
{
    public AppOverlayWindow(AppOverlayViewModel viewModel)
    {
        InitializeComponent();
        DataContext = viewModel;

        Loaded += OnLoaded;
        SizeChanged += OnSizeChanged;
        Closed += OnClosed;

        ApplicationThemeManager.Changed += OnThemeChanged;
    }

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        ApplyShadowForTheme();
        PositionWindow();
    }

    private void OnSizeChanged(object sender, SizeChangedEventArgs e) => PositionWindow();

    private void OnClosed(object? sender, EventArgs e) =>
        ApplicationThemeManager.Changed -= OnThemeChanged;

    private void OnThemeChanged(ApplicationTheme theme, System.Windows.Media.Color _) => ApplyShadowForTheme();

    private void OnAppItemClick(object sender, MouseButtonEventArgs e)
    {
        if (sender is FrameworkElement { DataContext: OverlayAppItem item } && DataContext is AppOverlayViewModel viewModel)
        {
            viewModel.OpenAppCommand.Execute(item);
            e.Handled = true;
        }
    }

    private void ApplyShadowForTheme()
    {
        var isDark = ApplicationThemeManager.GetAppTheme() == ApplicationTheme.Dark;

        CardBorder.Effect = isDark
            ? new DropShadowEffect { BlurRadius = 20, ShadowDepth = 0, Opacity = 0.25, Color = Colors.White }
            : new DropShadowEffect { BlurRadius = 24, ShadowDepth = 8, Direction = 270, Opacity = 0.4, Color = Colors.Black };
    }

    private void PositionWindow()
    {
        var cursor = Forms.Cursor.Position;
        var workArea = Forms.Screen.FromPoint(cursor).WorkingArea;
        var dpi = VisualTreeHelper.GetDpi(this);
        var leftEdge = workArea.Left / dpi.DpiScaleX;
        var rightEdge = workArea.Right / dpi.DpiScaleX;
        var topEdge = workArea.Top / dpi.DpiScaleY;
        var bottomEdge = workArea.Bottom / dpi.DpiScaleY;
        var cursorX = cursor.X / dpi.DpiScaleX;
        var cursorY = cursor.Y / dpi.DpiScaleY;
        var left = Math.Clamp(cursorX - ActualWidth / 2, leftEdge, Math.Max(leftEdge, rightEdge - ActualWidth));
        var top = cursorY + 16;

        if (top + ActualHeight > bottomEdge)
        {
            top = cursorY - ActualHeight - 16;
        }

        Left = left;
        Top = Math.Clamp(top, topEdge, Math.Max(topEdge, bottomEdge - ActualHeight));
    }

    public void PositionAtCursor() => PositionWindow();
}
