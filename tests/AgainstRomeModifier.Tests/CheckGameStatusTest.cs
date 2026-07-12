using System;
using Xunit;
using Xunit.Abstractions;
using AgainstRomeModifier;

namespace AgainstRomeModifier.Tests
{
    public class CheckGameStatusTest
    {
        private readonly ITestOutputHelper _output;

        public CheckGameStatusTest(ITestOutputHelper output)
        {
            _output = output;
        }

        [Fact]
        public void VerifyRealGamePatches()
        {
            // 本測試是唯讀診斷輸出（無斷言），僅在明確設定 AR_GAME_PATH 環境變數時執行，
            // 不再硬編碼特定開發機的安裝路徑。
            string? gamePath = Environment.GetEnvironmentVariable("AR_GAME_PATH");
            if (string.IsNullOrWhiteSpace(gamePath) || !System.IO.Directory.Exists(gamePath)) {
                _output.WriteLine("[SKIPPED] 未設定 AR_GAME_PATH 環境變數（或目錄不存在），略過真實安裝目錄診斷輸出。");
                return;
            }
            _output.WriteLine($"正在檢測遊戲安裝目錄：{gamePath}");

            var orchestrator = new EndlessAiOrchestrator();

            foreach (var module in orchestrator.UserModules)
            {
                var state = orchestrator.DetectModule(gamePath, module);
                _output.WriteLine($"模組 {module.Id} ({module.Name}) 狀態：{state}");
                if (module.Id == "M4" || module.Id == "M6")
                {
                    foreach (var patch in module.Patches)
                    {
                        var paths = EndlessAiOrchestrator.ResolvePaths(gamePath, patch.TargetPattern);
                        _output.WriteLine($"  - 補丁 {patch.Id} (Target: {patch.TargetPattern}) 匹配檔案數：{paths.Count}");
                        foreach (var path in paths)
                        {
                            try
                            {
                                byte[] raw = System.IO.File.ReadAllBytes(path);
                                byte[] decomp = GameLZSS.DecompressPfil(raw);
                                // Detailed trace for P8
                                if (patch.Id == "P8")
                                {
                                    int sequenceOffset = BciPattern.FindBciWordPattern(decomp, new int?[] { 0x5A, 0, 0x42, null, 96, 98, 0x5B, 11 });
                                    int countGateOffset = BciPattern.FindBciWordPattern(decomp, new int?[] { 81, 63, 66, 5, 163, 91, 0, 90, 0, 66, null, 96, 99, 117 });
                                    int currentLimit = sequenceOffset >= 0 ? BitConverter.ToInt32(decomp, sequenceOffset + 12) : -1;
                                    int currentCountLimit = countGateOffset >= 0 ? BitConverter.ToInt32(decomp, countGateOffset + 10 * 4) : -1;
                                    int gateOffset = sequenceOffset + 32;
                                    
                                    int[] OriginalGateWords = new int[] { 66, 0, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117 };
                                    int[] BoundedGateWords = new int[] { 90, 11, 117, 252, 90, 11, 117, 236, 90, 11, 117, 220, 112, 224 };
                                    int[] LegacyBoundedGateWords3 = new int[] { 90, 11, 117, 252, 90, 11, 117, 236, 90, 11, 117, 220, 112, 232 };
                                    
                                    bool isOriginalGate = sequenceOffset >= 0 && HasWords(decomp, gateOffset, OriginalGateWords);
                                    bool isBoundedGate = sequenceOffset >= 0 && HasWords(decomp, gateOffset, BoundedGateWords);
                                    bool isLegacyBoundedGate3 = sequenceOffset >= 0 && HasWords(decomp, gateOffset, LegacyBoundedGateWords3);
                                    
                                    _output.WriteLine($"    [Trace P8] seqOffset={sequenceOffset}, countGateOffset={countGateOffset}, currentLimit={currentLimit}, currentCountLimit={currentCountLimit}");
                                    _output.WriteLine($"    [Trace P8] isOriginal={isOriginalGate}, isBounded={isBoundedGate}, isLegacy3={isLegacyBoundedGate3}");
                                }
                                if (patch.Id == "P9")
                                {
                                    int?[] RetreatQuotaSignature = new int?[] { 81, 57, 90, -3, 90, 14, 164, 81, 56, 90, -3, null, null, 164, 81, 61 };
                                    int?[] State48Signature = new int?[] { 81, 48, 90, -3, 66, null, 164 };
                                    
                                    int sigOffset = BciPattern.FindBciWordPattern(decomp, RetreatQuotaSignature);
                                    int stateOffset = BciPattern.FindBciWordPattern(decomp, State48Signature);
                                    
                                    int opcode = sigOffset >= 0 ? BitConverter.ToInt32(decomp, sigOffset + 11 * 4) : -1;
                                    int value = sigOffset >= 0 ? BitConverter.ToInt32(decomp, sigOffset + 11 * 4 + 4) : -1;
                                    int stateVal = stateOffset >= 0 ? BitConverter.ToInt32(decomp, stateOffset + 5 * 4) : -1;
                                    
                                    _output.WriteLine($"    [Trace P9] sigOffset={sigOffset}, stateOffset={stateOffset}, opcode={opcode}, value={value}, stateVal={stateVal}");
                                }
                                var pState = patch.Detect(decomp);
                                _output.WriteLine($"    * 檔案 {System.IO.Path.GetRelativePath(gamePath, path)} 狀態：{pState}");
                            }
                            catch (Exception ex)
                            {
                                _output.WriteLine($"    * 檔案 {System.IO.Path.GetRelativePath(gamePath, path)} 異常：{ex.Message}");
                            }
                        }
                    }
                }
            }
        }

        [Fact]
        public void VerifyOriginalGamePatches()
        {
            string gamePath = System.IO.Path.GetFullPath(System.IO.Path.Combine(AppContext.BaseDirectory, "../../../../../遊戲原始檔案"));
            _output.WriteLine($"正在檢測開發目錄下的遊戲原始檔案：{gamePath}");

            // 遊戲原始檔案為專有資料、不進版控；CI 上目錄不存在時略過本驗證
            // （DetectModule 對找不到檔案的情況回傳 Unknown，不可再拿來斷言 Original）。
            if (!System.IO.Directory.Exists(gamePath)) {
                _output.WriteLine("遊戲原始檔案目錄不存在（CI 環境），略過本機專屬驗證。");
                return;
            }

            var orchestrator = new EndlessAiOrchestrator();

            foreach (var module in orchestrator.UserModules)
            {
                var state = orchestrator.DetectModule(gamePath, module);
                _output.WriteLine($"模組 {module.Id} ({module.Name}) 狀態：{state}");
                Assert.Equal(PatchState.Original, state);
            }

            var r0State = orchestrator.DetectModule(gamePath, orchestrator.R0);
            _output.WriteLine($"模組 R0 (常駐修復) 狀態：{r0State}");
            Assert.Equal(PatchState.Original, r0State);
        }

        private static bool HasWords(byte[] buffer, int offset, int[] words)
        {
            if (offset < 0 || offset + words.Length * 4 > buffer.Length) return false;
            for (int i = 0; i < words.Length; i++)
            {
                if (BitConverter.ToInt32(buffer, offset + i * 4) != words[i]) return false;
            }
            return true;
        }
    }
}
