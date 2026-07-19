using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;

namespace AgainstRomeModifier
{
    public class EndlessAiOrchestrator
    {
        public EndlessAiModule M1 { get; }
        public EndlessAiModule RespawnCore { get; }
        public EndlessAiModule M2 { get; }
        public EndlessAiModule M3 { get; }
        public EndlessAiModule M4 { get; }
        public EndlessAiModule M5 { get; }
        public EndlessAiModule M6 { get; }
        public EndlessAiModule VillageGarrisonQuota3x { get; }
        public EndlessAiModule R0 { get; }

        public List<EndlessAiModule> UserModules { get; }

        private readonly P20_VillageGarrisonQuotaPatch _p20;
        private readonly Dictionary<string, BciScriptFile> _fileCache = new Dictionary<string, BciScriptFile>(StringComparer.OrdinalIgnoreCase);

        /// <summary>
        /// 設定村莊駐軍配額倍率（P20 helper 的乘法字面值）。1 表示停用，不改變目前選擇；
        /// 其餘值必須是 P20 支援的倍率（2/3/5/10）。
        /// </summary>
        public void SetVillageGarrisonQuotaMultiplier(int multiplier)
        {
            if (multiplier <= 1) return;
            _p20.Multiplier = multiplier;
        }

        /// <summary>
        /// 讀回目前安裝在 Dorfverteidigung.bci 中的駐軍配額倍率；未套用（或無法辨識）時回傳 1。
        /// </summary>
        public int DetectVillageGarrisonQuotaMultiplier(string gamePath)
        {
            var paths = ResolvePaths(gamePath, _p20.TargetPattern);
            if (paths.Count != 1) return 1;
            BciScriptFile file = GetOrCreateFile(paths[0]);
            return P20_VillageGarrisonQuotaPatch.TryReadInstalledMultiplier(file.DecompressedBytes, out int multiplier)
                ? multiplier
                : 1;
        }

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
                new int[] { 30000 },
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
            var p8 = new P8_ReinforcementUnitThresholdPatch();

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

            // P17: 羅馬奠基者生成門檻 — type-4 spawner 的 60% 機率閘 60 -> 100（必過）。
            // 先前僅有測試（RomanFounderGatePatchTests）而未接進任何模組，導致實際檔案仍為 60。
            var p17 = new BciLiteralPatch(
                "P17",
                "MAPS/ENDL_*/SCRIPT/ak_level.bci",
                new int?[] { 66, null, 66, 100, 66, 1, 128, 16, 73, -2, 86, 96, 101, 117 },
                new int[] { 1 },
                new int[] { 60 },
                new int[] { 100 },
                1
            );

            // P18: 重生據點解鎖 — 定居地點的單位鄰近檢查不再被玩家（team 0）單位否決。
            // fn0x858 回傳「地點半徑內有單位的最小隊伍編號」（0..7，玩家=0；無=-1），
            // fnA54 以 result >= <比較值> 判定地點被佔用。比較值 0->1 使玩家駐軍不再永久封鎖重生地點，
            // CPU 隊伍單位仍會否決。唯一錨點：檔案內唯一的 callint -636（fn0x858 呼叫）。
            var p18 = new BciLiteralPatch(
                "P18",
                "MAPS/ENDL_*/SCRIPT/ak_level.bci",
                new int?[] { 120, -636, 73, -3, 86, 66, null, 96, 101, 117, 20, 66, 0, 87 },
                new int[] { 6 },
                new int[] { 0 },
                new int[] { 1 },
                1
            );

            // P19: 重生據點解鎖 — 地點佔用判定半徑 2500 -> 800。
            // fn0x9904 對每個定居地點以此半徑呼叫 fnA54（村莊中心距離 + 單位搜尋共用同一半徑）。
            // 2500 會讓玩家後期擴張與死亡隊伍殘留把全部 8 個地點永久封死（重生停止的根因）；
            // 800 僅在地點近旁確實被佔用時才否決。唯一錨點：檔案內唯一的 callint -36800（fnA54 呼叫）。
            var p19 = new BciLiteralPatch(
                "P19",
                "MAPS/ENDL_*/SCRIPT/ak_level.bci",
                new int?[] { 66, null, 90, 1, 90, 0, 120, -36800, 73, -3, 86 },
                new int[] { 1 },
                new int[] { 2500 },
                new int[] { 800 },
                1
            );

            // P13: 聚落模板開局資源
            var p13 = new P13_SettlementTemplatePatch();

