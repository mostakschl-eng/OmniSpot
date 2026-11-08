using System.Windows.Media;

namespace OmniSpot.Spotlight;

public enum SearchResultType
{
    App,
    File,
    Folder
}

public sealed record SearchResult(
    string Title,
    string Subtitle,
    string Target,
    SearchResultType Type,
    ImageSource? Icon = null,
    int Score = 0);