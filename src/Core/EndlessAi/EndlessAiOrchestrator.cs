using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;

namespace AgainstRomeModifier
{
    public class EndlessAiOrchestrator
    {
        public EndlessAiModule M1 { get; }
        public EndlessAiModule M2 { get; }
        public EndlessAiModule M3 { get; }
        public EndlessAiModule M4 { get; }
        public EndlessAiModule M5 { get; }
        public EndlessAiModule M6 { get; }
        public EndlessAiModule R0 { get; }

        public List<EndlessAiModule> UserModules { get; }

        private readonly Dictionary<string, BciScriptFile> _fileCache = new Dictionary<string, BciScriptFile>(StringComparer.OrdinalIgnoreCase);

        public EndlessAiOrchestrator()
        {
            // P1: 軍事增援單位人數
            var p1 = new BciLiteralPatch(
                "P1",
                "MAPS/ENDL_*/SCRIPT/ak_level.bci",
                new int?[] {
                    0x42, null, 0x42, 1, 0x42, null, 0x42, null, 0x42, 0, 0x42, 0, 0x42, 8, 0x42, 3, 0x5A, 7, 0x80, 0xD4, 0x49, unchecked((int)0xFFFFFFF7), 0x56
                },
                new int[] { 5, 7 },
                new int[] { 4, 4 },
                new int[] { 20, 20 },
                1
            );

            // P2: 完工 Job 自動回收旗標
            var p2 = new BciLiteralPatch(
                "P2",
                "MAPS/ENDL_*/SCRIPT/ak_level.bci",
                new int?[] {
                    0x42, null, 0x42, 1, 0x42, null, 0x42, null, 0x42, 0, 0x42, 0, 0x42, 8, 0x42, 3, 0x5A, 7, 0x80, 0xD4, 0x49, unchecked((int)0xFFFFFFF7), 0x56
                },
                new int[] { 1 },
                new int[] { 0 },
                new int[] { 1 },
                1
            );

            // P3: 軍事增援等待時間
            var p3 = new BciLiteralPatch(
                "P3",
                "MAPS/ENDL_*/SCRIPT/ak_level.bci",
                new int?[] {
                    0x80, 83, 0x56, 66, null, 32, 44, 164, 0x42, 34, 0x5B, 5
                },
                new int[] { 4 },
                new int[] { 180000 },
                new int[] { 5000 },
                1
            );

            // P4: 撤退期限加速
            var p4 = new P4_RetreatDeadlinePatch();

            // P5: 死亡黨確認去彈跳
            var p5 = new BciLiteralPatch(
                "P5",
                "MAPS/ENDL_*/SCRIPT/ak_level.bci",
                new int?[] {
                    0x5A, 24, 0x42, null, 96, 101, 117, 16, 0x42, 1, 0x5B, 17
                },
                new int[] { 3 },
                new int[] { 20 },
                new int[] { 3 },
                1
            );

            // P6: 排程迴圈延遲（6 站）
            var p6 = new P6_LoopDelayPatch();

            // P7: 聚落生成機率
            var p7 = new P7_SpawnProbabilitiesPatch();

            // P8: 增援單位數門檻
            var p8 = new P8_ActiveLimitPatch();

            // P9: 撤退配額歸零
            var p9 = new P9_RetreatQuotaPatch();

            // P10: 主營轉兵批量
            var p10 = new BciLiteralPatch(
                "P10",
                "SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci",
                new int?[] { null, null, 81, 11, 81, 10, 81, 98, 128, 81, 73, -4, 86 },
                new int[] { 0, 1 },
                new int[] { 81, 59 },
                new int[] { 66, 20 },
                1
            );

            // P11: 村莊拆除延遲
            var p11 = new BciLiteralPatch(
                "P11",
                "SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci",
                new int?[] {
                    66, null, 66, 25, 66, -25, 128, 34, 73, -2, 86, 32, 82, 14, 81, 14, 82, 15
                },
                new int[] { 1 },
                new int[] { 1500 },
                new int[] { 100 },
                1
            );

            // P12: 村防轉兵人數
            var p12 = new BciLiteralPatch(
                "P12",
                "SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci",
                new int?[] {
                    66, 0, 66, 1, 66, null, 66, null, 66, 0, 66, 0, 66, null, 66, 1, 90, 8, 128, 157, 73, -9, 86
                },
                new int[] { 5, 7 },
                new int[] { 6, 6 },
                new int[] { 20, 20 },
                4
            );

            // P13: 聚落模板開局資源
            var p13 = new P13_SettlementTemplatePatch();

            // P16: Keep four settled opponents without starving the separate type-4
            // military settlement and attack parties of team slots.  Vanilla adds
            // the type-4 settlement outside v70, so fix the type-1 cap at three.
            // Recognize the former 4/4 implementation as legacy and migrate it.
            var p16 = new BciLiteralPatch(
                "P16",
                "MAPS/ENDL_*/SCRIPT/ak_level.bci",
                new int?[] { 66, null, 66, null, 128, 16, 73, -2, 86, 82, 70 },
                new int[] { 1, 3 },
                new int[] { 4, 2 },
                new int[] { 3, 3 },
                1,
                (buffer, site) =>
                {
                    int upper = BitConverter.ToInt32(buffer, site + 4);
                    int lower = BitConverter.ToInt32(buffer, site + 12);
                    if (upper == 4 && lower == 2) return PatchState.Original;
                    if (upper == 3 && lower == 3) return PatchState.Ultimate;
                    if (upper == 4 && lower == 4) return PatchState.Legacy;
                    return PatchState.Unknown;
                }
            );

            // Restore the unsafe legacy DELETE_TEAM terminal transitions.
            var p15 = new P15_SettledPartyDeleteTeamPatch();

            // P14: 強制還原已被否決的修補
            var p14 = new P14_ForcedRestorePatch();

            M1 = new EndlessAiModule("M1", "增援規模", new List<IEndlessPatch> { p1, p10, p12 });
            M2 = new EndlessAiModule("M2", "增援節奏", new List<IEndlessPatch> { p3, p6, p2 });
            M3 = new EndlessAiModule("M3", "敗亡快速回收", new List<IEndlessPatch> { p4, p5, p11 });
            M4 = new EndlessAiModule("M4", "保證聚落生成與留守", new List<IEndlessPatch> { p7, p8, p9 });
            M5 = new EndlessAiModule("M5", "開局資源", new List<IEndlessPatch> { p13 });
            M6 = new EndlessAiModule("M6", "四個定居 AI 配額", new List<IEndlessPatch> { p16 });
            R0 = new EndlessAiModule("R0", "常駐修復", new List<IEndlessPatch> { p14, p15 });

            UserModules = new List<EndlessAiModule> { M1, M2, M3, M4, M5, M6 };
        }