            // Restore the unsafe legacy DELETE_TEAM terminal transitions.
            var p15 = new P15_SettledPartyDeleteTeamPatch();

            // P14: 強制還原已被否決的修補
            var p14 = new P14_ForcedRestorePatch();
            _p20 = new P20_VillageGarrisonQuotaPatch();

            M1 = new EndlessAiModule("M1", "增援規模", new List<IEndlessPatch> { p1, p10, p12 });
            M2 = new EndlessAiModule("M2", "增援節奏", new List<IEndlessPatch> { p3, p6, p2 });
            M3 = new EndlessAiModule("M3", "敗亡快速回收", new List<IEndlessPatch> { p4, p5, p11 });
            M4 = new EndlessAiModule("M4", "強制部落生成", new List<IEndlessPatch> { p7, p17, p18, p19 });
            M5 = new EndlessAiModule("M5", "開局資源", new List<IEndlessPatch> { p13 });
            // Legacy module id M6 remains internal for profile migration. The
            // user-facing feature is RomanReinforcementGarrison; P8 and P9 stay
            // atomic because retained squads require the raised unit threshold.
            M6 = new EndlessAiModule("M6", "羅馬增援士兵留守", new List<IEndlessPatch> { p8, p9 });
            VillageGarrisonQuota3x = new EndlessAiModule(
                "VillageGarrisonQuota3x",
                "村莊 AI 駐軍配額倍率",
                new List<IEndlessPatch> { _p20 });
            R0 = new EndlessAiModule("R0", "常駐修復", new List<IEndlessPatch> { p14, p15 });

            RespawnCore = new EndlessAiModule(
                "Core",
                "無盡重生核心",
                M2.Patches.Concat(M3.Patches).Concat(M4.Patches).ToList());

            UserModules = new List<EndlessAiModule> { M1, RespawnCore, M5, M6, VillageGarrisonQuota3x };
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

        internal BciScriptFile GetScriptFile(string path) => GetOrCreateFile(path);

        public static List<string> ResolvePaths(string gamePath, string pattern)
        {
            var paths = new List<string>();
            string mapsPath = Path.Combine(gamePath, "MAPS");
            string systemPath = Path.Combine(gamePath, "SYSTEM");

            if (pattern == "MAPS/ENDL_*/SCRIPT/ak_level.bci")
            {
                if (Directory.Exists(mapsPath))
                {
                    foreach (string mapDirectory in Directory.GetDirectories(mapsPath, "ENDL_???", SearchOption.TopDirectoryOnly).OrderBy(x => x, StringComparer.OrdinalIgnoreCase))
                    {
                        string path = Path.Combine(mapDirectory, "SCRIPT", "ak_level.bci");
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

        public static int GetExpectedFileCount(string gamePath, string pattern)
        {
            if (pattern != "MAPS/ENDL_*/SCRIPT/ak_level.bci" && pattern != "MAPS/ENDL_*/Endlos_*_Siedlung*.sdl") return GetExpectedFileCount(pattern);
            return ResolvePaths(gamePath, pattern).Count;
        }

        public PatchState DetectModule(string gamePath, EndlessAiModule module)
        {
            bool allOriginal = true;
            bool allUltimate = true;
            bool anyFileFound = false;

            foreach (var patch in module.Patches)
            {
                var paths = ResolvePaths(gamePath, patch.TargetPattern);
                if (paths.Count > 0)
                {
                    anyFileFound = true;
                }
                int expectedCount = GetExpectedFileCount(gamePath, patch.TargetPattern);
                if (paths.Count != expectedCount)
                {
                    if (paths.Count == 0)
                    {
                        allUltimate = false;
                        continue;
                    }
                    // 檔案數量不符預期但仍有找到檔案，視為不完整套用的 Legacy 狀態，而不應直接阻斷為 Unknown
                    allOriginal = false;
                    allUltimate = false;
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

            // 一個檔案都找不到（路徑錯誤、MAPS 缺失）時不能宣稱「原版」——那是「無法判定」。
            if (!anyFileFound) return PatchState.Unknown;

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
            if (state == PatchState.Unknown && enabled)
            {
                throw new InvalidOperationException($"模組 {module.Name} ({module.Id}) 處於未知或不相容狀態，無法安全套用。");
            }
            if (!enabled && (state == PatchState.Original || state == PatchState.Unknown))
            {
                return false;
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
            Core.Services.SafeFileWriter.WriteAllBytes(dest, bytes, rollback);
        }
    }
}
