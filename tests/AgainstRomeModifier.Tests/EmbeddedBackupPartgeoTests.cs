using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 迴歸測試：內建 Backup.zip 曾漏掉原始 partgeo.dau，導致勾選「拋射彈道增高」
/// 套用時 PartgeoPatchFileContributor 因缺備份而拋例外、整次套用失敗。
/// 這裡確保嵌入備份含原始 partgeo，且 ProjectileArcHeight 能順利組合。
/// </summary>
public sealed class EmbeddedBackupPartgeoTests
{
    private const string PartgeoKey = "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau";

    [Fact]
    public void Embedded_backup_contains_original_partgeo()
    {
        var loader = new BackupSourceLoader(typeof(BackupManager).Assembly, AppContext.BaseDirectory);
        BackupZipLoadResult result = loader.TryLoadPreferredZip();

        // Backup.zip is gitignored and only embedded when present at build time
        // (csproj IncludeBackupZip). Where it isn't embedded (e.g. a clean CI
        // checkout) there is nothing to regress against, so skip rather than fail.
        if (result.Source != BackupZipSource.Embedded) return;

        Assert.True(result.Files.ContainsKey(PartgeoKey),
            "嵌入的 Backup.zip 缺少 " + PartgeoKey);
        Assert.True(OriginalFileValidator.IsPartgeoOriginal(result.Files[PartgeoKey]),
            "Backup.zip 內的 partgeo.dau 不是原始檔（Pfeil00 ysub 應為 5832704）");
    }

    [Fact]
    public void ProjectileArcHeight_composes_from_embedded_backup_without_throwing()
    {
        var loader = new BackupSourceLoader(typeof(BackupManager).Assembly, AppContext.BaseDirectory);
        BackupZipLoadResult loaded = loader.TryLoadPreferredZip();
        if (loaded.Source != BackupZipSource.Embedded || !loaded.Files.ContainsKey(PartgeoKey)) return;

        var files = new Dictionary<string, byte[]>(loaded.Files, StringComparer.OrdinalIgnoreCase);
        byte[] original = files[PartgeoKey];
        var patched = PartgeoPatcher.GetPatchedBytes(original, new PartgeoOptions(true));

        // Patch 必須真的產生變化，且產物解壓後仍是合法 CSV（不再是原始）。
        Assert.NotEqual(original, patched);
        Assert.False(OriginalFileValidator.IsPartgeoOriginal(patched));
    }
}
