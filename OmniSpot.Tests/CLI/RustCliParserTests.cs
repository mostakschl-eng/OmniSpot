using AwesomeAssertions;
using OmniSpot.CLI;
using Xunit;

namespace OmniSpot.Tests.CLI;

public class RustCliParserTests
{
    [Fact]
    public void Parse_ReportsUnknownArgumentsWithoutTreatingOptionValuesAsUnknown()
    {
        var builder = new CliBuilder()
            .AddOption("--path", "Test path", (_, _) => { })
            .AddOption("--debug", "Debug", _ => { });
        var result = RustCliParser.Parse(["--typo", "--path", "value", "--debug", "--help", "--ignored"], builder);

        result.Status.Should().Be("help");
        result.UnknownArguments.Should().Equal("--typo");
        result.MatchedFlags.Should().Equal("--debug");
    }
}