        public void ClearCache()
        {
            _fileCache.Clear();
        }

        private BciScriptFile GetOrCreateFile(string path)
        {
            if (!_fileCache.TryGetValue(path, out var file))
            {
                file = new BciScriptFile(path);
                _fileCache[path] = file;
            }
            return file;
        }

        public static List<string> ResolvePaths(string gamePath, string pattern)
        {
            var paths = new List<string>();
            string mapsPath = Path.Combine(gamePath, "MAPS");
            string systemPath = Path.Combine(gamePath, "SYSTEM");

            if (pattern == "MAPS/ENDL_*/SCRIPT/ak_level.bci")
            {
                if (Directory.Exists(mapsPath))
                {
                    for (int i = 0; i < 5; i++)
                    {
                        string path = Path.Combine(mapsPath, $"ENDL_{i:000}", "SCRIPT", "ak_level.bci");
                        if (File.Exists(path))
                        {
                            paths.Add(path);
                        }
                    }
                }
            }
            else if (pattern == "SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci")
            {
                string path = Path.Combine(systemPath, "CLAK", "SCRIPT", "ak_haupthaus.bci");
                if (File.Exists(path)) paths.Add(path);
            }
            else if (pattern == "SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci")
            {
                string path = Path.Combine(systemPath, "CLAK", "SCRIPT", "Dorfverteidigung.bci");
                if (File.Exists(path)) paths.Add(path);
            }
            else if (pattern == "MAPS/ENDL_*/Endlos_*_Siedlung*.sdl")
            {
                if (Directory.Exists(mapsPath))
                {
                    string[] templates = Directory.GetFiles(mapsPath, "Endlos_*_Siedlung*.sdl", SearchOption.AllDirectories)
                        .Where(p => p.IndexOf(Path.DirectorySeparatorChar + "ENDL_", StringComparison.OrdinalIgnoreCase) >= 0)
                        .ToArray();
                    paths.AddRange(templates);
                }
            }
            else if (pattern == "SYSTEM/CLAK/SCRIPT/ak_npc.bci|SYSTEM/CLAK/SCRIPT/ak_produktion.bci")
            {
                string pathNpc = Path.Combine(systemPath, "CLAK", "SCRIPT", "ak_npc.bci");
                string pathProd = Path.Combine(systemPath, "CLAK", "SCRIPT", "ak_produktion.bci");
                if (File.Exists(pathNpc)) paths.Add(pathNpc);
                if (File.Exists(pathProd)) paths.Add(pathProd);
            }
            return paths;
        }

