using System;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;
using System.Globalization;
using Xunit;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Tests
{
    public class ClScriptPatchTests
    {
        private readonly Xunit.Abstractions.ITestOutputHelper _output;

        public ClScriptPatchTests(Xunit.Abstractions.ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void Test_ClScript_Patch_And_Detect()
        {
            Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

            // 1. Prepare backup manager with real embedded backup
            var backupManager = new BackupManager(new NullLogger());
            backupManager.LoadBackupZipToMemory("");

            // Backup.zip 內含專有遊戲資料，被 .gitignore 排除，不會推送到 GitHub。
            // 因此在 CI 環境（沒有內嵌資源）時視為無需驗證而略過；本機開發（有 Backup.zip）則完整執行。
            if (!backupManager.BackupFiles.ContainsKey("SYSTEM/cl_script.ini"))
            {
                // Embedded Backup.zip 不可用（gitignore 的專有遊戲資料）；在 CI 中略過。
                // 在輸出中明確標示，避免測試報告呈現「Passed」造成覆蓋率假象。
                _output.WriteLine("[SKIPPED] Backup.zip 不可用（CI 環境），本測試未執行任何斷言。");
                return;
            }

            // 2. Patch — 走與 Apply 相同的生產路徑（IniFeaturePatcher/ObjdefFeaturePatcher，
            // InternalsVisibleTo），確保測試覆蓋的就是實際出貨行為（含實驗性功能旗標）。
            var engine = new PatchEngine(new NullLogger());
            var profile = new AgainstRomeModifier.Core.Features.PatchProfile {
                FastCiviProduction = true,
                InfiniteMorale = true,
                Balance = true,
                SpellDamage5x = true,
                SpellHealing10x = true,
                SpellResurrection = true,
                GeneralSkills = true,
                LeaderGlory = true
            };
            byte[] patchedCompressed = AgainstRomeModifier.Core.Features.Ini.IniFeaturePatcher.BuildClScript(backupManager, profile);
            byte[] patchedObjdef = AgainstRomeModifier.Core.Features.Objdef.ObjdefFeaturePatcher.Build(backupManager, profile);


            // 3. Decompress and verify
            byte[] decompressed = GameLZSS.DecompressPfil(patchedCompressed);
            string text = Encoding.GetEncoding(1251).GetString(decompressed);
            string cleanText = text.Replace(" ", "").Replace("\t", "");

            // Verify content
            Assert.Contains("CiviDelay=GER,500", cleanText);
            Assert.Contains("MoralsDecLostMem=GER,0", cleanText);
            Assert.Contains("MoralsDecFlee=GER,0", cleanText);
            Assert.Contains("MoralsDecOverPop=GER,99999999", cleanText);
            Assert.Contains("MoralsIncIdle=GER,500", cleanText);

            // 4. Verify detect logic in PatchEngine
            // DetectCurrentPatchState 讀取 gamePath 磁碟上的 cl_script.ini，
            // 以臨時目錄模擬遊戲目錄後再執行偵測。
            string tempDir = Path.Combine(Path.GetTempPath(), "AgainstRomeTest_" + Guid.NewGuid().ToString());
            Directory.CreateDirectory(tempDir);
            try
            {
                Directory.CreateDirectory(Path.Combine(tempDir, "SYSTEM"));
                File.WriteAllBytes(Path.Combine(tempDir, "SYSTEM", "cl_script.ini"), patchedCompressed);
                Directory.CreateDirectory(Path.Combine(tempDir, "SYSTEM", "DATA_MP", "DEFAULTS"));
                File.WriteAllBytes(Path.Combine(tempDir, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau"), patchedObjdef);

                // Run detection
                var detectedOptions = engine.DetectCurrentPatchState(tempDir, backupManager);

                Assert.True(detectedOptions.FastCiviProduction, "Fast Civi Production should be detected as true");
                Assert.True(detectedOptions.InfiniteMorale, "Infinite Morale should be detected as true");
                Assert.True(detectedOptions.SpellDamage5x, "Spell Damage 5x should be detected as true");
                Assert.True(detectedOptions.SpellHealing10x, "Spell Healing 10x should be detected as true");
                Assert.True(detectedOptions.SpellResurrection, "Spell Resurrection should be detected as true");
                Assert.True(detectedOptions.GeneralSkills, "General Skills should be detected as true");
                Assert.True(detectedOptions.LeaderGlory, "Leader Glory should be detected as true");
            }
            finally
            {
                if (Directory.Exists(tempDir))
                {
                    Directory.Delete(tempDir, true);
                }
            }
        }
    }

    public class NullLogger : AgainstRomeModifier.Core.Services.ILogger
    {
        public void Log(string message) { }
    }
}
