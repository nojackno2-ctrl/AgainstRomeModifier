using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class PatchOperationRunnerTests
{
    [Fact]
    public async Task ExecuteAsync_commits_successful_operation()
    {
        string root = CreateTempDirectory();
        string path = Path.Combine(root, "target.bin");
        File.WriteAllText(path, "original");
        var logs = new List<string>();
        var runner = new PatchOperationRunner(logs.Add);
        try
        {
            await runner.ExecuteAsync(
                rollback =>
                {
                    rollback.TrackFile(path);
                    File.WriteAllText(path, "changed");
                },
                "checkpoint",
                "rollback-start",
                "rollback-done");

            Assert.Equal("changed", File.ReadAllText(path));
            Assert.Equal(["checkpoint"], logs);
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    [Fact]
    public async Task ExecuteAsync_restores_failed_operation_and_rethrows()
    {
        string root = CreateTempDirectory();
        string path = Path.Combine(root, "target.bin");
        File.WriteAllText(path, "original");
        var logs = new List<string>();
        var runner = new PatchOperationRunner(logs.Add);
        try
        {
            await Assert.ThrowsAsync<InvalidOperationException>(() => runner.ExecuteAsync(
                rollback =>
                {
                    rollback.TrackFile(path);
                    File.WriteAllText(path, "changed");
                    throw new InvalidOperationException("failure");
                },
                "checkpoint",
                "rollback-start",
                "rollback-done"));

            Assert.Equal("original", File.ReadAllText(path));
            Assert.Equal(["checkpoint", "rollback-start", "rollback-done"], logs);
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    private static string CreateTempDirectory()
    {
        string path = Path.Combine(Path.GetTempPath(), "ARM_OperationRunner_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(path);
        return path;
    }
}
