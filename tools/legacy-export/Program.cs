using System;
using System.Buffers;
using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Windows.Input;
using LiteDB;
using JsonSerializer = System.Text.Json.JsonSerializer;

internal static class Program
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        WriteIndented = true
    };
    private static readonly JsonSerializerOptions DigestOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower
    };

    private static int Main(string[] args)
    {
        try
        {
            if (args is ["--self-test"] || args is ["--self-test-export", _])
            {
                SelfTest(args.Length == 2 ? args[1] : null);
                Console.WriteLine("Synthetic export self-check passed.");
                return 0;
            }
            if (args.Length != 4 || args[0] != "--profile" || args[2] != "--out")
            {
                Console.Error.WriteLine("Usage: OmniSpot.LegacyExport --profile <profile-directory> --out <export.json>");
                return 2;
            }

            var profile = Path.GetFullPath(args[1]);
            var output = Path.GetFullPath(args[3]);
            var settingsPath = Path.Combine(profile, "settings.db");
            var statsPath = Path.Combine(profile, "stats.db");
            if (!File.Exists(settingsPath))
                throw new FileNotFoundException("The selected profile has no settings.db.");
            if (File.Exists(output) || Path.GetFullPath(output).Equals(Path.GetFullPath(settingsPath), StringComparison.OrdinalIgnoreCase)
                || Path.GetFullPath(output).Equals(Path.GetFullPath(statsPath), StringComparison.OrdinalIgnoreCase))
                throw new IOException("The export destination already exists or points at a source database.");
            var bundle = Seal(Export(profile));
            var json = JsonSerializer.Serialize(bundle, JsonOptions);
            Directory.CreateDirectory(Path.GetDirectoryName(output)!);
            using (var stream = new FileStream(output, FileMode.CreateNew, FileAccess.Write, FileShare.None))
                stream.Write(Encoding.UTF8.GetBytes(json));
            Console.WriteLine($"Exported {bundle.Stats.Buckets.Count} stats buckets; source databases were opened read-only.");
            return 0;
        }
        catch (Exception error)
        {
            Console.Error.WriteLine(error.Message);
            return 1;
        }
    }

    private static Bundle Export(string profile)
    {
        BsonMapper.Global.EnumAsInteger = true;
        BsonMapper.Global.RegisterType<DateOnly>(
            serialize: date => date.ToString("yyyy-MM-dd"),
            deserialize: bson => DateOnly.ParseExact(bson.AsString, "yyyy-MM-dd"));

        var settingsPath = Path.Combine(profile, "settings.db");
        var statsPath = Path.Combine(profile, "stats.db");
        var settings = ReadSettings(settingsPath);
        var buckets = ReadBuckets(statsPath);
        var appRegistry = ReadAppRegistry(statsPath);
        var usage = ReadSpotlightUsage(settingsPath);
        var identity = SourceIdentity(settingsPath, statsPath);
        return new Bundle(1, "1.0.2", MigrationVersion(settingsPath), MigrationVersion(statsPath),
            identity, DateTimeOffset.UtcNow, settings,
            new StatsDto(buckets), appRegistry, new UsageDto(usage), "");
    }

    private static SettingsDto ReadSettings(string path)
    {
        if (!File.Exists(path)) return SettingsDto.Default;
        using var db = OpenReadOnly(path);
        var row = db.GetCollection<BsonDocument>("settings").FindById(1);
        if (row is null) return SettingsDto.Default;
        var apps = Array(row, "Applications").Select(value =>
        {
            var app = value.AsDocument;
            var key = ReadKey(app, "Key", Key.None);
            return new ApplicationDto(key.Name, key.Value, Text(app, "ProcessPath"),
                EnumName<CycleMode>(Number(app, "CycleMode")), Boolean(app, "StartIfNotRunning"),
                EnumName<ApplicationType>(Number(app, "Type")), NullableText(app, "Aumid"));
        }).ToList();
        var modifier = ReadKey(row, "Modifier", Key.RightCtrl);
        var spotlightModifier = ReadKey(row, "SpotlightModifier", Key.LeftAlt);
        return new SettingsDto(2, modifier.Name, modifier.Value, apps,
            Boolean(row, "PulseBorderEnabled"), EnumName<AppThemeSetting>(Number(row, "Theme")),
            Boolean(row, "OverlayEnabled"), Number(row, "OverlayShowDelayMs", 1000),
            Boolean(row, "OverlayKeepOpenWhileModifierHeld"), Boolean(row, "PeekEnabled"),
            Boolean(row, "DynamicModeEnabled", true), Boolean(row, "StatsEnabled"),
            Boolean(row, "SpotlightEnabled", true), spotlightModifier.Name, spotlightModifier.Value);
    }

    private static List<DailyBucketDto> ReadBuckets(string path)
    {
        if (!File.Exists(path)) return [];
        using var db = OpenReadOnly(path);
        return db.GetCollection<BsonDocument>("daily_buckets").FindAll()
            .Select(row => new DailyBucketDto(
                Date(row, "Date"), Number(row, "TotalSwitches"), Number(row, "TotalTimeSavedMs"),
                Number(row, "TotalPeeks"), ReadUsage(row, "StaticAppUsage"), ReadUsage(row, "DynamicAppUsage"),
                Number(row, "AltTabSwitches"), Number(row, "AltTabKeystrokes"), ReadFastest(row)))
            .OrderBy(bucket => bucket.Date, StringComparer.Ordinal).ToList();
    }

    private static SortedDictionary<string, UsageStatsDto> ReadUsage(BsonDocument row, string field)
    {
        var result = new SortedDictionary<string, UsageStatsDto>(StringComparer.Ordinal);
        if (!row.TryGetValue(field, out var value) || value.IsNull) return result;
        foreach (var (name, data) in value.AsDocument)
        {
            var stats = data.AsDocument;
            result.Add(name, new UsageStatsDto(Number(stats, "Switches"), Number(stats, "Peeks"),
                Number(stats, "TotalPeekTimeMs"), Number(stats, "TotalSwitchTimeMs")));
        }
        return result;
    }

    private static FastestDto? ReadFastest(BsonDocument row)
    {
        if (!row.TryGetValue("FastestSwitch", out var value) || value.IsNull) return null;
        var record = value.AsDocument;
        var rawLetter = Text(record, "Letter");
        var keyName = rawLetter.Length == 1 && char.IsAsciiDigit(rawLetter[0]) ? $"D{rawLetter}" : rawLetter;
        if (!Enum.TryParse<Key>(keyName, false, out var key) || !Enum.IsDefined(key))
            throw new InvalidDataException("A fastest-switch key cannot be mapped to a Rust key.");
        var date = record.TryGetValue("Date", out var dateValue) && dateValue.IsDateTime
            ? dateValue.AsDateTime
            : DateTime.MinValue;
        return new FastestDto(Number(record, "DurationMs"), Text(record, "AppName"), key.ToString(),
            new DateTimeOffset(DateTime.SpecifyKind(date, DateTimeKind.Local)).ToUnixTimeMilliseconds());
    }

    private static SortedDictionary<string, string> ReadAppRegistry(string path)
    {
        var entries = new SortedDictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        if (!File.Exists(path)) return entries;
        using var db = OpenReadOnly(path);
        foreach (var row in db.GetCollection<BsonDocument>("app_registry").FindAll())
            entries[Text(row, "ProcessName").ToLowerInvariant()] = Text(row, "DisplayName");
        return entries;
    }

    private static SortedDictionary<string, int> ReadSpotlightUsage(string path)
    {
        var entries = new SortedDictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        if (!File.Exists(path)) return entries;
        using var db = OpenReadOnly(path);
        foreach (var row in db.GetCollection<BsonDocument>("spotlight_usage").FindAll())
            entries[Text(row, "_id").ToUpperInvariant()] = Number(row, "count");
        return entries;
    }

    private static LiteDatabase OpenReadOnly(string path) => new(new ConnectionString(path)
    {
        Connection = ConnectionType.Direct,
        ReadOnly = true
    });

    private static (string Name, int Value) ReadKey(BsonDocument row, string field, Key fallback)
    {
        var value = Number(row, field, (int)fallback);
        var key = (Key)value;
        if (!Enum.IsDefined(key)) throw new InvalidDataException($"Unsupported WPF key value in {field}.");
        return (key.ToString(), value);
    }

    private static string EnumName<T>(int value) where T : struct, Enum
    {
        var parsed = (T)Enum.ToObject(typeof(T), value);
        return Enum.IsDefined(parsed) ? parsed.ToString() : throw new InvalidDataException("Unknown stored enum value.");
    }

    private static string Date(BsonDocument row, string field)
    {
        var value = row[field];
        return value.IsString ? DateOnly.ParseExact(value.AsString, "yyyy-MM-dd").ToString("yyyy-MM-dd") :
            DateOnly.FromDateTime(DateTime.SpecifyKind(value.AsDateTime, DateTimeKind.Local)).ToString("yyyy-MM-dd");
    }
    private static int Number(BsonDocument row, string field, int fallback = 0) =>
        row.TryGetValue(field, out var value) && !value.IsNull ? value.AsInt32 : fallback;
    private static bool Boolean(BsonDocument row, string field, bool fallback = false) =>
        row.TryGetValue(field, out var value) && !value.IsNull ? value.AsBoolean : fallback;
    private static string Text(BsonDocument row, string field) =>
        row.TryGetValue(field, out var value) && !value.IsNull ? value.AsString : "";
    private static string? NullableText(BsonDocument row, string field) =>
        row.TryGetValue(field, out var value) && !value.IsNull ? value.AsString : null;
    private static IEnumerable<BsonValue> Array(BsonDocument row, string field) =>
        row.TryGetValue(field, out var value) && !value.IsNull ? value.AsArray : [];

    private static string SourceIdentity(string settingsPath, string statsPath)
    {
        using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        foreach (var path in new[] { settingsPath, statsPath })
        {
            hash.AppendData(Encoding.UTF8.GetBytes(Path.GetFileName(path)));
            if (File.Exists(path))
            {
                using var stream = File.OpenRead(path);
                hash.AppendData(SHA256.HashData(stream));
            }
        }
        return Convert.ToHexString(hash.GetHashAndReset());
    }

    private static int MigrationVersion(string path)
    {
        if (!File.Exists(path)) return 0;
        using var db = OpenReadOnly(path);
        return db.GetCollection<BsonDocument>("_migrations").FindAll()
            .Select(row => Number(row, "Version")).DefaultIfEmpty().Max();
    }

    private static Bundle Seal(Bundle bundle) => bundle with
    {
        IntegritySha256 = Digest(bundle)
    };
    private static string Digest(Bundle bundle) => Convert.ToHexString(SHA256.HashData(
        CanonicalJson(JsonSerializer.SerializeToElement(Payload.From(bundle), DigestOptions))));

    private static byte[] CanonicalJson(JsonElement value)
    {
        var buffer = new ArrayBufferWriter<byte>();
        using (var writer = new Utf8JsonWriter(buffer, new JsonWriterOptions
        {
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping
        })) WriteCanonical(writer, value);
        return buffer.WrittenSpan.ToArray();
    }

    private static void WriteCanonical(Utf8JsonWriter writer, JsonElement value)
    {
        if (value.ValueKind == JsonValueKind.Object)
        {
            writer.WriteStartObject();
            foreach (var property in value.EnumerateObject().OrderBy(property => property.Name, StringComparer.Ordinal))
            {
                writer.WritePropertyName(property.Name);
                WriteCanonical(writer, property.Value);
            }
            writer.WriteEndObject();
        }
        else if (value.ValueKind == JsonValueKind.Array)
        {
            writer.WriteStartArray();
            foreach (var item in value.EnumerateArray()) WriteCanonical(writer, item);
            writer.WriteEndArray();
        }
        else value.WriteTo(writer);
    }

    private static void SelfTest(string? outputPath)
    {
        var root = Path.Combine(Path.GetTempPath(), $"omnispot-export-selftest-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        try
        {
            using (var settings = new LiteDatabase(new ConnectionString(Path.Combine(root, "settings.db"))))
            {
                settings.GetCollection("settings").Insert(new BsonDocument
                {
                    ["_id"] = 1, ["Modifier"] = (int)Key.RightCtrl,
                    ["Applications"] = new BsonArray { new BsonDocument
                    {
                        ["Key"] = (int)Key.N, ["ProcessPath"] = @"C:\Tools\Editor.exe",
                        ["CycleMode"] = 2, ["StartIfNotRunning"] = true, ["Type"] = 0
                    } }, ["Theme"] = 1, ["OverlayShowDelayMs"] = 500
                });
                settings.GetCollection("spotlight_usage").Insert(new BsonDocument { ["_id"] = "EDITOR.EXE", ["count"] = 4 });
            }
            using (var stats = new LiteDatabase(new ConnectionString(Path.Combine(root, "stats.db"))))
            {
                stats.GetCollection("daily_buckets").Insert(new BsonDocument
                {
                    ["_id"] = "2026-11-05", ["Date"] = "2026-11-05", ["TotalSwitches"] = 9,
                    ["TotalPeeks"] = 2,
                    ["StaticAppUsage"] = new BsonDocument { ["editor.exe"] = new BsonDocument
                    {
                        ["Switches"] = 9, ["Peeks"] = 2, ["TotalPeekTimeMs"] = 500,
                        ["TotalSwitchTimeMs"] = 800
                    } },
                    ["DynamicAppUsage"] = new BsonDocument()
                });
                stats.GetCollection("app_registry").Insert(new BsonDocument
                { ["_id"] = "editor.exe", ["ProcessName"] = "editor.exe", ["DisplayName"] = "Editor" });
            }
            var before = Directory.GetFiles(root).ToDictionary(path => path, path => SHA256.HashData(File.ReadAllBytes(path)));
            var bundle = Seal(Export(root));
            var after = Directory.GetFiles(root).ToDictionary(path => path, path => SHA256.HashData(File.ReadAllBytes(path)));
            if (!before.All(entry => after.TryGetValue(entry.Key, out var bytes) && entry.Value.SequenceEqual(bytes)) ||
                bundle.Settings.Applications.Count != 1 || bundle.Settings.Applications[0].Key != "N" ||
                bundle.Settings.Applications[0].CycleMode != "NextWindow" || bundle.Stats.Buckets.Single().TotalSwitches != 9 ||
                bundle.Stats.Buckets.Single().TotalPeeks != 2 ||
                bundle.Stats.Buckets.Single().StaticAppUsage["editor.exe"].TotalPeekTimeMs != 500 ||
                bundle.SpotlightUsage.Counts["EDITOR.EXE"] != 4 || bundle.AppRegistry["editor.exe"] != "Editor" ||
                bundle.IntegritySha256 != Digest(bundle) || bundle.IntegritySha256.Length != 64)
                throw new InvalidOperationException("Synthetic export did not preserve the expected data or read-only source files.");
            if (outputPath is not null)
            {
                var json = JsonSerializer.Serialize(Seal(bundle), JsonOptions);
                Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(outputPath))!);
                File.WriteAllText(outputPath, json);
            }
        }
        finally { Directory.Delete(root, recursive: true); }
    }

    private enum CycleMode { NextApp, Hide, NextWindow }
    private enum ApplicationType { Win32, Packaged }
    private enum AppThemeSetting { System, Dark, Light }
}

