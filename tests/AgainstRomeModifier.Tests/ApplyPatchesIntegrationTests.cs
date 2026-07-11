using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using Xunit;
using Xunit.Abstractions;
using AgainstRomeModifier;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests
{
    /// <summary>
    /// 端到端整合測試：以本機「遊戲原始檔案」複製到臨時目錄後，
    /// 執行完整的 ApplyPatches（全功能開啟）→ DetectCurrentPatchState 回讀 →
    /// RestoreOriginalFiles → 再次回讀 + 與來源檔逐位元組（解壓後）比對。
    /// 覆蓋 dry-run/共用 orchestrator/內容相同跳過寫入等真實套用流程。
    /// 需要本機的遊戲原始檔案與 Backup.zip；CI 環境自動略過。
    /// 絕不觸碰真實遊戲安裝目錄。
    /// </summary>
    public class ApplyPatchesIntegrationTests
    {
        private readonly ITestOutputHelper _output;

        public ApplyPatchesIntegrationTests(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void ApplyPatches_full_roundtrip_on_local_game_tree()
        {
            Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

            string sourceRoot = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../遊戲原始檔案"));
            if (!Directory.Exists(sourceRoot))
            {
                _output.WriteLine("[SKIPPED] 遊戲原始檔案目錄不存在（CI 環境），略過整合測試。");
                return;
            }

            var backupManager = new BackupManager(new NullLogger());
            backupManager.LoadBackupZipToMemory("");
            if (!backupManager.BackupFiles.ContainsKey("SYSTEM/cl_script.ini"))
            {
                _output.WriteLine("[SKIPPED] Backup.zip 不可用（CI 環境），略過整合測試。");
                return;
            }

            string gameDir = Path.Combine(Path.GetTempPath(), "ARM_ApplyIntegration_" + Guid.NewGuid().ToString("N"));
            var copiedFiles = new List<string>();
            try
            {
                // 只複製套用流程會觸碰的檔案（避免整棵樹搬運）
                CopyFile(sourceRoot, gameDir, "Against_Rome.exe", copiedFiles);
                CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "cl_script.ini"), copiedFiles);
                CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "cl_epara.ini"), copiedFiles);
                CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "ress.ini"), copiedFiles);
                CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "CLAK", "cl_scint.ini"), copiedFiles);
                CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau"), copiedFiles);
                CopyTree(sourceRoot, gameDir, Path.Combine("SYSTEM", "CLAK", "SCRIPT"), copiedFiles);
                foreach (string endlDir in Directory.GetDirectories(Path.Combine(sourceRoot, "MAPS"), "ENDL_*"))
                {
                    CopyTree(sourceRoot, gameDir, Path.GetRelativePath(sourceRoot, endlDir), copiedFiles);
                }
                _output.WriteLine($"已複製 {copiedFiles.Count} 個檔案到 {gameDir}");

                var engine = new PatchEngine(new NullLogger());

                // 1) 全功能開啟套用
                var options = new PatchOptions
                {
                    FocusLoss = true,
                    FastCiviProduction = true,
                    InfiniteMorale = true,
                    FreeProduction = true,
                    FreeUpgrade = true,
                    NoSpellCost = true,
                    MaxPopulation = true,
                    Balance = true,
                    HousingCapacity20x = true,
                    StorageCapacity10x = true,
                    HqHp10x = true,
                    FastBuildUpgradeRepair = true,
                    FoodHealing10x = true,
                    VillageBuildRange = true,
                    NoSpellAltar = true,
                    GameSpeed = 3,
                    ToEnglish = false,
                    DgVoodoo = false,
                    EndlessAiModules = new Dictionary<string, bool>(StringComparer.OrdinalIgnoreCase) {
                        ["M1"] = true, ["M2"] = true, ["M3"] = true,
                        ["M4"] = true, ["M5"] = true, ["M6"] = true
                    }
                };
                using (var rollback = new FileRollbackScope())
                {
                    engine.ApplyPatches(gameDir, options, backupManager, rollback);
                    rollback.Commit();
                }

                // 2) 偵測回讀應與套用選項一致
                var detected = engine.DetectCurrentPatchState(gameDir, backupManager);
                Assert.True(detected.FocusLoss, "FocusLoss 未回讀為 true");
                Assert.True(detected.FastCiviProduction, "FastCiviProduction 未回讀為 true");
                Assert.True(detected.InfiniteMorale, "InfiniteMorale 未回讀為 true");
                Assert.True(detected.FreeProduction, "FreeProduction 未回讀為 true");
                Assert.True(detected.FreeUpgrade, "FreeUpgrade 未回讀為 true");
                Assert.True(detected.NoSpellCost, "NoSpellCost 未回讀為 true");
                Assert.True(detected.MaxPopulation, "MaxPopulation 未回讀為 true");
                Assert.True(detected.Balance, "Balance 未回讀為 true");
                Assert.True(detected.HousingCapacity20x, "HousingCapacity20x 未回讀為 true");
                Assert.True(detected.StorageCapacity10x, "StorageCapacity10x 未回讀為 true");
                Assert.True(detected.HqHp10x, "HqHp10x 未回讀為 true");
                Assert.True(detected.FastBuildUpgradeRepair, "FastBuildUpgradeRepair 未回讀為 true");
                Assert.True(detected.FoodHealing10x, "FoodHealing10x 未回讀為 true");
                Assert.True(detected.VillageBuildRange, "VillageBuildRange 未回讀為 true");
                Assert.True(detected.NoSpellAltar, "NoSpellAltar 未回讀為 true");
                Assert.Equal(3, detected.GameSpeed);
                foreach (string id in new[] { "M1", "M2", "M3", "M4", "M5", "M6" })
                {
                    Assert.True(detected.GetEndlessAiModule(id), $"Endless AI 模組 {id} 未回讀為 true");
                }

                // 3) 全部還原
                using (var rollback = new FileRollbackScope())
                {
                    engine.RestoreOriginalFiles(gameDir, backupManager, rollback);
                    rollback.Commit();
                }

                var restored = engine.DetectCurrentPatchState(gameDir, backupManager);
                Assert.False(restored.FocusLoss, "還原後 FocusLoss 仍為 true");
                Assert.False(restored.FastCiviProduction, "還原後 FastCiviProduction 仍為 true");
                Assert.False(restored.InfiniteMorale, "還原後 InfiniteMorale 仍為 true");
                Assert.False(restored.MaxPopulation, "還原後 MaxPopulation 仍為 true");
                Assert.False(restored.Balance, "還原後 Balance 仍為 true");
                Assert.False(restored.FoodHealing10x, "還原後 FoodHealing10x 仍為 true");
                Assert.False(restored.VillageBuildRange, "還原後 VillageBuildRange 仍為 true");
                Assert.False(restored.NoSpellAltar, "還原後 NoSpellAltar 仍為 true");
                Assert.Equal(1, restored.GameSpeed);
                foreach (string id in new[] { "M1", "M2", "M3", "M4", "M5", "M6" })
                {
                    Assert.False(restored.GetEndlessAiModule(id), $"還原後 Endless AI 模組 {id} 仍為 true");
                }

                // 4) 位元組層驗證：還原後的每個複製檔（解壓後）必須與來源 100% 相同
                foreach (string relPath in copiedFiles)
                {
                    byte[] expected = File.ReadAllBytes(Path.Combine(sourceRoot, relPath));
                    byte[] actual = File.ReadAllBytes(Path.Combine(gameDir, relPath));
                    if (!relPath.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
                    {
                        expected = GameLZSS.DecompressPfil(expected);
                        actual = GameLZSS.DecompressPfil(actual);
                    }
                    Assert.True(expected.SequenceEqual(actual), $"還原後內容與來源不符: {relPath}");
                }
                _output.WriteLine($"整合測試通過：{copiedFiles.Count} 個檔案還原後與來源 100% 相同。");
            }
            finally
            {
                try
                {
                    if (Directory.Exists(gameDir)) Directory.Delete(gameDir, true);
                }
                catch (Exception cleanupEx)
                {
                    _output.WriteLine("清理臨時目錄失敗: " + cleanupEx.Message);
                }
            }
        }

        private static void CopyFile(string sourceRoot, string destRoot, string relPath, List<string> copied)
        {
            string src = Path.Combine(sourceRoot, relPath);
            Assert.True(File.Exists(src), $"整合測試來源缺少必要檔案: {relPath}");
            string dest = Path.Combine(destRoot, relPath);
            Directory.CreateDirectory(Path.GetDirectoryName(dest)!);
            File.Copy(src, dest, true);
            copied.Add(relPath);
        }

        private static void CopyTree(string sourceRoot, string destRoot, string relDir, List<string> copied)
        {
            string srcDir = Path.Combine(sourceRoot, relDir);
            Assert.True(Directory.Exists(srcDir), $"整合測試來源缺少必要目錄: {relDir}");
            foreach (string file in Directory.GetFiles(srcDir, "*", SearchOption.AllDirectories))
            {
                string rel = Path.GetRelativePath(sourceRoot, file);
                string dest = Path.Combine(destRoot, rel);
                Directory.CreateDirectory(Path.GetDirectoryName(dest)!);
                File.Copy(file, dest, true);
                copied.Add(rel);
            }
        }
    }
}
