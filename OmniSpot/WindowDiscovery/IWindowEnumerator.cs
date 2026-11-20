namespace OmniSpot.WindowDiscovery;

internal interface IWindowEnumerator
{
    List<ApplicationWindow> GetWindows();

    /// <summary>
    /// Get cached windows if available - 10 seconds cache
    /// </summary>
    List<ApplicationWindow> GetCachedWindows();

    ApplicationWindow? GetCurrentWindow();

    /// <summary>
    /// Gets all windows including windows irrelevant (IsValidWindow = false) for OmniSpot
    /// </summary>
    List<ApplicationWindow> GetAllWindows();
    void LogAllWindows();
}
