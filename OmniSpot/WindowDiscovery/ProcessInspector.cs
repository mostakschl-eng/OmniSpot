using OmniSpot;

namespace OmniSpot.WindowDiscovery;

/// <summary>Managed adapter for Rust-owned process inspection.</summary>
internal sealed class ProcessInspector(RustProcessInspectorBackend rust) : IProcessInspector
{
    public bool NeedsElevation(uint processId) => rust.NeedsElevation(processId);

    public Task<bool> WaitForPotentialElevation(string processPath, int timeoutMs = 2000) =>
        rust.WaitForPotentialElevation(processPath, timeoutMs);

    public bool IsWindowsExecutable(string path) => rust.IsWindowsExecutable(path);

    public string GetProcessDisplayName(string path) => rust.GetProcessDisplayName(path);
}
