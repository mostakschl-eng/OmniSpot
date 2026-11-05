using AwesomeAssertions;
using LiteDB;
using Microsoft.Extensions.Logging.Abstractions;
using NSubstitute;
using OmniSpot.Configuration;
using OmniSpot.Configuration.Migrations;
using OmniSpot.Spotlight;
using OmniSpot.Stats;
using OmniSpot.Stats.Storage;
using OmniSpot.WindowDiscovery;
using System.Windows.Input;
using Xunit;

namespace OmniSpot.Tests.Spotlight;

public class AppSearchProviderTests
{
    private readonly ConfigurationManager _configManager;
    private readonly RunningApplicationsService _runningAppsService;
    private readonly IPackagedAppsService _packagedAppsService = Substitute.For<IPackagedAppsService>();
    private readonly IAppRegistryCache _appRegistryCache = Substitute.For<IAppRegistryCache>();
    private readonly IconExtractor _iconExtractor;
    private readonly AppSearchProvider _sut;

    public AppSearchProviderTests()
    {
        var db = new LiteDatabase(":memory:");
        var configService = new ConfigurationService(db, NullLogger<ConfigurationService>.Instance);
        var config = new OmniSpot.Configuration.Configuration(
            Key.RightCtrl,
            [
                new ApplicationConfiguration(Key.C, "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe", CycleMode.NextApp, false),
                new ApplicationConfiguration(Key.N, "C:\\Windows\\notepad.exe", CycleMode.NextApp, false)
            ],
            true, AppThemeSetting.System, false, 1000, true, false, true, false);

        configService.WriteConfiguration(config);

        var configValidator = new ConfigurationValidator(NullLogger<ConfigurationValidator>.Instance);
        var migrationRunner = new MigrationRunner(db, [], NullLogger<MigrationRunner>.Instance);
        var sanitizer = new PackagedAppPathSanitizer(_packagedAppsService, NullLogger<PackagedAppPathSanitizer>.Instance);

        _configManager = new ConfigurationManager(
            configService,
            configValidator,
            migrationRunner,
            sanitizer,
            NullLogger<ConfigurationManager>.Instance);

        var windowEnumerator = Substitute.For<IWindowEnumerator>();
        windowEnumerator.GetWindows().Returns([]);

        _iconExtractor = new IconExtractor();
        _runningAppsService = new RunningApplicationsService(
            windowEnumerator,
            _iconExtractor,
            _packagedAppsService,
            Substitute.For<AppRegistryCache>(db, windowEnumerator, _packagedAppsService, Substitute.For<IProcessInspector>(), NullLogger<AppRegistryCache>.Instance));

        _packagedAppsService.GetInstalledPackages().Returns(new InstalledPackages([]));

        _appRegistryCache.GetDisplayName("chrome.exe", Arg.Any<string>()).Returns("Google Chrome");
        _appRegistryCache.GetDisplayName("notepad.exe", Arg.Any<string>()).Returns("Notepad");

        _sut = new AppSearchProvider(
            _configManager,
            _runningAppsService,
            _packagedAppsService,
            _appRegistryCache,
            _iconExtractor,
            new SessionStats(),
            new StatsRepository(
                new StatsDbProvider("unused", () => new LiteDatabase(":memory:")),
                NullLogger<StatsRepository>.Instance),
            NullLogger<AppSearchProvider>.Instance);
    }

    [Fact]
    public async Task GetSuggestedAppsAsync_ReturnsConfiguredApplications()
    {
        // Act
        var suggestions = await _sut.GetSuggestedAppsAsync(TestContext.Current.CancellationToken);

        // Assert
        suggestions.Should().NotBeEmpty();
        suggestions.Should().Contain(s => s.Title == "Google Chrome");
        suggestions.Should().Contain(s => s.Title == "Notepad");
    }

    [Fact]
    public async Task SearchAppsAsync_MatchesQuery_CaseInsensitively()
    {
        // Act
        var results = await _sut.SearchAppsAsync("chr", TestContext.Current.CancellationToken);

        // Assert
        results.Should().Contain(r => r.Title == "Google Chrome" && r.Type == SearchResultType.App);
    }

    [Fact]
    public async Task SearchAppsAsync_ReturnsEmpty_WhenNoAppMatches()
    {
        // Act
        var results = await _sut.SearchAppsAsync("xyz123nonexistent", TestContext.Current.CancellationToken);

        // Assert
        results.Should().BeEmpty();
    }

    [Fact]
    public async Task SearchAppsAsync_MatchesRelevantCharacterSequence()
    {
        // Act
        var results = await _sut.SearchAppsAsync("gch", TestContext.Current.CancellationToken);

        // Assert
        results.Should().Contain(r => r.Title == "Google Chrome");
    }

    [Fact]
    public async Task SearchAppsAsync_ReusesCatalog_AcrossQueries()
    {
        // Act
        await _sut.SearchAppsAsync("chr", TestContext.Current.CancellationToken);
        await _sut.SearchAppsAsync("note", TestContext.Current.CancellationToken);

        // Assert
        _ = _packagedAppsService.Received(1).GetSearchableApps();
    }
}