        public static int GetExpectedFileCount(string pattern)
        {
            if (pattern == "MAPS/ENDL_*/SCRIPT/ak_level.bci") return 5;
            if (pattern == "SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci") return 1;
            if (pattern == "SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci") return 1;
            if (pattern == "MAPS/ENDL_*/Endlos_*_Siedlung*.sdl") return 42;
            if (pattern == "SYSTEM/CLAK/SCRIPT/ak_npc.bci|SYSTEM/CLAK/SCRIPT/ak_produktion.bci") return 2;
            return 0;
        }

        public PatchState DetectModule(string gamePath, EndlessAiModule module)
        {
            bool allOriginal = true;
            bool allUltimate = true;

            foreach (var patch in module.Patches)
            {
                var paths = ResolvePaths(gamePath, patch.TargetPattern);
                int expectedCount = GetExpectedFileCount(patch.TargetPattern);
                if (paths.Count != expectedCount)
                {
                    if (paths.Count == 0)
                    {
                        allUltimate = false;
                        continue;
                    }
                    return PatchState.Unknown;
                }

                foreach (string path in paths)
                {
                    BciScriptFile file = GetOrCreateFile(path);
                    PatchState state = patch.Detect(file.DecompressedBytes);
                    if (state == PatchState.Unknown) return PatchState.Unknown;
                    if (state == PatchState.Legacy)
                    {
                        allOriginal = false;
                        allUltimate = false;
                    }
                    else if (state == PatchState.Original)
                    {
                        allUltimate = false;
                    }
                    else if (state == PatchState.Ultimate)
                    {
                        allOriginal = false;
                    }
                }
            }

            if (allOriginal) return PatchState.Original;
            if (allUltimate) return PatchState.Ultimate;
            return PatchState.Legacy;
        }

        public PatchState DetectGlobalState(string gamePath)
        {
            bool allOriginal = true;
            bool allUltimate = true;

            foreach (var module in UserModules)
            {
                PatchState state = DetectModule(gamePath, module);
                if (state == PatchState.Unknown) return PatchState.Unknown;
                if (state == PatchState.Legacy)
                {
                    allOriginal = false;
                    allUltimate = false;
                }
                else if (state == PatchState.Original)
                {
                    allUltimate = false;
                }
                else if (state == PatchState.Ultimate)
                {
                    allOriginal = false;
                }
            }

            // Also check mandatory repair pass (R0). It should be strictly clean (Original).
            // If R0 is legacy, it needs repair, so we report legacy overall.
            PatchState r0State = DetectModule(gamePath, R0);
            if (r0State == PatchState.Unknown) return PatchState.Unknown;
            if (r0State == PatchState.Legacy)
            {
                allOriginal = false;
                allUltimate = false;
            }

            if (allOriginal) return PatchState.Original;
            if (allUltimate) return PatchState.Ultimate;
            return PatchState.Legacy;
        }

