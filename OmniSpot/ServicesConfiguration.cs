using OmniSpot.UI;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using NLog.Extensions.Logging;
using OmniSpot.CLI;
using OmniSpot.Configuration;
using OmniSpot.Extensions;
using OmniSpot.Input;
using OmniSpot.Overlay;
using OmniSpot.Spotlight;
using OmniSpot.Startup;
using OmniSpot.Stats;
using OmniSpot.Stats.Storage;
using OmniSpot.UI.Pages;
using OmniSpot.UI.ViewModels;
using OmniSpot.UI.ViewModels.Common;
using OmniSpot.UI.Windows;
using OmniSpot.WindowDiscovery;
using System.IO;
using System.Reflection;
using System.Windows.Controls;
using Wpf.Ui;
using Wpf.Ui.Abstractions;
using Wpf.Ui.Controls;

namespace OmniSpot;

internal static class ServicesConfiguration
{
    public static IServiceProvider? Build(bool isPortableMode)
    {
        var services = new ServiceCollection();

        services.AddLogging(logging => logging.AddNLog());

        if (!services.SetupDatabases(isPortableMode))
        {
            return null;
        }

        services.AddTransient<INavigationViewPageProvider, PageProviderService>();
        services.AddSingleton<ISnackbarService, SnackbarService>();
        services.AddSingleton<IContentDialogService, ContentDialogService>();

        services.AddTransient<ApplicationsValidator>();
        services.AddSingleton<SettingsBridge>();
        services.AddSingleton<Hook>();
        services.AddSingleton<RustHookBackend>();
        services.AddSingleton<DynamicModeService>();
        services.AddSingleton<IAppNameResolver, AppNameResolver>();
        services.AddTransient<AutoStart>();
        services.AddSingleton<IconExtractor>();
        services.AddTransient<OverlayShowTimer>();
        services.AddTransient<RunningApplicationsService>();
        services.AddSingleton<IPackagedAppsService, RustPackagedAppsBackend>();
        services.AddSingleton<IWindowEnumerator, WindowEnumerator>();
        services.AddTransient<WindowTitleParser>();
        services.AddTransient<AppOverlayService>();
        services.AddSingleton<WarningOverlayService>();
        services.AddSingleton<IProcessInspector, RustProcessInspectorBackend>();

        services.AddSingleton<RustStatsBackend>();
        services.AddSingleton<RustAppRegistryBackend>();
        services.AddSingleton<SessionStats>();
        services.AddSingleton<AppRegistryCache>();
        services.AddSingleton<IAppRegistryCache>(sp => sp.GetRequiredService<AppRegistryCache>());
        services.AddSingleton<StatsService>();

        services.AddCliHandler();

        // windows
        services.AddTransient<MainWindow>();
        services.AddTransient<Settings>();
        services.AddSingleton<AppOverlayWindow>();
        services.AddSingleton<WarningOverlayWindow>();

        // pages
        services.AddImplementationsOf<Page>(ServiceLifetime.Transient, registerAsConcreteType: true);

        // view models
        services.AddTransient<MainWindowViewModel>();
        services.AddSingleton<ISettingsState, SettingsState>();
        services.AddTransient<SettingsViewModel>();
        services.AddTransient<HotkeysViewModel>();
        services.AddTransient<GeneralSettingsViewModel>();
        services.AddTransient<OverlaySettingsViewModel>();
        services.AddTransient<StatsSettingsViewModel>();
        services.AddTransient<AddApplicationFlyoutViewModel>();
        services.AddTransient<AboutViewModel>();
        services.AddSingleton<AppOverlayViewModel>();

        // spotlight
        services.AddSingleton<SpotlightIconService>();
        services.AddSingleton<RustSearchBackend>();
        services.AddTransient<ISpotlightSearchService, RustSpotlightSearchService>();
        services.AddSingleton<SpotlightService>();
        services.AddSingleton<SpotlightWindow>();
        services.AddSingleton<SpotlightViewModel>();
        services.AddTransient<SpotlightSearchSettingsViewModel>();

        return services.BuildServiceProvider();
    }

    private static bool SetupDatabases(this ServiceCollection services, bool isPortableMode)
    {
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        var directory = isPortableMode ? AppContext.BaseDirectory : Path.Combine(appData, "OmniSpot");
        var settingsPath = Path.Combine(directory, "settings.sqlite3");

        try
        {
            var settings = new RustDatabase(settingsPath);
            LegacyDatabaseImport.Settings(settings, Path.Combine(directory, "settings.db"),
                new RustPackagedAppsBackend());
            services.AddSingleton(settings);
            services.AddSingleton(new StatsDbProvider(directory));

            return true;
        }
        catch (Exception error)
        {
            new MessageBox
            {
                Title = "Database error",
                Content = $"Could not open or migrate the settings database.\n\n{settingsPath}\n{error.Message}",
                CloseButtonIcon = new SymbolIcon(SymbolRegular.ErrorCircle24),
                CloseButtonText = "Quit",
                CloseButtonAppearance = ControlAppearance.Danger,
            }.ShowSync();
            return false;
        }

    }

    private static void AddImplementationsOf<TInterface>(this IServiceCollection services, ServiceLifetime lifetime,
        bool registerAsConcreteType = false)
    {
        var interfaceType = typeof(TInterface);
        var implementations = Assembly.GetExecutingAssembly().GetTypes()
            .Where(t => interfaceType.IsAssignableFrom(t) && t is { IsClass: true, IsAbstract: false });

        foreach (var implementation in implementations)
        {
            var serviceType = registerAsConcreteType ? implementation : interfaceType;
            var serviceDescriptor = new ServiceDescriptor(serviceType, implementation, lifetime);

            if (registerAsConcreteType)
            {
                services.TryAdd(serviceDescriptor);
            }
            else
            {
                // Allow multiple implementations for the same interface to be registered.
                services.Add(serviceDescriptor);
            }
        }
    }
}
