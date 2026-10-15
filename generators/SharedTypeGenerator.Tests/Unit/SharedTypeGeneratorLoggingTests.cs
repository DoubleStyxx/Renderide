using System.Globalization;
using NotEnoughLogs;
using SharedTypeGenerator.Logging;
using SharedTypeGenerator.Tests.Unit.Support;
using Xunit;

namespace SharedTypeGenerator.Tests.Unit;

/// <summary>Unit tests for <see cref="SharedTypeGeneratorLogging.CreateMainSink"/>.</summary>
public sealed class SharedTypeGeneratorLoggingTests
{
    /// <summary>Info-level lines are written to the log file but never duplicated to stderr.</summary>
    [Fact]
    public void CreateMainSink_routes_info_to_log_file_only()
    {
        using var temp = new TempFile();
        var stderr = new StringWriter(CultureInfo.InvariantCulture);

        using (var sink = SharedTypeGeneratorLogging.CreateMainSink(temp.FilePath, stderr))
        {
            sink.Log(LogLevel.Info, "Cat", "hello info");
        }

        string fileText = File.ReadAllText(temp.FilePath);
        Assert.Contains("hello info", fileText, StringComparison.Ordinal);
        Assert.Contains(" Info ", fileText, StringComparison.Ordinal);
        Assert.Equal(string.Empty, stderr.ToString());
    }

    /// <summary>Warnings are dropped before they reach the file or the error mirror.</summary>
    [Fact]
    public void CreateMainSink_drops_warnings_silently()
    {
        using var temp = new TempFile();
        var stderr = new StringWriter(CultureInfo.InvariantCulture);

        using (var sink = SharedTypeGeneratorLogging.CreateMainSink(temp.FilePath, stderr))
        {
            sink.Log(LogLevel.Warning, "Cat", "should not appear");
        }

        string fileText = File.Exists(temp.FilePath) ? File.ReadAllText(temp.FilePath) : string.Empty;
        Assert.DoesNotContain("should not appear", fileText, StringComparison.Ordinal);
        Assert.Equal(string.Empty, stderr.ToString());
    }

    /// <summary>Error-level lines are written to both the log file and stderr with the same formatted text.</summary>
    [Fact]
    public void CreateMainSink_mirrors_errors_to_stderr()
    {
        using var temp = new TempFile();
        var stderr = new StringWriter(CultureInfo.InvariantCulture);

        using (var sink = SharedTypeGeneratorLogging.CreateMainSink(temp.FilePath, stderr))
        {
            sink.Log(LogLevel.Error, "Cat", "critical failure");
        }

        string fileText = File.ReadAllText(temp.FilePath).TrimEnd('\r', '\n');
        string err = stderr.ToString().TrimEnd('\r', '\n');
        Assert.Equal(fileText, err);
        Assert.Contains("critical failure", err, StringComparison.Ordinal);
        Assert.Contains(" Error ", err, StringComparison.Ordinal);
    }

    /// <summary>Blank or whitespace log file paths are rejected by the factory's argument guard.</summary>
    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    public void CreateMainSink_throws_on_blank_path(string path)
    {
        Assert.Throws<ArgumentException>(() => SharedTypeGeneratorLogging.CreateMainSink(path));
    }
}
