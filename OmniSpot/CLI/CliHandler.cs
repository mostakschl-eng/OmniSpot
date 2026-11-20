using Microsoft.Extensions.Logging;
using OmniSpot.Extensions;
using System.Text;
using Wpf.Ui.Controls;
using FontFamily = System.Windows.Media.FontFamily;
using MessageBox = Wpf.Ui.Controls.MessageBox;

namespace OmniSpot.CLI;

internal class CliHandler(
    ILogger<CliHandler> logger,
    IServiceProvider serviceProvider,
    CliBuilder builder,
    CliOptions options)
{
    public bool Handle(string[] args)
    {
        var parsed = RustCliParser.Parse(args, builder);
        foreach (var unknown in parsed.UnknownArguments)
            logger.LogWarning("Unknown argument: {Arg}", unknown);

        foreach (var flag in parsed.MatchedFlags)
        {
            var option = builder.FlagOptions.FirstOrDefault(o => o.Name == flag);
            if (option is not null) option.Handler(options);
        }
        foreach (var pair in parsed.ValuedValues)
        {
            if (pair.Length == 2 && builder.ValuedOptions.FirstOrDefault(o => o.Name == pair[0]) is { } option)
                option.Handler(options, pair[1]);
        }

        if (parsed.Status == "help")
        {
            ShowHelp();
            return true;
        }
        if (parsed.Status == "error")
        {
            new MessageBox
            {
                Title = "OmniSpot",
                Content = parsed.Error,
                CloseButtonIcon = new SymbolIcon(SymbolRegular.ErrorCircle24),
                CloseButtonText = "OK",
                CloseButtonAppearance = ControlAppearance.Danger,
            }.ShowSync();
            return true;
        }

        if (parsed.Status == "command" && parsed.Command is { } commandName)
        {
            var command = builder.Commands.First(c => c.Name == commandName);
            logger.LogDebug("Executing command: {Name}", command.Name);
            command.Handler(serviceProvider);
            return true;
        }

        // No command executed; Rust applied the options and ignored unknown arguments.
        return false;
    }

    private void ShowHelp()
    {
        var message = new StringBuilder();
        message.AppendLine("OmniSpot.exe [command] [options]");
        message.AppendLine();

        if (builder.Commands.Count > 0)
        {
            message.AppendLine("Commands:");
            foreach (var cmd in builder.Commands)
            {
                message.AppendLine($"  {cmd.Name,-20} {cmd.Description}");
            }
            message.AppendLine($"  {"--help",-20} Show this help message");
            message.AppendLine();
        }

        if (builder.FlagOptions.Count > 0 || builder.ValuedOptions.Count > 0)
        {
            message.AppendLine("Options:");
            foreach (var opt in builder.FlagOptions)
            {
                message.AppendLine($"  {opt.Name,-20} {opt.Description}");
            }
            foreach (var opt in builder.ValuedOptions)
            {
                message.AppendLine($"  {opt.Name,-20} {opt.Description}");
            }
        }

        new MessageBox
        {
            FontFamily = new FontFamily("Consolas, Courier New, Monospace"),
            MinWidth = 300,
            MaxWidth = 600,
            Title = "OmniSpot",
            Content = message.ToString(),
            CloseButtonIcon = new SymbolIcon(SymbolRegular.Info24),
            CloseButtonText = "OK",
        }.ShowSync();
    }
}
