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
        [Fact]
        public void Test_ClScript_Patch_And_Detect()
        {
            Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

            // 1. Prepare backup manager with real embedded backup
            var backupManager = new BackupManager(new NullLogger());
            backupManager.LoadBackupZipToMemory(null);

            Assert.True(backupManager.BackupFiles.ContainsKey("SYSTEM/cl_script.ini"), "Embedded Backup.zip should contain cl_script.ini");

            // 2. Patch
            var engine = new PatchEngine(new NullLogger());
            var method = typeof(PatchEngine).GetMethod("GetPatchedClScriptBytes", 
                System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance);
            Assert.NotNull(method);

            // fastCiviProduction=true, infiniteMoraleChecked=true, balanceChecked=true
            byte[] patchedCompressed = (byte[])method.Invoke(engine, new object[] { "C:\\dummy", backupManager, true, true, true })!;


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
            var options = engine.DetectCurrentPatchState("C:\\dummy", backupManager);
            // Wait, DetectCurrentPatchState reads cl_script.ini from the gamePath on disk.
            // Let's create a temp directory to simulate the game directory
            string tempDir = Path.Combine(Path.GetTempPath(), "AgainstRomeTest_" + Guid.NewGuid().ToString());
            Directory.CreateDirectory(tempDir);
            try
            {
                Directory.CreateDirectory(Path.Combine(tempDir, "SYSTEM"));
                File.WriteAllBytes(Path.Combine(tempDir, "SYSTEM", "cl_script.ini"), patchedCompressed);

                // Run detection
                var detectedOptions = engine.DetectCurrentPatchState(tempDir, backupManager);

                Assert.True(detectedOptions.FastCiviProduction, "Fast Civi Production should be detected as true");
                Assert.True(detectedOptions.InfiniteMorale, "Infinite Morale should be detected as true");
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
