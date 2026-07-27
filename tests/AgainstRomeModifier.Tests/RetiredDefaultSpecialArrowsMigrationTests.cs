using AgainstRomeModifier.Core.Features.Bci;

namespace AgainstRomeModifier.Tests;

public sealed class RetiredDefaultSpecialArrowsMigrationTests
{
    [Fact]
    public void Retired_bci_output_is_restored_byte_exactly()
    {
        string root = CreateRoot();
        try
        {
            string path = ScriptPath(root);
            BackupZipGameFixture.WritePfil(path, BackupZipGameFixture.Words(
                66, 1000001, 87, 130,
                66, 1, 66, 0,
                81, 10, 81, 98,
                128, 106, 73, -4, 86));
            byte[] pristine = File.ReadAllBytes(path);
            BackupZipGameFixture.WritePfil(path, BackupZipGameFixture.Words(
                66, 1000001, 87, 130,
                66, 1, 66, 1,
                81, 10, 81, 98,
                128, 106, 73, -4, 86));

            Assert.True(RetiredDefaultSpecialArrowsMigration.TryDetectLegacy(root, out bool patched));
            Assert.True(patched);

            var orchestrator = new EndlessAiOrchestrator();
            RetiredDefaultSpecialArrowsMigration.RestoreIfPresent(root, orchestrator, new NullLogger());
            orchestrator.SaveAll(root, rollback: null);

            Assert.True(RetiredDefaultSpecialArrowsMigration.TryDetectLegacy(root, out patched));
            Assert.False(patched);
            Assert.Equal(pristine, File.ReadAllBytes(path));
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    [Fact]
    public void Unknown_bci_is_preserved()
    {
        string root = CreateRoot();
        try
        {
            string path = ScriptPath(root);
            BackupZipGameFixture.WritePfil(path, BackupZipGameFixture.Words(66, 99, 86));
            byte[] before = File.ReadAllBytes(path);

            var orchestrator = new EndlessAiOrchestrator();
            RetiredDefaultSpecialArrowsMigration.RestoreIfPresent(root, orchestrator, new NullLogger());
            orchestrator.SaveAll(root, rollback: null);

            Assert.Equal(before, File.ReadAllBytes(path));
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    private static string CreateRoot()
    {
        string root = Path.Combine(Path.GetTempPath(), "arm-retired-arrows-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        return root;
    }

    private static string ScriptPath(string root) =>
        Path.Combine(root, "SYSTEM", "CLAK", "SCRIPT", "ak_krieger.bci");
}
