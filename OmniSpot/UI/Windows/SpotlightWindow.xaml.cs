using System.Windows;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Threading;
using System.Runtime.InteropServices;
using OmniSpot.Spotlight;
using OmniSpot.UI.ViewModels;
using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.UI.Input.KeyboardAndMouse;
using Wpf.Ui.Controls;
using Forms = System.Windows.Forms;

namespace OmniSpot.UI.Windows;

internal partial class SpotlightWindow : FluentWindow
{
    private readonly SpotlightViewModel _viewModel;
    private readonly DispatcherTimer _foregroundTimer;
    private long _showTimestamp;

    public SpotlightWindow(SpotlightViewModel viewModel)
    {
        InitializeComponent();
        _viewModel = viewModel;
        DataContext = viewModel;
        _foregroundTimer = new DispatcherTimer(TimeSpan.FromMilliseconds(200), DispatcherPriority.Background, OnForegroundTimerTick, Dispatcher);

        _viewModel.RequestClose += Hide;
        Deactivated += OnWindowDeactivated;
        Loaded += OnLoaded;
        IsVisibleChanged += OnIsVisibleChanged;
        Closed += OnClosed;
        PreviewKeyDown += OnPreviewKeyDown;

    }

    private void OnWindowDeactivated(object? sender, EventArgs e)
    {
        if (System.Diagnostics.Stopwatch.GetElapsedTime(_showTimestamp).TotalMilliseconds < 350)
        {
            return;
        }
        Hide();
    }

    public void FocusSearchBox()
    {
        Dispatcher.BeginInvoke(() => FocusSearchBoxCore(), DispatcherPriority.Input);
    }

    private void FocusSearchBoxCore()
    {
        if (!IsVisible)
        {
            return;
        }

        var handle = new HWND(new WindowInteropHelper(this).Handle);
        INPUT input = new() { type = INPUT_TYPE.INPUT_MOUSE, Anonymous = new() { mi = new() } };
        PInvoke.SendInput([input], Marshal.SizeOf<INPUT>());
        Activate();
        PInvoke.SetForegroundWindow(handle);
        SearchBox.Focus();
        Keyboard.Focus(SearchBox);
        SearchBox.SelectAll();
    }

    private void OnIsVisibleChanged(object sender, DependencyPropertyChangedEventArgs e)
    {
        if (IsVisible)
        {
            _showTimestamp = System.Diagnostics.Stopwatch.GetTimestamp();
            _foregroundTimer.Start();
            Dispatcher.BeginInvoke(() => FocusSearchBoxCore(), DispatcherPriority.Input);
        }
        else
        {
            _foregroundTimer.Stop();
        }
    }

    private void OnForegroundTimerTick(object? sender, EventArgs e)
    {
        if (!IsVisible || System.Diagnostics.Stopwatch.GetElapsedTime(_showTimestamp).TotalMilliseconds < 350)
        {
            return;
        }

        var spotlightHandle = new HWND(new WindowInteropHelper(this).Handle);
        if (PInvoke.GetForegroundWindow() != spotlightHandle)
        {
            Hide();
        }
    }

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        PositionWindow();
    }

    private void OnClosed(object? sender, EventArgs e)
    {
        _foregroundTimer.Stop();
    }

    private void PositionWindow()
    {
        var cursor = Forms.Cursor.Position;
        var workArea = Forms.Screen.FromPoint(cursor).WorkingArea;
        var dpi = VisualTreeHelper.GetDpi(this);
        var leftEdge = workArea.Left / dpi.DpiScaleX;
        var topEdge = workArea.Top / dpi.DpiScaleY;
        var screenWidth = workArea.Width / dpi.DpiScaleX;
        var screenHeight = workArea.Height / dpi.DpiScaleY;

        Left = leftEdge + (screenWidth - ActualWidth) / 2;
        Top = topEdge + (screenHeight * 0.20);
    }

    private void OnPreviewKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Escape)
        {
            Hide();
            e.Handled = true;
        }
        else if (e.Key == Key.Down)
        {
            _viewModel.MoveSelection(1);
            e.Handled = true;
        }
        else if (e.Key == Key.Up)
        {
            _viewModel.MoveSelection(-1);
            e.Handled = true;
        }
        else if (e.Key == Key.Enter)
        {
            _viewModel.OpenItemCommand.Execute(null);
            e.Handled = true;
        }
    }

    private void OnItemClicked(object sender, MouseButtonEventArgs e)
    {
        if (sender is FrameworkElement { DataContext: SearchResult item })
        {
            _viewModel.OpenItemCommand.Execute(item);
            e.Handled = true;
        }
    }
}
