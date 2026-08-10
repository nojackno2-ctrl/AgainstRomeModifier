using System.Text;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Tests
{
    public class PhysicalBackupTests
    {
        private void PrepareCleanGameDir(string sourceRoot, string gameDir)
        {
            Directory.CreateDirectory(gameDir);
            
            // 複製必要檔案
            CopyFile(sourceRoot, gameDir, "Against_Rome.exe");
            CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "cl_script.ini"));
            CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "cl_epara.ini"));
            CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "ress.ini"));
            CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "CLMK", "icon.ini"));
            CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "CLAK", "cl_scint.ini"));
            CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "CLAK", "SCRIPT", "ak_anfuehrer.bci"));
            CopyFile(sourceRoot, gameDir, Path.Combine("SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau"));
            
            // 複製至少一個 MAPS 下的 team.dat
            string sourceMaps = Path.Combine(sourceRoot, "MAPS");
            if (Directory.Exists(sourceMaps))
            {
                foreach (string file in Directory.GetFiles(sourceMaps, "team.dat", SearchOption.AllDirectories))
                {
                    string relPath = Path.GetRelativePath(sourceRoot, file);
                    CopyFile(sourceRoot, gameDir, relPath);
                    break; // 只要複製一個做測試即可
                }
            }
        }

        private void CopyFile(string sourceRoot, string destRoot, string relPath)
        {
            string src = Path.Combine(sourceRoot, relPath);
            string dest = Path.Combine(destRoot, relPath);
            string? dir = Path.GetDirectoryName(dest);
            if (dir != null && !Directory.Exists(dir))
            {
                Directory.CreateDirectory(dir);
            }
            if (File.Exists(src))
            {
                File.Copy(src, dest, true);
            }
        }

        [Fact]
        public void TryLoadBackupFromGameDirectory_FirstTime_CreatesBakFiles()
        {
            Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
            string sourceRoot = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../遊戲原始檔案"));
            if (!Directory.Exists(sourceRoot))
            {
                return; // CI 環境跳過
            }

            string gameDir = Path.Combine(Path.GetTempPath(), "ARM_PhysBak_Create_" + Guid.NewGuid().ToString("N"));
            try
            {
                PrepareCleanGameDir(sourceRoot, gameDir);

                var backupManager = new BackupManager(new NullLogger());
                bool result = backupManager.TryLoadBackupFromGameDirectory(gameDir, true);

                Assert.True(result);
                Assert.True(backupManager.HasFile("Against_Rome.exe"));
                Assert.True(backupManager.HasFile("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"));

                // 驗證 .bak 檔案是否已被建立在硬碟上
                Assert.True(File.Exists(Path.Combine(gameDir, "Against_Rome.exe.bak")));
                Assert.True(File.Exists(Path.Combine(gameDir, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau.bak")));
            }
            finally
            {
                if (Directory.Exists(gameDir))
                {
                    Directory.Delete(gameDir, true);
                }
            }
        }

        [Fact]
        public void TryLoadBackupFromGameDirectory_SecondTime_LoadsFromBakFiles()
        {
            Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
            string sourceRoot = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../遊戲原始檔案"));
            if (!Directory.Exists(sourceRoot))
            {
                return; // CI 環境跳過
            }

            string gameDir = Path.Combine(Path.GetTempPath(), "ARM_PhysBak_Load_" + Guid.NewGuid().ToString("N"));
            try
            {
                PrepareCleanGameDir(sourceRoot, gameDir);

                // 第一次加載並備份
                var backupManager1 = new BackupManager(new NullLogger());
                backupManager1.TryLoadBackupFromGameDirectory(gameDir, true);

                // 故意破壞/修改硬碟上的原版檔案
                string exePath = Path.Combine(gameDir, "Against_Rome.exe");
                string objdefPath = Path.Combine(gameDir, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau");
                
                File.WriteAllText(exePath, "DAMAGED EXE DATA");
                File.WriteAllText(objdefPath, "DAMAGED OBJDEF DATA");

                // 第二次啟動新 BackupManager 加載，應該依然要成功（因為從 .bak 讀取）
                var backupManager2 = new BackupManager(new NullLogger());
                bool result = backupManager2.TryLoadBackupFromGameDirectory(gameDir, true);

                Assert.True(result);
                // 驗證讀入記憶體中的不是破壞後的資料，而是原版長度
                byte[] loadedExeBytes = backupManager2.GetBackupBytes("Against_Rome.exe");
                Assert.True(loadedExeBytes.Length > 1000);
                Assert.NotEqual((byte)'D', loadedExeBytes[0]);
            }
            finally
            {
                if (Directory.Exists(gameDir))
                {
                    Directory.Delete(gameDir, true);
                }
            }
        }

        [Fact]
        public void TryLoadBackupFromGameDirectory_DirtyOriginal_ThrowsException()
        {
            Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
            string sourceRoot = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../遊戲原始檔案"));
            if (!Directory.Exists(sourceRoot))
            {
                return; // CI 環境跳過
            }

            string gameDir = Path.Combine(Path.GetTempPath(), "ARM_PhysBak_Dirty_" + Guid.NewGuid().ToString("N"));
            try
            {
                PrepareCleanGameDir(sourceRoot, gameDir);

                // 故意把 Against_Rome.exe 弄髒（但沒有 .bak 檔案）
                string exePath = Path.Combine(gameDir, "Against_Rome.exe");
                byte[] rawExe = File.ReadAllBytes(exePath);
                // 弄壞 EXE 的 signature 偏移處的數值
                Buffer.BlockCopy(new byte[] { 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 }, 0, rawExe, (int)ExePatchModel.FocusPatchOffset, 6);
                File.WriteAllBytes(exePath, rawExe);

                var backupManager = new BackupManager(new NullLogger());
                
                // 預期因為防呆校驗不通過，會拋出 InvalidDataException
                Assert.Throws<InvalidDataException>(() => backupManager.TryLoadBackupFromGameDirectory(gameDir, true));

                // 驗證此時沒有建立任何 .bak 檔案（避免把髒的 EXE 備份了）
                Assert.False(File.Exists(Path.Combine(gameDir, "Against_Rome.exe.bak")));
            }
            finally
            {
                if (Directory.Exists(gameDir))
                {
                    Directory.Delete(gameDir, true);
                }
            }
        }

        [Fact]
        public void InspectOriginalObjdef()
        {
            Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
            string sourceRoot = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "../../../../../遊戲原始檔案"));
            string path = Path.Combine(sourceRoot, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau");
            if (File.Exists(path))
            {
                byte[] bytes = File.ReadAllBytes(path);
                byte[] decomp = GameLZSS.DecompressPfil(bytes);
                string text = Encoding.GetEncoding(1251).GetString(decomp);
                string lineEnding = text.Contains("\r\n") ? "\r\n" : "\n";
                string[] lines = text.Split(new string[] { lineEnding }, StringSplitOptions.None);
                var found = new List<string>();
                foreach (string line in lines)
                {
                    if (line.Contains("FigRomAnf00_Anfuehrer"))
                    {
                        found.Add(line);
                    }
                }
                Assert.True(found.Count > 0);
            }
        }
    }
}