        public bool ApplyModule(string gamePath, EndlessAiModule module, bool enabled)
        {
            var state = DetectModule(gamePath, module);
            if (state == PatchState.Unknown)
            {
                throw new InvalidOperationException($"模組 {module.Name} ({module.Id}) 處於未知或不相容狀態，無法安全套用。");
            }

            bool changed = false;
            foreach (var patch in module.Patches)
            {
                var paths = ResolvePaths(gamePath, patch.TargetPattern);
                foreach (string path in paths)
                {
                    BciScriptFile file = GetOrCreateFile(path);
                    byte[] decomp = file.DecompressedBytes;
                    if (patch.Apply(ref decomp, enabled))
                    {
                        file.UpdateDecompressedBytes(decomp);
                        changed = true;
                    }
                }
            }
            return changed;
        }

        public bool ApplyMandatoryRepair(string gamePath)
        {
            // R0 is mandatory repair, always "enabled" means restore to original.
            bool changed = false;
            foreach (var patch in R0.Patches)
            {
                var paths = ResolvePaths(gamePath, patch.TargetPattern);
                foreach (string path in paths)
                {
                    BciScriptFile file = GetOrCreateFile(path);
                    byte[] decomp = file.DecompressedBytes;
                    if (patch.Apply(ref decomp, false))
                    {
                        file.UpdateDecompressedBytes(decomp);
                        changed = true;
                    }
                }
            }
            return changed;
        }

        public void SaveAll(string gamePath, FileRollbackScope? rollback)
        {
            foreach (var kvp in _fileCache)
            {
                BciScriptFile file = kvp.Value;
                if (file.IsModified)
                {
                    byte[] compressed = file.GetCompressedBytes();
                    SafeWriteAllBytes(file.FilePath, compressed, rollback);
                }
            }
        }

        private static void SafeWriteAllBytes(string dest, byte[] bytes, FileRollbackScope? rollback = null)
        {
            int maxRetries = 3;
            int delayMs = 500;
            string? dir = Path.GetDirectoryName(dest);
            if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
            {
                Directory.CreateDirectory(dir);
            }

            for (int i = 0; i < maxRetries; i++)
            {
                string tempFile = Path.Combine(dir ?? AppContext.BaseDirectory, Path.GetFileName(dest) + "." + Guid.NewGuid().ToString("N") + ".tmp");
                try
                {
                    rollback?.TrackFile(dest);
                    File.WriteAllBytes(tempFile, bytes);

                    if (File.Exists(dest))
                    {
                        File.SetAttributes(dest, FileAttributes.Normal);
                        File.Replace(tempFile, dest, null, true);
                    }
                    else
                    {
                        File.Move(tempFile, dest);
                    }
                    return;
                }
                catch (IOException ioEx)
                {
                    try
                    {
                        if (File.Exists(tempFile))
                        {
                            File.SetAttributes(tempFile, FileAttributes.Normal);
                            File.Delete(tempFile);
                        }
                    }
                    catch { }

                    if (i == maxRetries - 1)
                    {
                        throw new Exception(string.Format("寫入檔案失敗，檔案可能被佔用或權限不足：{0}。錯誤訊息：{1}", dest, ioEx.Message), ioEx);
                    }
                    System.Threading.Thread.Sleep(delayMs);
                }
                catch (UnauthorizedAccessException accessEx)
                {
                    try
                    {
                        if (File.Exists(tempFile))
                        {
                            File.SetAttributes(tempFile, FileAttributes.Normal);
                            File.Delete(tempFile);
                        }
                    }
                    catch { }

                    if (i == maxRetries - 1)
                    {
                        throw new Exception(string.Format("寫入檔案失敗，檔案可能被佔用或權限不足：{0}。錯誤訊息：{1}", dest, accessEx.Message), accessEx);
                    }
                    System.Threading.Thread.Sleep(delayMs);
                }
            }
        }
    }
}
