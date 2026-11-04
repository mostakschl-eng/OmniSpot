using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using OmniSpot.Spotlight;
using System.Collections.ObjectModel;
using System.Diagnostics;
using System.IO;

namespace OmniSpot.UI.ViewModels;

internal partial class SpotlightViewModel : ObservableObject
{
    private readonly ISpotlightSearchService _searchService;
    private CancellationTokenSource? _searchCts;

    [ObservableProperty]
    private string _query = string.Empty;

    [ObservableProperty]
    private ObservableCollection<SearchResult> _suggestions = [];

    [ObservableProperty]
    private ObservableCollection<SearchResult> _results = [];

    [ObservableProperty]
    private int _selectedIndex = -1;

    [ObservableProperty]
    private bool _isSearching;

    [ObservableProperty]
    private bool _hasResults;

    [ObservableProperty]
    private bool _showSuggestions = true;

    [ObservableProperty]
    private bool _showNoResults;

    public event Action? RequestClose;

    public SpotlightViewModel(ISpotlightSearchService searchService)
    {
        _searchService = searchService;
    }

    public void ResetAndLoad()
    {
        _searchCts?.Cancel();
        Query = string.Empty;
        Results.Clear();
        SelectedIndex = -1;
        ShowSuggestions = true;
        HasResults = false;
        IsSearching = false;
        ShowNoResults = false;

        _ = LoadSuggestionsAsync();
    }

    private async Task LoadSuggestionsAsync()
    {
        try
        {
            var suggestions = await _searchService.GetSuggestionsAsync();
            Suggestions = new ObservableCollection<SearchResult>(suggestions);
        }
        catch
        {
            // Ignore suggestions error
        }
    }

    async partial void OnQueryChanged(string value)
    {
        _searchCts?.Cancel();
        _searchCts = new CancellationTokenSource();
        var ct = _searchCts.Token;

        if (string.IsNullOrWhiteSpace(value))
        {
            Results.Clear();
            ShowSuggestions = true;
            HasResults = false;
            SelectedIndex = -1;
            IsSearching = false;
            ShowNoResults = false;
            return;
        }

        ShowSuggestions = false;
        IsSearching = true;
        ShowNoResults = false;

        try
        {
            // Debounce 60ms
            await Task.Delay(60, ct);

            var items = await _searchService.SearchAsync(value, ct);
            if (ct.IsCancellationRequested)
            {
                return;
            }

            Results = new ObservableCollection<SearchResult>(items);
            HasResults = Results.Count > 0;
            ShowNoResults = !HasResults;
            SelectedIndex = HasResults ? 0 : -1;
        }
        catch (OperationCanceledException)
        {
            // Cancelled
        }
        finally
        {
            if (!ct.IsCancellationRequested)
            {
                IsSearching = false;
            }
        }
    }

    public void MoveSelection(int delta)
    {
        var collection = ShowSuggestions ? Suggestions : Results;
        if (collection.Count == 0)
        {
            return;
        }

        var count = collection.Count;
        SelectedIndex = (SelectedIndex + delta + count) % count;
    }

    [RelayCommand]
    public void OpenItem(SearchResult? item)
    {
        var target = item ?? (ShowSuggestions ? Suggestions.ElementAtOrDefault(SelectedIndex) : Results.ElementAtOrDefault(SelectedIndex));
        if (target is null)
        {
            return;
        }

        LaunchTarget(target);
        RequestClose?.Invoke();
    }

    private static void LaunchTarget(SearchResult item)
    {
        try
        {
            if (item.Type == SearchResultType.App && !item.Target.EndsWith(".exe", StringComparison.OrdinalIgnoreCase) && !File.Exists(item.Target))
            {
                Process.Start("explorer.exe", $"shell:AppsFolder\\{item.Target}");
            }
            else
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = item.Target,
                    UseShellExecute = true
                });
            }
        }
        catch
        {
            // Log or fallback
        }
    }
}
