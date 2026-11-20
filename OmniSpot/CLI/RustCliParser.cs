using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;

namespace OmniSpot.CLI;

internal static class RustCliParser
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public static ParseResult Parse(string[] args, CliBuilder builder)
    {
        var request = new Request(args, builder.Commands.Select(x => x.Name).ToArray(),
            builder.FlagOptions.Select(x => x.Name).ToArray(),
            builder.ValuedOptions.Select(x => x.Name).ToArray());
        var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(request, JsonOptions));
        UIntPtr length = UIntPtr.Zero;
        var pointer = NativeParse(bytes, (UIntPtr)bytes.Length, ref length);
        if (pointer == IntPtr.Zero)
        {
            throw new InvalidOperationException("Rust CLI parser returned no response.");
        }

        try
        {
            var responseBytes = new byte[checked((int)length.ToUInt64())];
            Marshal.Copy(pointer, responseBytes, 0, responseBytes.Length);
            return JsonSerializer.Deserialize<ParseResult>(responseBytes, JsonOptions)
                ?? throw new InvalidOperationException("Rust CLI parser returned an invalid response.");
        }
        finally
        {
            NativeFree(pointer, length);
        }
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_cli_parse", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeParse(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);

    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, UIntPtr length);

    private sealed record Request(string[] Args, string[] Commands, string[] Flags, string[] ValuedOptions);
    internal sealed record ParseResult(string Status, string? Command, bool Debug, bool Trace, string? Error,
        string[] MatchedFlags, string[][] ValuedValues, string[] UnknownArguments);
}
