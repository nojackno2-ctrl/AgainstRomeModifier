using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Text;
using AgainstRomeModifier;

namespace AgainstRomeModifierTests {
    class Program {
        static int Main(string[] args) {
            Console.WriteLine("==================================================");
            Console.WriteLine("開始執行 Endless AI Split Patches Isomorphic 驗證");
            Console.WriteLine("==================================================");

            try {
                // Find project root
                string baseDir = AppContext.BaseDirectory;
                string projectRoot = baseDir;
                while (!Directory.Exists(Path.Combine(projectRoot, "遊戲原始檔案")) && Directory.GetParent(projectRoot) != null) {
                    projectRoot = Directory.GetParent(projectRoot)!.FullName;
                }

                string gameSourcePath = Path.Combine(projectRoot, "遊戲原始檔案");
                if (!Directory.Exists(gameSourcePath)) {
                    Console.WriteLine($"[錯誤] 找不到遊戲原始檔案目錄於: {gameSourcePath}");
                    return 1;
                }

                Console.WriteLine($"找到遊戲原始檔案目錄: {gameSourcePath}");

                // Create temp directory for validation
                string tempDir = Path.Combine(Path.GetTempPath(), "AgainstRomeModifierTest_" + Guid.NewGuid().ToString("N"));
                Directory.CreateDirectory(tempDir);
                Console.WriteLine($"建立臨時測試目錄: {tempDir}");

                var originalBytesDict = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);

                // Scan and copy related files from 遊戲原始檔案
                string[] allFiles = Directory.GetFiles(gameSourcePath, "*.*", SearchOption.AllDirectories);
                foreach (string file in allFiles) {
                    string relPath = Path.GetRelativePath(gameSourcePath, file);
                    
                    // We only extract files that our Endless AI code might touch:
                    // 1. ENDL_* level scripts (ak_level.bci)
                    // 2. Endlos_*_Siedlung*.sdl template files
                    // 3. NPC / economy BCI scripts under SYSTEM\CLAK\SCRIPT
                    bool isEndlessLevelScript = relPath.StartsWith(@"MAPS\", StringComparison.OrdinalIgnoreCase) && 
                                                relPath.Contains(@"\ENDL_", StringComparison.OrdinalIgnoreCase) && 
                                                relPath.EndsWith(@"\ak_level.bci", StringComparison.OrdinalIgnoreCase);
                                                
                    bool isEndlessSdl = relPath.StartsWith(@"MAPS\", StringComparison.OrdinalIgnoreCase) && 
                                        relPath.Contains(@"\ENDL_", StringComparison.OrdinalIgnoreCase) && 
                                        Path.GetFileName(relPath).StartsWith("Endlos_", StringComparison.OrdinalIgnoreCase) && 
                                        relPath.EndsWith(".sdl", StringComparison.OrdinalIgnoreCase);
                                        
                    bool isNpcScript = relPath.StartsWith(@"SYSTEM\CLAK\SCRIPT\", StringComparison.OrdinalIgnoreCase) && 
                                       (relPath.EndsWith("ak_npc.bci", StringComparison.OrdinalIgnoreCase) || 
                                        relPath.EndsWith("ak_produktion.bci", StringComparison.OrdinalIgnoreCase) || 
                                        relPath.EndsWith("ak_haupthaus.bci", StringComparison.OrdinalIgnoreCase) || 
                                        relPath.EndsWith("Dorfverteidigung.bci", StringComparison.OrdinalIgnoreCase));

                    if (isEndlessLevelScript || isEndlessSdl || isNpcScript) {
                        string destPath = Path.Combine(tempDir, relPath);
                        string? dir = Path.GetDirectoryName(destPath);
                        if (dir != null && !Directory.Exists(dir)) {
                            Directory.CreateDirectory(dir);
                        }

                        byte[] fileBytes = File.ReadAllBytes(file);
                        File.WriteAllBytes(destPath, fileBytes);
                        originalBytesDict[relPath] = fileBytes;
                    }
                }

                Console.WriteLine($"已複製 {originalBytesDict.Count} 個測試檔案。");
                if (originalBytesDict.Count == 0) {
                    Console.WriteLine("[錯誤] 未複製到任何測試檔案，請檢查遊戲原始檔案內容。");
                    return 1;
                }

                // Initialize orchestrator
                var orchestrator = new EndlessAiOrchestrator();

                // Test 1: Detect clean original state
                Console.WriteLine("測試 1：偵測原始原版狀態...");

                // Detailed debug of module and patch states
                foreach (var module in new[] { orchestrator.M1, orchestrator.M2, orchestrator.M3, orchestrator.M4, orchestrator.M5, orchestrator.M6, orchestrator.R0 }) {
                    var mState = orchestrator.DetectModule(tempDir, module);
                    Console.WriteLine($"Module {module.Name} ({module.Id}) state: {mState}");
                    foreach (var patch in module.Patches) {
                        var paths = EndlessAiOrchestrator.ResolvePaths(tempDir, patch.TargetPattern);
                        Console.WriteLine($"  Patch {patch.GetType().Name} pattern: {patch.TargetPattern}, resolved files: {paths.Count}");
                        foreach (var path in paths) {
                            var decomp = File.ReadAllBytes(path);
                            if (path.EndsWith(".bci", StringComparison.OrdinalIgnoreCase) || path.EndsWith(".sdl", StringComparison.OrdinalIgnoreCase)) {
                                decomp = GameLZSS.DecompressPfil(decomp);
                            }
                            var pState = patch.Detect(decomp);
                            Console.WriteLine($"    File: {Path.GetFileName(path)} state: {pState}");
                        }
                    }
                }

                var stateBefore = orchestrator.DetectGlobalState(tempDir);
                Console.WriteLine($"偵測結果: {stateBefore}");
                if (stateBefore != PatchState.Original) {
                    Console.WriteLine($"[失敗] 預期原始狀態為 {PatchState.Original}，但得到 {stateBefore}");
                    return 1;
                }
                Console.WriteLine("[成功] 測試 1 通過。");

                // Test 2: Apply Ultimate Mode
                Console.WriteLine("測試 2：套用 AI 終極模式 (Ultimate)...");
                var rollback = new FileRollbackScope();
                foreach (var module in orchestrator.UserModules) {
                    orchestrator.ApplyModule(tempDir, module, true);
                }
                orchestrator.ApplyMandatoryRepair(tempDir);
                orchestrator.SaveAll(tempDir, rollback);
                rollback.Commit();
                rollback.Dispose();

                var stateAfterApply = orchestrator.DetectGlobalState(tempDir);
                Console.WriteLine($"偵測結果: {stateAfterApply}");
                if (stateAfterApply != PatchState.Ultimate) {
                    Console.WriteLine($"[失敗] 預期套用後狀態為 {PatchState.Ultimate}，但得到 {stateAfterApply}");
                    return 1;
                }
                Console.WriteLine("[成功] 測試 2 通過。");

                // Test 2.5: 獨立 golden 值驗證。
                // Detect/Apply 共用同一份常數，若某個「終極值」被寫錯但前後自洽，
                // Test 2/3/4 都會照樣通過。本測試用逆向文件記載的預期值（新鮮硬編碼於此，
                // 不引用產品常數）逐一斷言終極輸出的實際數字，堵住這個盲點。
                Console.WriteLine("測試 2.5：獨立 golden 值驗證（對照逆向文件的預期終極值）...");
                if (!VerifyUltimateGoldenValues(orchestrator, tempDir)) {
                    return 1;
                }
                Console.WriteLine("[成功] 測試 2.5 通過。");

                // Test 3: Restore to Original
                Console.WriteLine("測試 3：還原為官方原版 (Original)...");
                rollback = new FileRollbackScope();
                foreach (var module in orchestrator.UserModules) {
                    orchestrator.ApplyModule(tempDir, module, false);
                }
                orchestrator.ApplyMandatoryRepair(tempDir);
                orchestrator.SaveAll(tempDir, rollback);
                rollback.Commit();
                rollback.Dispose();

                var stateAfterRestore = orchestrator.DetectGlobalState(tempDir);
                Console.WriteLine($"偵測結果: {stateAfterRestore}");
                if (stateAfterRestore != PatchState.Original) {
                    Console.WriteLine($"[失敗] 預期還原後狀態為 {PatchState.Original}，但得到 {stateAfterRestore}");
                    return 1;
                }
                Console.WriteLine("[成功] 測試 3 通過。");

                // Test 4: Byte-for-byte verification (Isomorphism check of decompressed content)
                Console.WriteLine("測試 4：比對還原後的解壓內容與原始備份是否 100% 同位素 (Isomorphic)...");
                foreach (var kvp in originalBytesDict) {
                    string relPath = kvp.Key;
                    byte[] expectedCompressed = kvp.Value;
                    string actualPath = Path.Combine(tempDir, relPath);
                    if (!File.Exists(actualPath)) {
                        Console.WriteLine($"[失敗] 還原後找不到檔案: {relPath}");
                        return 1;
                    }
                    byte[] actualCompressed = File.ReadAllBytes(actualPath);

                    byte[] expectedDecompressed = expectedCompressed;
                    byte[] actualDecompressed = actualCompressed;

                    if (relPath.EndsWith(".bci", StringComparison.OrdinalIgnoreCase) || relPath.EndsWith(".sdl", StringComparison.OrdinalIgnoreCase)) {
                        expectedDecompressed = GameLZSS.DecompressPfil(expectedCompressed);
                        actualDecompressed = GameLZSS.DecompressPfil(actualCompressed);
                    }

                    if (expectedDecompressed.Length != actualDecompressed.Length) {
                        Console.WriteLine($"[失敗] 解壓後長度不符: {relPath}，預期 {expectedDecompressed.Length}，實際 {actualDecompressed.Length}");
                        return 1;
                    }
                    for (int i = 0; i < expectedDecompressed.Length; i++) {
                        if (expectedDecompressed[i] != actualDecompressed[i]) {
                            Console.WriteLine($"[失敗] 解壓內容不符: {relPath} 於位元組偏移量 {i}");
                            Console.WriteLine($"  預期: Hex {expectedDecompressed[i]:X2} ('{(char)expectedDecompressed[i]}')");
                            Console.WriteLine($"  實際: Hex {actualDecompressed[i]:X2} ('{(char)actualDecompressed[i]}')");
                            // Show surrounding bytes in hex
                            int start = Math.Max(0, i - 20);
                            int end = Math.Min(expectedDecompressed.Length - 1, i + 20);
                            Console.WriteLine("預期周邊: " + string.Join(" ", expectedDecompressed.Skip(start).Take(end - start + 1).Select(b => b.ToString("X2"))));
                            Console.WriteLine("實際周邊: " + string.Join(" ", actualDecompressed.Skip(start).Take(end - start + 1).Select(b => b.ToString("X2"))));
                            return 1;
                        }
                    }
                }
                Console.WriteLine("[成功] 測試 4 通過。所有還原後檔案之解壓內容與原版檔案 100% 相同！");

                // Test 5: 逐模組獨立套用（驗證新 UI 各模組獨立勾選的底層機制）。
                // 只啟用 M1、M4 與 M6（M4 含 P8+P9 硬耦合），其餘保持關閉，驗證各模組狀態互不干擾。
                Console.WriteLine("測試 5：混合模組狀態（僅啟用 M1、M4、M6）...");
                rollback = new FileRollbackScope();
                var mixed = new Dictionary<string, bool> {
                    { "M1", true }, { "M2", false }, { "M3", false }, { "M4", true }, { "M5", false }, { "M6", true }
                };
                foreach (var module in orchestrator.UserModules) {
                    orchestrator.ApplyModule(tempDir, module, mixed[module.Id]);
                }
                orchestrator.ApplyMandatoryRepair(tempDir);
                orchestrator.SaveAll(tempDir, rollback);
                rollback.Commit();
                rollback.Dispose();

                bool mixedOk = true;
                foreach (var module in orchestrator.UserModules) {
                    var expected = mixed[module.Id] ? PatchState.Ultimate : PatchState.Original;
                    var actual = orchestrator.DetectModule(tempDir, module);
                    Console.WriteLine($"  模組 {module.Name} ({module.Id})：預期 {expected}，實際 {actual}");
                    if (actual != expected) mixedOk = false;
                }
                if (!mixedOk) {
                    Console.WriteLine("[失敗] 混合模組狀態偵測與預期不符——模組間可能互相干擾。");
                    return 1;
                }
                Console.WriteLine("[成功] 測試 5 通過：各模組狀態互相獨立。");

                // 收尾：全部還原並再次驗證 byte-for-byte 無損。
                Console.WriteLine("測試 5.1：混合狀態全部還原後再次比對無損...");
                rollback = new FileRollbackScope();
                foreach (var module in orchestrator.UserModules) {
                    orchestrator.ApplyModule(tempDir, module, false);
                }
                orchestrator.ApplyMandatoryRepair(tempDir);
                orchestrator.SaveAll(tempDir, rollback);
                rollback.Commit();
                rollback.Dispose();
                foreach (var kvp in originalBytesDict) {
                    string actualPath = Path.Combine(tempDir, kvp.Key);
                    byte[] exp = kvp.Value, act = File.ReadAllBytes(actualPath);
                    if (kvp.Key.EndsWith(".bci", StringComparison.OrdinalIgnoreCase) || kvp.Key.EndsWith(".sdl", StringComparison.OrdinalIgnoreCase)) {
                        exp = GameLZSS.DecompressPfil(exp);
                        act = GameLZSS.DecompressPfil(act);
                    }
                    if (exp.Length != act.Length || !exp.SequenceEqual(act)) {
                        Console.WriteLine($"[失敗] 混合狀態還原後檔案不符: {kvp.Key}");
                        return 1;
                    }
                }
                Console.WriteLine("[成功] 測試 5.1 通過。");

                // Cleanup temp dir
                try {
                    Directory.Delete(tempDir, true);
                    Console.WriteLine("已清理臨時測試目錄。");
                } catch {
                    // Ignore
                }

                Console.WriteLine("\n==================================================");
                Console.WriteLine("驗證成功！新架構無損還原與檢測與舊版 100% 同位素。");
                Console.WriteLine("==================================================");
                return 0;

            } catch (Exception ex) {
                Console.WriteLine($"[錯誤] 執行驗證時拋出異常: {ex.Message}\n{ex.StackTrace}");
                return 1;
            }
        }

        // ------------------------------------------------------------------
        // 獨立 golden 值驗證：預期值全部在此新鮮硬編碼，來源為
        // docs/reverse-engineering/endless-mode-ai.md，刻意不引用產品程式碼的常數，
        // 以確保「產品端某個終極值被改錯」時本測試會失敗。
        // 定位站點沿用 BciPattern（結構比對）；被斷言的是各站點的實際「數值」。
        // ------------------------------------------------------------------
        static bool VerifyUltimateGoldenValues(EndlessAiOrchestrator orchestrator, string tempDir) {
            var failures = new List<string>();
            void Fail(string msg) => failures.Add(msg);
            int I32(byte[] b, int off) => BitConverter.ToInt32(b, off);

            byte[] Load(string path) {
                byte[] raw = File.ReadAllBytes(path);
                return GameLZSS.DecompressPfil(raw);
            }

            // ---- ak_level.bci ×5 ----
            var akLevelPaths = EndlessAiOrchestrator.ResolvePaths(tempDir, "MAPS/ENDL_*/SCRIPT/ak_level.bci");
            if (akLevelPaths.Count != 5) Fail($"ak_level.bci 應為 5 個，實為 {akLevelPaths.Count}");
            foreach (string path in akLevelPaths) {
                string name = Path.GetFileName(Path.GetDirectoryName(Path.GetDirectoryName(path))!);
                byte[] d = Load(path);

                // P1 軍事增援人數 20,20；P2 完工回收旗標 1
                int?[] createSig = { 0x42, null, 0x42, 1, 0x42, null, 0x42, null, 0x42, 0, 0x42, 0, 0x42, 8, 0x42, 3, 0x5A, 7, 0x80, 0xD4, 0x49, unchecked((int)0xFFFFFFF7), 0x56 };
                int c = BciPattern.FindBciWordPattern(d, createSig);
                if (c < 0) Fail($"{name}: 找不到 P1/P2 建立單位簽章");
                else {
                    if (I32(d, c + 20) != 20 || I32(d, c + 28) != 20) Fail($"{name}: P1 增援人數應為 20,20，實為 {I32(d, c + 20)},{I32(d, c + 28)}");
                    if (I32(d, c + 4) != 1) Fail($"{name}: P2 回收旗標應為 1，實為 {I32(d, c + 4)}");
                }

                // P3 軍事增援等待 5000ms
                int?[] respawnSig = { 0x80, 83, 0x56, 66, null, 32, 44, 164, 0x42, 34, 0x5B, 5 };
                int r = BciPattern.FindBciWordPattern(d, respawnSig);
                if (r < 0) Fail($"{name}: 找不到 P3 增援等待簽章");
                else if (I32(d, r + 16) != 5000) Fail($"{name}: P3 增援等待應為 5000，實為 {I32(d, r + 16)}");

                // P4 撤退期限：6 站中恰 4 站 5000、2 站（settled）600000
                var deadlines = FindRetreatDeadlines(d);
                if (deadlines.Count != 6) Fail($"{name}: P4 撤退期限站數應為 6，實為 {deadlines.Count}");
                else {
                    int acc = deadlines.Count(o => I32(d, o) == 5000);
                    int settled = deadlines.Count(o => I32(d, o) == 600000);
                    if (acc != 4 || settled != 2) Fail($"{name}: P4 應為 4×5000 + 2×600000，實為 {acc}×5000 + {settled}×600000");
                    // 索引 0、4 必須保持 600000（§8 不變式）
                    if (I32(d, deadlines[0]) != 600000 || I32(d, deadlines[4]) != 600000)
                        Fail($"{name}: P4 settled 站(索引0/4)必須為 600000");
                }

                // P5 死亡黨去彈跳 3
                int?[] debounceSig = { 0x5A, 24, 0x42, null, 96, 101, 117, 16, 0x42, 1, 0x5B, 17 };
                int db = BciPattern.FindBciWordPattern(d, debounceSig);

                // P15 safety migration: both settled-party handlers remain on
                // DELETE_PARTY (256); legacy DELETE_TEAM (257) must be absent.
                int safeSettledSites = 0;
                int legacyDeleteTeamSites = 0;
                for (int off = 0; off <= d.Length - 68; off += 4) {
                    if (I32(d, off) != 71 || I32(d, off + 4) != 66 || I32(d, off + 8) != 0 ||
                        I32(d, off + 12) != 117 || I32(d, off + 16) != 16 || I32(d, off + 20) != 66 ||
                        I32(d, off + 28) != 91 || I32(d, off + 36) != 112 ||
                        I32(d, off + 44) != 66 || I32(d, off + 48) != 256 ||
                        I32(d, off + 52) != 90 || I32(d, off + 56) != 14 ||
                        I32(d, off + 60) != 96 || I32(d, off + 64) != 118) continue;
                    int stateLocal = I32(d, off + 32);
                    if (stateLocal != 6 && stateLocal != 7) continue;
                    if (I32(d, off + 24) == 256) safeSettledSites++;
                    if (I32(d, off + 24) == 257) legacyDeleteTeamSites++;
                }
                if (safeSettledSites != 2 || legacyDeleteTeamSites != 0)
                    Fail($"{name}: P15 safe DELETE_PARTY sites should be 2 and legacy DELETE_TEAM sites 0, actual {safeSettledSites}/{legacyDeleteTeamSites}");
                if (db < 0) Fail($"{name}: 找不到 P5 去彈跳簽章");
                else if (I32(d, db + 12) != 3) Fail($"{name}: P5 去彈跳應為 3，實為 {I32(d, db + 12)}");

                // P6 排程迴圈延遲：符合 pushlit/pushlit/pushsym-16 形狀的站點很多（symbol 16
                // 有多處呼叫），無法只靠 opcode 形狀定位迴圈站。改用值特徵獨立判定：
                // 迴圈延遲站的值必為「終極 10000/5000」或「原始大範圍」之一，其餘小值站是無關呼叫。
                // 斷言：恰 6 站持有終極值 10000/5000，且無任何站殘留未加速的原始/legacy 迴圈範圍。
                var origRanges = new HashSet<(int, int)> {
                    (960000, 480000), (360000, 240000), (120000, 60000), (240000, 120000)
                };
                int ultLoops = 0, leftover = 0;
                for (int off = 0; off <= d.Length - 24; off += 4) {
                    if (I32(d, off) != 0x42 || I32(d, off + 8) != 0x42 || I32(d, off + 16) != 0x80 || I32(d, off + 20) != 16) continue;
                    int up = I32(d, off + 4), lo = I32(d, off + 12);
                    if (up == 10000 && lo == 5000) ultLoops++;
                    else if (origRanges.Contains((up, lo)) || (up == 2000 && lo == 1000)) leftover++;
                }
                if (ultLoops != 6) Fail($"{name}: P6 應有恰 6 站持有終極值 10000/5000，實為 {ultLoops}");
                if (leftover != 0) Fail($"{name}: P6 有 {leftover} 站殘留未加速的原始/legacy 迴圈範圍");

                // P7 聚落生成機率：6 個 101
                int?[] spawnSig = { 66, null, 91, 2, 66, null, 91, 2, 66, 0, 91, 3, 90, 0, 91, 3 };
                int sp = BciPattern.FindBciWordPattern(d, spawnSig);
                if (sp < 0) Fail($"{name}: 找不到 P7 生成機率簽章");
                else {
                    foreach (int rel in new[] { 4, 20, 96, 148, 200, 252 })
                        if (I32(d, sp + rel) != 101) Fail($"{name}: P7 生成機率(+{rel})應為 101，實為 {I32(d, sp + rel)}");
                }

                // P8 增援門檻 40，gate 保持 66,0
                int?[] limitSig = { 0x5A, 0, 0x42, null, 96, 98, 0x5B, 11, null, null, 0x42, 0, 0x42, 0, 0x42, 0, 0x42, 0, 0x5A, 6, 102, 117, 32 };
                int lim = BciPattern.FindBciWordPattern(d, limitSig);
                if (lim < 0) Fail($"{name}: 找不到 P8 門檻簽章");
                else {
                    if (I32(d, lim + 12) != 40) Fail($"{name}: P8 增援門檻應為 40，實為 {I32(d, lim + 12)}");
                    if (I32(d, lim + 32) != 66 || I32(d, lim + 36) != 0) Fail($"{name}: P8 gate 應保持 66,0，實為 {I32(d, lim + 32)},{I32(d, lim + 36)}");
                }

                // P9 撤退配額歸零 [66,0]
                int?[] quotaSig = { 81, 57, 90, -3, 90, 14, 164, 81, 56, 90, -3, null, null, 164, 81, 61 };
                int q = BciPattern.FindBciWordPattern(d, quotaSig);
                if (q < 0) Fail($"{name}: 找不到 P9 撤退配額簽章");
                else if (I32(d, q + 44) != 66 || I32(d, q + 48) != 0) Fail($"{name}: P9 撤退配額應為 66,0，實為 {I32(d, q + 44)},{I32(d, q + 48)}");

                // P16: three type-1 settlements leave room for the type-4 military settlement.
                int?[] settlementCapSig = { 66, null, 66, null, 128, 16, 73, -2, 86, 82, 70 };
                int cap = BciPattern.FindBciWordPattern(d, settlementCapSig);
                if (cap < 0) Fail($"{name}: 找不到 P16 村莊上限簽章");
                else if (I32(d, cap + 4) != 3 || I32(d, cap + 12) != 3)
                    Fail($"{name}: P16 type-1 配額應為 3,3，實為 {I32(d, cap + 4)},{I32(d, cap + 12)}");
            }

            // ---- ak_haupthaus.bci ----
            foreach (string path in EndlessAiOrchestrator.ResolvePaths(tempDir, "SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci")) {
                byte[] d = Load(path);
                int?[] p10Sig = { null, null, 81, 11, 81, 10, 81, 98, 128, 81, 73, -4, 86 };
                int h = BciPattern.FindBciWordPattern(d, p10Sig);
                if (h < 0) Fail("ak_haupthaus: 找不到 P10 轉兵批量簽章");
                else if (I32(d, h) != 66 || I32(d, h + 4) != 20) Fail($"P10 轉兵批量應為 66,20，實為 {I32(d, h)},{I32(d, h + 4)}");

                int?[] p11Sig = { 66, null, 66, 25, 66, -25, 128, 34, 73, -2, 86, 32, 82, 14, 81, 14, 82, 15 };
                int cl = BciPattern.FindBciWordPattern(d, p11Sig);
                if (cl < 0) Fail("ak_haupthaus: 找不到 P11 拆村延遲簽章");
                else if (I32(d, cl + 4) != 100) Fail($"P11 拆村延遲應為 100，實為 {I32(d, cl + 4)}");
            }

            // ---- Dorfverteidigung.bci ----
            foreach (string path in EndlessAiOrchestrator.ResolvePaths(tempDir, "SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci")) {
                byte[] d = Load(path);
                int?[] p12Sig = { 66, 0, 66, 1, 66, null, 66, null, 66, 0, 66, 0, 66, null, 66, 1, 90, 8, 128, 157, 73, -9, 86 };
                var sites = BciPattern.FindAllBciWordPatternSites(d, p12Sig);
                if (sites.Count != 4) Fail($"Dorfverteidigung: P12 站數應為 4，實為 {sites.Count}");
                foreach (int s in sites)
                    if (I32(d, s + 20) != 20 || I32(d, s + 28) != 20) Fail($"P12 村防轉兵應為 20,20，實為 {I32(d, s + 20)},{I32(d, s + 28)}");
            }

            // ---- Endlos_*_Siedlung*.sdl ×42 ----
            var sdlPaths = EndlessAiOrchestrator.ResolvePaths(tempDir, "MAPS/ENDL_*/Endlos_*_Siedlung*.sdl");
            if (sdlPaths.Count != 42) Fail($"聚落模板應為 42 個，實為 {sdlPaths.Count}");
            foreach (string path in sdlPaths) {
                byte[] d = Load(path);
                if (!System.Text.Encoding.Latin1.GetString(d).Contains("614,300,372,250,460,288"))
                    Fail($"P13 {Path.GetFileName(path)}: 主營 resv 未含終極值 614,300,372,250,460,288");
            }

            // ---- P14 常駐修復：ak_npc 應為 0、ak_produktion 應為 117（原版，恆還原）----
            foreach (string path in EndlessAiOrchestrator.ResolvePaths(tempDir, "SYSTEM/CLAK/SCRIPT/ak_npc.bci|SYSTEM/CLAK/SCRIPT/ak_produktion.bci")) {
                byte[] d = Load(path);
                string fn = Path.GetFileName(path);
                if (fn.Equals("ak_npc.bci", StringComparison.OrdinalIgnoreCase)) {
                    int?[] npcSig = { 128, 43, 73, -2, 86, 66, null, 96, 99, 117, 476 };
                    int n = BciPattern.FindBciWordPattern(d, npcSig);
                    if (n >= 0 && I32(d, n + 24) != 0) Fail($"P14 ak_npc 應恆為 0，實為 {I32(d, n + 24)}");
                } else {
                    int?[] prodSig = { 128, 69, 73, -2, 86, null, 56, 66, 1, 82, 46 };
                    int p = BciPattern.FindBciWordPattern(d, prodSig);
                    if (p >= 0 && I32(d, p + 20) != 117) Fail($"P14 ak_produktion 應恆為 117，實為 {I32(d, p + 20)}");
                }
            }

            if (failures.Count > 0) {
                Console.WriteLine($"[失敗] golden 值驗證發現 {failures.Count} 項不符：");
                foreach (string f in failures) Console.WriteLine("  - " + f);
                return false;
            }
            return true;
        }

        // P4 撤退期限站點定位（獨立複製，供 golden 驗證用）。
        static List<int> FindRetreatDeadlines(byte[] d) {
            var results = new List<int>();
            int?[] prefix = { 0x51, 61, 0x5A, -3, 0x80, 83, 0x56, 0x42 };
            int prefixBytes = prefix.Length * 4;
            for (int off = 0; off <= d.Length - (prefixBytes + 24); off += 4) {
                bool match = true;
                for (int i = 0; i < prefix.Length; i++) {
                    if (prefix[i].HasValue && BitConverter.ToInt32(d, off + i * 4) != prefix[i]!.Value) { match = false; break; }
                }
                if (!match) continue;
                if (BitConverter.ToInt32(d, off + 36) != 32 || BitConverter.ToInt32(d, off + 40) != 44 || BitConverter.ToInt32(d, off + 44) != 164) continue;
                if (BitConverter.ToInt32(d, off + 48) == 0x42 && BitConverter.ToInt32(d, off + 52) == 34) continue;
                results.Add(off + 32);
            }
            return results;
        }
    }
}