internal sealed record Bundle(int FormatVersion, string SourceAppVersion, int SettingsMigrationVersion,
    int StatsMigrationVersion, string SourceProfileId,
    DateTimeOffset ExportedAt, SettingsDto Settings, StatsDto Stats,
    SortedDictionary<string, string> AppRegistry, UsageDto SpotlightUsage, string IntegritySha256);
internal sealed record Payload(int FormatVersion, string SourceAppVersion, int SettingsMigrationVersion,
    int StatsMigrationVersion, string SourceProfileId, DateTimeOffset ExportedAt, SettingsDto Settings,
    StatsDto Stats, SortedDictionary<string, string> AppRegistry, UsageDto SpotlightUsage)
{
    public static Payload From(Bundle bundle) => new(bundle.FormatVersion, bundle.SourceAppVersion,
        bundle.SettingsMigrationVersion, bundle.StatsMigrationVersion, bundle.SourceProfileId,
        bundle.ExportedAt, bundle.Settings, bundle.Stats, bundle.AppRegistry, bundle.SpotlightUsage);
}
internal sealed record SettingsDto(int SchemaVersion, string Modifier, int ModifierLegacyValue,
    List<ApplicationDto> Applications, bool PulseBorderEnabled, string Theme, bool OverlayEnabled,
    int OverlayShowDelayMs, bool OverlayKeepOpenWhileModifierHeld, bool PeekEnabled,
    bool DynamicModeEnabled, bool StatsEnabled, bool SpotlightEnabled, string SpotlightModifier,
    int SpotlightModifierLegacyValue)
{
    public static SettingsDto Default => new(2, "RightCtrl", (int)Key.RightCtrl, [], true, "System",
        false, 1000, true, false, true, false, true, "LeftAlt", (int)Key.LeftAlt);
}
internal sealed record ApplicationDto(string Key, int KeyLegacyValue, string ProcessPath,
    string CycleMode, bool StartIfNotRunning, string ApplicationType, string? Aumid);
internal sealed record StatsDto(List<DailyBucketDto> Buckets);
internal sealed record DailyBucketDto(string Date, int TotalSwitches, int TotalTimeSavedMs, int TotalPeeks,
    SortedDictionary<string, UsageStatsDto> StaticAppUsage, SortedDictionary<string, UsageStatsDto> DynamicAppUsage,
    int AltTabSwitches, int AltTabKeystrokes, FastestDto? FastestSwitch);
internal sealed record UsageStatsDto(int Switches, int Peeks, int TotalPeekTimeMs, int TotalSwitchTimeMs);
internal sealed record FastestDto(int DurationMs, string AppName, string Letter, long TimestampMs);
internal sealed record UsageDto(SortedDictionary<string, int> Counts);
