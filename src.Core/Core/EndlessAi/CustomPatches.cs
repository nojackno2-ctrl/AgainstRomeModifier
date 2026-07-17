using System;
using System.Collections.Generic;
using System.IO;
using System.Text;

namespace AgainstRomeModifier
{
    // ==========================================
    // P4: 撤退期限加速（4/6 站）
    // ==========================================
    public class P4_RetreatDeadlinePatch : IEndlessPatch
    {
        public string Id => "P4";
        public string TargetPattern => "MAPS/ENDL_*/SCRIPT/ak_level.bci";

        private const int OriginalRetreatDeadlineMs = 600000;
        private const int UltimateRespawnDelayMs = 60000;
        private const int RetreatDeadlineSiteCount = 6;

        public PatchState Detect(byte[] decompressed)
        {
            var sites = FindRetreatDeadlineSites(decompressed);
            if (sites.Count != RetreatDeadlineSiteCount)
            {
                return PatchState.Unknown;
            }

            int acceleratedDeadlines = 0;
            int protectedSettledDeadlines = 0;
            int originalDeadlines = 0;

            for (int i = 0; i < sites.Count; i++)
            {
                int val = BitConverter.ToInt32(decompressed, sites[i]);
                if (val == UltimateRespawnDelayMs)
                {
                    if (!IsSettledSite(i)) acceleratedDeadlines++;
                }
                else if (val == OriginalRetreatDeadlineMs)
                {
                    originalDeadlines++;
                    if (IsSettledSite(i)) protectedSettledDeadlines++;
                }
                else
                {
                    return PatchState.Unknown;
                }
            }

            if (originalDeadlines == RetreatDeadlineSiteCount)
            {
                return PatchState.Original;
            }
            if (acceleratedDeadlines == 4 && protectedSettledDeadlines == 2)
            {
                return PatchState.Ultimate;
            }

            // Other combinations (e.g., all 6 accelerated) are legacy/need migration
            return PatchState.Legacy;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            var sites = FindRetreatDeadlineSites(decompressed);
            if (sites.Count != RetreatDeadlineSiteCount)
            {
                throw new InvalidOperationException("P4 signature count mismatch.");
            }

            bool changed = false;
            for (int i = 0; i < sites.Count; i++)
            {
                int target = (!enabled || IsSettledSite(i)) ? OriginalRetreatDeadlineMs : UltimateRespawnDelayMs;
                int offset = sites[i];
                int current = BitConverter.ToInt32(decompressed, offset);
                if (current != target)
                {
                    BciPattern.WriteBciInt32(decompressed, offset, current, target, "P4 retreat deadline");
                    changed = true;
                }
            }
            return changed;
        }

        private static bool IsSettledSite(int index) => index == 0 || index == 4;

        private static List<int> FindRetreatDeadlineSites(byte[] decompressedBci)
        {
            var results = new List<int>();
            int?[] prefix = new int?[] { 0x51, 61, 0x5A, -3, 0x80, 83, 0x56, 0x42 };
            int prefixBytes = prefix.Length * 4;

            for (int offset = 0; offset <= decompressedBci.Length - (prefixBytes + 24); offset += 4)
            {
                bool match = true;
                for (int i = 0; i < prefix.Length; i++)
                {
                    int? expected = prefix[i];
                    if (expected.HasValue && BitConverter.ToInt32(decompressedBci, offset + (i * 4)) != expected.Value)
                    {
                        match = false;
                        break;
                    }
                }
                if (!match) continue;

                if (BitConverter.ToInt32(decompressedBci, offset + 36) != 32 ||
                    BitConverter.ToInt32(decompressedBci, offset + 40) != 44 ||
                    BitConverter.ToInt32(decompressedBci, offset + 44) != 164)
                {
                    continue;
                }

                if (BitConverter.ToInt32(decompressedBci, offset + 48) == 0x42 &&
                    BitConverter.ToInt32(decompressedBci, offset + 52) == 34)
                {
                    continue;
                }
                results.Add(offset + 32);
            }
            return results;
        }
    }

    // ==========================================
    // P6: 排程迴圈延遲（6 站）
    // ==========================================
    public class P6_LoopDelayPatch : IEndlessPatch
    {
        public string Id => "P6";
        public string TargetPattern => "MAPS/ENDL_*/SCRIPT/ak_level.bci";

        private static readonly (int OriginalUpperMs, int OriginalLowerMs)[] LoopDelayRanges = new (int, int)[] {
            (960000, 480000),
            (960000, 480000),
            (360000, 240000),
            (120000, 60000),
            (120000, 60000),
            (240000, 120000)
        };

        private const int UltimateLoopDelayUpperMs = 30000;
        private const int UltimateLoopDelayLowerMs = 30000;
        private const int LegacyLoopDelayUpperMs = 2000;
        private const int LegacyLoopDelayLowerMs = 1000;
        private const int NonSchedulerLoopCount = 4;

        // Saves serialize absolute script deadlines separately from the script clock. In late
        // games the restored v16 deadline can be millions of milliseconds ahead of s_getTime(),
        // preventing the outer settlement scheduler from reaching the accelerated branch at all.
        // The normal Ultimate deadline is 30 seconds, so only a gap greater than 60 seconds is
        // treated as a load-time rollback. This preserves ordinary throttling and the bounded
        // active-party/job-slot protections.
        internal const int DeadlineRollbackThresholdMs = 60000;

        // Code-stream range 0x1bc44..0x1bd48 in all five stock ENDL ak_level scripts.
        // The four delay literals are wildcarded when locating the original/legacy shape.
        private static readonly int[] OriginalSchedulerBlock = {
            66, 0, 128, 72, 86, 102, 117, 48,
            128, 83, 86, 81, 16, 96, 101, 117,
            12, 71, 66, 1, 117, 172, 66, 0,
            91, 19, 81, 71, 102, 117, 52, 66,
            120000, 66, 60000, 128, 16, 73, -2, 86,
            91, 19, 112, 44, 66, 240000, 66, 120000,
            128, 16, 73, -2, 86, 91, 19, 128,
            83, 86, 90, 19, 32, 82, 16, 120,
            -9748
        };

        // Equivalent Ultimate scheduler with a bounded rollback guard:
        //   due = now >= v16 || v16 > now + 60000
        //   if (due) { v16 = now + 30000; spawnSettlement(); }
        // The second outer dispatcher call remains at the original following offset. Branch
        // operands use the VM's verified target rule: opcode + 8 + displacement.
        private static readonly int[] UltimateSchedulerBlock = BuildUltimateSchedulerBlock();
        private static readonly int?[] OriginalSchedulerSignature = BuildOriginalSchedulerSignature();
        private static readonly int?[] UltimateSchedulerSignature = ToPattern(UltimateSchedulerBlock);

        internal static ReadOnlySpan<int> OriginalSchedulerBlockWords => OriginalSchedulerBlock;
        internal static ReadOnlySpan<int> UltimateSchedulerBlockWords => UltimateSchedulerBlock;

        public PatchState Detect(byte[] decompressed)
        {
            if (!TryFindSchedulerBlock(decompressed, out int schedulerOffset, out bool repaired))
                return PatchState.Unknown;

            List<int> loopSites = FindNonSchedulerLoopSites(decompressed, schedulerOffset);
            if (loopSites.Count != NonSchedulerLoopCount) return PatchState.Unknown;

            PatchState loopState = DetectNonSchedulerLoops(decompressed, loopSites);
            PatchState schedulerState = repaired
                ? PatchState.Ultimate
                : DetectOriginalShapeScheduler(decompressed, schedulerOffset);

            if (loopState == PatchState.Original && schedulerState == PatchState.Original)
                return PatchState.Original;
            if (loopState == PatchState.Ultimate && schedulerState == PatchState.Ultimate)
                return PatchState.Ultimate;
            return PatchState.Legacy;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            PatchState state = Detect(decompressed);
            if (state == PatchState.Unknown)
            {
                if (!enabled) return false;
                throw new InvalidOperationException("P6 scheduler bytes do not match a supported original, legacy, or repaired pattern.");
            }
            if ((enabled && state == PatchState.Ultimate) || (!enabled && state == PatchState.Original))
                return false;

            if (!TryFindSchedulerBlock(decompressed, out int schedulerOffset, out bool repaired))
                throw new InvalidOperationException("P6 scheduler block signature count mismatch during Apply.");

            List<int> loopSites = FindNonSchedulerLoopSites(decompressed, schedulerOffset);
            if (loopSites.Count != NonSchedulerLoopCount)
                throw new InvalidOperationException("P6 non-scheduler loop count mismatch during Apply.");

            bool changed = false;
            for (int i = 0; i < loopSites.Count; i++)
            {
                int site = loopSites[i];
                (int originalUpperMs, int originalLowerMs) = LoopDelayRanges[i];
                int targetUpperMs = enabled ? UltimateLoopDelayUpperMs : originalUpperMs;
                int targetLowerMs = enabled ? UltimateLoopDelayLowerMs : originalLowerMs;
                changed |= WriteIfDifferent(ref decompressed, site + 4, targetUpperMs, "P6 scheduler upper delay");
                changed |= WriteIfDifferent(ref decompressed, site + 12, targetLowerMs, "P6 scheduler lower delay");
            }

            int[] targetBlock = enabled ? UltimateSchedulerBlock : OriginalSchedulerBlock;
            if ((enabled && !repaired) || (!enabled && repaired) || state == PatchState.Legacy)
                changed |= ReplaceSchedulerBlock(ref decompressed, schedulerOffset, targetBlock);

            return changed;
        }

        private static PatchState DetectNonSchedulerLoops(byte[] decompressed, IReadOnlyList<int> sites)
        {
            bool allOriginal = true;
            bool allUltimate = true;
            for (int i = 0; i < sites.Count; i++)
            {
                int upper = BitConverter.ToInt32(decompressed, sites[i] + 4);
                int lower = BitConverter.ToInt32(decompressed, sites[i] + 12);
                (int originalUpper, int originalLower) = LoopDelayRanges[i];
                bool original = upper == originalUpper && lower == originalLower;
                bool ultimate = upper == UltimateLoopDelayUpperMs && lower == UltimateLoopDelayLowerMs;
                bool legacy = upper == LegacyLoopDelayUpperMs && lower == LegacyLoopDelayLowerMs;
                if (!original && !ultimate && !legacy) return PatchState.Unknown;
                allOriginal &= original;
                allUltimate &= ultimate;
            }
            if (allOriginal) return PatchState.Original;
            if (allUltimate) return PatchState.Ultimate;
            return PatchState.Legacy;
        }

        private static PatchState DetectOriginalShapeScheduler(byte[] decompressed, int offset)
        {
            int upperA = BitConverter.ToInt32(decompressed, offset + (32 * 4));
            int lowerA = BitConverter.ToInt32(decompressed, offset + (34 * 4));
            int upperB = BitConverter.ToInt32(decompressed, offset + (45 * 4));
            int lowerB = BitConverter.ToInt32(decompressed, offset + (47 * 4));
            if (upperA == LoopDelayRanges[4].OriginalUpperMs && lowerA == LoopDelayRanges[4].OriginalLowerMs &&
                upperB == LoopDelayRanges[5].OriginalUpperMs && lowerB == LoopDelayRanges[5].OriginalLowerMs)
                return PatchState.Original;
            return PatchState.Legacy;
        }

        private static List<int> FindNonSchedulerLoopSites(byte[] decompressed, int schedulerOffset)
        {
            var sites = new List<int>();
            int schedulerEnd = schedulerOffset + (OriginalSchedulerBlock.Length * 4);
            for (int offset = 0; offset <= decompressed.Length - 24 && sites.Count < NonSchedulerLoopCount; offset += 4)
            {
                if (offset >= schedulerOffset && offset < schedulerEnd) continue;
                if (BitConverter.ToInt32(decompressed, offset) != 0x42 ||
                    BitConverter.ToInt32(decompressed, offset + 8) != 0x42 ||
                    BitConverter.ToInt32(decompressed, offset + 16) != 0x80 ||
                    BitConverter.ToInt32(decompressed, offset + 20) != 16)
                    continue;

                int index = sites.Count;
                int upper = BitConverter.ToInt32(decompressed, offset + 4);
                int lower = BitConverter.ToInt32(decompressed, offset + 12);
                (int originalUpper, int originalLower) = LoopDelayRanges[index];
                bool supported = (upper == originalUpper && lower == originalLower) ||
                                 (upper == UltimateLoopDelayUpperMs && lower == UltimateLoopDelayLowerMs) ||
                                 (upper == LegacyLoopDelayUpperMs && lower == LegacyLoopDelayLowerMs);
                if (supported) sites.Add(offset);
            }
            return sites;
        }

        private static bool TryFindSchedulerBlock(byte[] decompressed, out int offset, out bool repaired)
        {
            List<int> originalSites = BciPattern.FindAllBciWordPatternSites(decompressed, OriginalSchedulerSignature);
            List<int> repairedSites = BciPattern.FindAllBciWordPatternSites(decompressed, UltimateSchedulerSignature);
            if (originalSites.Count + repairedSites.Count != 1)
            {
                offset = -1;
                repaired = false;
                return false;
            }
            repaired = repairedSites.Count == 1;
            offset = repaired ? repairedSites[0] : originalSites[0];
            return true;
        }

        private static bool ReplaceSchedulerBlock(ref byte[] decompressed, int offset, IReadOnlyList<int> target)
        {
            bool changed = false;
            for (int i = 0; i < target.Count; i++)
                changed |= WriteIfDifferent(ref decompressed, offset + (i * 4), target[i], "P6 save/load deadline repair");
            return changed;
        }

        private static bool WriteIfDifferent(ref byte[] decompressed, int offset, int target, string name)
        {
            int current = BitConverter.ToInt32(decompressed, offset);
            if (current == target) return false;
            BciPattern.WriteBciInt32(decompressed, offset, current, target, name);
            return true;
        }

        private static int?[] BuildOriginalSchedulerSignature()
        {
            int?[] signature = ToPattern(OriginalSchedulerBlock);
            foreach (int index in new[] { 32, 34, 45, 47 }) signature[index] = null;
            return signature;
        }

        private static int?[] ToPattern(IReadOnlyList<int> words)
        {
            var result = new int?[words.Count];
            for (int i = 0; i < words.Count; i++) result[i] = words[i];
            return result;
        }

        private static int[] BuildUltimateSchedulerBlock()
        {
            var words = new List<int> {
                // Single-player guard; skip the settlement spawner in net games.
                66, 0, 128, 72, 86, 102, 117, 228,
                // Normal due check: s_getTime() >= v16 -> reset/spawn.
                128, 83, 86, 81, 16, 96, 101, 118, 56,
                // Rollback check: v16 > s_getTime() + 60000 -> reset/spawn.
                81, 16, 128, 83, 86, 66, DeadlineRollbackThresholdMs, 32, 96, 100, 118, 8,
                // Otherwise preserve the normal pending deadline and skip this spawner.
                112, 136,
                // Reset to the normal accelerated interval.
                128, 83, 86, 66, UltimateLoopDelayUpperMs, 32, 82, 16
            };

            // Preserve the original call offset so downstream internal-call displacements and
            // the second dispatcher call remain byte-for-byte stable.
            while (words.Count < 63)
            {
                words.Add(112);
                words.Add(0);
            }
            words.Add(120);
            words.Add(-9748);
            if (words.Count != OriginalSchedulerBlock.Length)
                throw new InvalidOperationException("P6 repaired scheduler block length mismatch.");
            return words.ToArray();
        }
    }

    // ==========================================
    // P7: 聚落生成機率
    // ==========================================
    public class P7_SpawnProbabilitiesPatch : IEndlessPatch
    {
        public string Id => "P7";
        public string TargetPattern => "MAPS/ENDL_*/SCRIPT/ak_level.bci";

        private static readonly int?[] SpawnerPattern = new int?[] {
            66, null, 91, 2, 66, null, 91, 2, 66, 0, 91, 3, 90, 0, 91, 3
        };

        public PatchState Detect(byte[] decompressed)
        {
            int offset = BciPattern.FindBciWordPattern(decompressed, SpawnerPattern);
            if (offset < 0) return PatchState.Unknown;

            int d1 = BitConverter.ToInt32(decompressed, offset + 4);
            int d2 = BitConverter.ToInt32(decompressed, offset + 20);
            int c0 = BitConverter.ToInt32(decompressed, offset + 96);
            int c1 = BitConverter.ToInt32(decompressed, offset + 148);
            int c2 = BitConverter.ToInt32(decompressed, offset + 200);
            int c3 = BitConverter.ToInt32(decompressed, offset + 252);

            bool isOriginal = d1 == 0 && d2 == 0 && c0 == 80 && c1 == 60 && c2 == 40 && c3 == 20;
            bool isUltimate = d1 == 101 && d2 == 101 && c0 == 101 && c1 == 101 && c2 == 101 && c3 == 101;

            if (isOriginal) return PatchState.Original;
            if (isUltimate) return PatchState.Ultimate;
            // Signature found but values are non-standard (e.g., partial or experimental patch);
            // treat as Legacy so Apply can safely overwrite to the correct values.
            return PatchState.Legacy;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            int offset = BciPattern.FindBciWordPattern(decompressed, SpawnerPattern);
            if (offset < 0)
            {
                if (!enabled) return false;
                throw new InvalidOperationException("P7 signature not found.");
            }

            int targetDefault = enabled ? 101 : 0;
            int targetCase0 = enabled ? 101 : 80;
            int targetCase1 = enabled ? 101 : 60;
            int targetCase2 = enabled ? 101 : 40;
            int targetCase3 = enabled ? 101 : 20;

            bool changed = false;
            changed |= WriteIfDifferent(decompressed, offset + 4, targetDefault);
            changed |= WriteIfDifferent(decompressed, offset + 20, targetDefault);
            changed |= WriteIfDifferent(decompressed, offset + 96, targetCase0);
            changed |= WriteIfDifferent(decompressed, offset + 148, targetCase1);
            changed |= WriteIfDifferent(decompressed, offset + 200, targetCase2);
            changed |= WriteIfDifferent(decompressed, offset + 252, targetCase3);

            return changed;
        }

        private static bool WriteIfDifferent(byte[] buffer, int offset, int val)
        {
            int current = BitConverter.ToInt32(buffer, offset);
            if (current != val)
            {
                BciPattern.WriteBciInt32(buffer, offset, current, val, "P7 spawner literal");
                return true;
            }
            return false;
        }
    }

    // ==========================================
    // P8: 增援單位數門檻
    // ==========================================
    public class P8_ReinforcementUnitThresholdPatch : IEndlessPatch
    {
        public string Id => "P8";
        public string TargetPattern => "MAPS/ENDL_*/SCRIPT/ak_level.bci";

        private const int OriginalUnitThreshold = 4;
        private const int LegacyLowUnitThreshold = 8;
        private const int LegacyHighUnitThreshold = 40;
        private const int UltimateUnitThreshold = 30;

        // Original gate: 5× pushlit 0 (66,0) + pushsym 6 (90,6) + cmp (102) + jmp (117) + done (32)
        // Total 15 words at offset +32 from sequence start.
        // We keep these as the canonical "original" values to restore to.
        private static readonly int[] OriginalGateWords = {
            66, 0, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32
        };

        // Exact legacy gates written by earlier experimental builds. Do not loosen
        // these signatures: unknown BCI control flow must never be rewritten.
        private static readonly int[] LegacyUnboundedGateWords = {
            112, 272, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32
        };

        private static readonly int[] LegacyBoundedGateWords = {
            90, 11, 117, 252, 90, 11, 117, 236, 90, 11, 117, 220, 112, 224, 32
        };

        private static readonly int[] LegacyBoundedGateWordsV3 = {
            90, 11, 117, 252, 90, 11, 117, 236, 90, 11, 117, 220, 112, 232, 32
        };

        public PatchState Detect(byte[] decompressed)
        {
            int sequenceOffset = FindReinforcementThresholdSequenceOffset(decompressed);
            if (sequenceOffset < 0) return PatchState.Unknown;

            int currentLimit = BitConverter.ToInt32(decompressed, sequenceOffset + 12);

            int gateStart = sequenceOffset + 32;
            bool isOriginalGate = IsOriginalGate(decompressed, gateStart);

            if (isOriginalGate && currentLimit == OriginalUnitThreshold)
            {
                return PatchState.Original;
            }
            if (isOriginalGate && currentLimit == UltimateUnitThreshold)
            {
                return PatchState.Ultimate;
            }
            if (isOriginalGate && IsLegacyUnitThreshold(currentLimit))
            {
                return PatchState.Legacy;
            }

            bool isKnownLegacyGate =
                HasGateWords(decompressed, gateStart, LegacyUnboundedGateWords) ||
                HasGateWords(decompressed, gateStart, LegacyBoundedGateWords) ||
                HasGateWords(decompressed, gateStart, LegacyBoundedGateWordsV3);
            if (isKnownLegacyGate && IsRecognizedUnitThreshold(currentLimit))
            {
                return PatchState.Legacy;
            }

            return PatchState.Unknown;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            PatchState state = Detect(decompressed);
            if (state == PatchState.Unknown)
            {
                if (!enabled) return false;
                throw new InvalidOperationException("P8 reinforcement unit threshold bytes do not match a supported original, legacy, or current pattern.");
            }
            if ((enabled && state == PatchState.Ultimate) || (!enabled && state == PatchState.Original))
                return false;

            int sequenceOffset = FindReinforcementThresholdSequenceOffset(decompressed);
            bool changed = false;
            int targetLimit = enabled ? UltimateUnitThreshold : OriginalUnitThreshold;
            int limitOffset = sequenceOffset + 12;

            int currentLimit = BitConverter.ToInt32(decompressed, limitOffset);
            if (currentLimit != targetLimit)
            {
                BciPattern.WriteBciInt32(decompressed, limitOffset, currentLimit, targetLimit, "P8 reinforcement unit threshold");
                changed = true;
            }

            // Always restore the entire gate to original values, handling any legacy variant
            int gateOffset = sequenceOffset + 32;
            for (int i = 0; i < OriginalGateWords.Length; i++)
            {
                int byteOffset = gateOffset + i * 4;
                int current = BitConverter.ToInt32(decompressed, byteOffset);
                if (current != OriginalGateWords[i])
                {
                    BciPattern.WriteBciInt32(decompressed, byteOffset, current, OriginalGateWords[i], $"P8 gate word {i}");
                    changed = true;
                }
            }

            return changed;
        }

        /// <summary>
        /// Checks whether the gate region matches the original 5×pushlit_0 pattern.
        /// </summary>
        private static bool IsOriginalGate(byte[] data, int gateOffset)
            => HasGateWords(data, gateOffset, OriginalGateWords);

        private static bool HasGateWords(byte[] data, int gateOffset, int[] expectedWords)
        {
            if (gateOffset + expectedWords.Length * 4 > data.Length) return false;
            for (int i = 0; i < expectedWords.Length; i++)
            {
                if (BitConverter.ToInt32(data, gateOffset + i * 4) != expectedWords[i])
                    return false;
            }
            return true;
        }

        private static bool IsLegacyUnitThreshold(int value) =>
            value == LegacyLowUnitThreshold || value == LegacyHighUnitThreshold;

        private static bool IsRecognizedUnitThreshold(int value) =>
            value == OriginalUnitThreshold ||
            value == LegacyLowUnitThreshold ||
            value == UltimateUnitThreshold ||
            value == LegacyHighUnitThreshold;

        private static int FindReinforcementThresholdSequenceOffset(byte[] decompressedBci)
        {
            // The prefix identifies the P8 block; Detect then validates the complete
            // 15-word gate against exact original/current/legacy signatures.
            int?[] pattern = new int?[] {
                0x5A, 0, 0x42, null, 96, 98, 0x5B, 11,
                null, null, null, null, null, null, null,
                null, null, null, null, null, null, null, null
            };
            return BciPattern.FindBciWordPattern(decompressedBci, pattern);
        }
    }

    // ==========================================
    // P9: 撤退配額歸零
    // ==========================================
    public class P9_RetreatQuotaPatch : IEndlessPatch
    {
        public string Id => "P9";
        public string TargetPattern => "MAPS/ENDL_*/SCRIPT/ak_level.bci";

        // 用通用 signature 匹配所有 10 處 v56 寫入點。
        // 注意：init 站點（含 site 8，0x16A44）寫入的 v56 是「士兵生成預算」，
        // 會被生成迴圈（內部函式 0xA964，讀取點 0xA9CC）消耗；只有 site 9
        // （0x17880，撤退前的捐贈剩餘寫入）才是真正的撤退配額。因此本補丁
        // 只動 site 9；site 8 一律維持原版 [90,6]，否則 type-5 增援不會生成
        // 任何士兵（實測：只出現村民）。
        private static readonly int?[] RetreatQuotaSignature = new int?[] {
            81, 56, 90, -3, null, null, 164
        };

        private const int RetreatQuotaOriginalOpcode = 90;
        private const int RetreatQuotaOriginalValue = 15;
        private const int SpawnBudgetOriginalOpcode = 90;
        private const int SpawnBudgetOriginalValue = 6;
        private const int RetreatQuotaPatchedOpcode = 66;
        private const int RetreatQuotaPatchedValue = 0;
        private const int RetreatQuotaOpcodeWordIndex = 4;
        private const int ExpectedQuotaSitesCount = 10;

        // 第三控制點：狀態 49 捐贈走訪的單位型別過濾（0x1825C 附近，全檔唯一的
        // s_getUnitType 呼叫）。原版 `if (s_getUnitType(obj) == 1)` 只讓型別 1
        // （單人單位：平民、駄馬、首領）進入配額/捐贈分支，士兵小隊（squad，
        // 型別 != 1）一律留在撤退陣列走回地圖出口——這就是「配額歸零後士兵
        // 仍撤退」的原因。
        //
        // 2026-07-17 修正：舊版 Ultimate 把 jz 位移 92 改 0（吃掉條件），所有
        // 單位都被捐贈留村——結果 type-5 增援隨隊的駄馬/平民也留下，每波堆積
        // （ESAVE_001 實測羅馬營地駄馬爆量）。新版改為「反轉」條件：jz(117)
        // → jnz(118)、位移保留 92。型別 1（駄馬/平民/首領）跳過捐贈分支、
        // 照原版走撤退離場；士兵小隊落入配額分支，配額 0 → 全部 s_setObjMark
        // 捐給 type-4 隊伍留村。士兵留下、駄馬撤退，兩者兼得（代價：原版
        // 會捐 2~3 個平民給村莊的行為不再發生，可接受）。
        // 簽章的 jz/jnz opcode 字（index 9）與位移字（index 10）都是萬用碼，
        // 否則已套用檔（118）會比對不到。
        private static readonly int?[] DonationTypeFilterSignature = new int?[] {
            128, 214, 73, -2, 86, 66, 1, 96, 102, null, null
        };
        private const int DonationTypeFilterJzOpcodeWordIndex = 9;
        private const int DonationTypeFilterJzOperandWordIndex = 10;
        private const int DonationTypeFilterOriginalOpcode = 117;  // jz
        private const int DonationTypeFilterOriginalOperand = 92;
        private const int DonationTypeFilterPatchedOpcode = 118;   // jnz（條件反轉）
        private const int DonationTypeFilterPatchedOperand = 92;
        // （2026-07-08 出貨的舊版 Ultimate 為 jz 位移 92→0：全部捐贈、駄馬堆積；
        //  Detect 時落入 Legacy，Apply 自動遷移。）

        public PatchState Detect(byte[] decompressed)
        {
            var sites = BciPattern.FindAllBciWordPatternSites(decompressed, RetreatQuotaSignature);
            if (sites.Count == 0) return PatchState.Unknown;

            if (sites.Count != ExpectedQuotaSitesCount)
            {
                return PatchState.Legacy;
            }

            // Site 8 (type-5 init: soldier spawn budget) must ALWAYS be vanilla [90,6].
            int sigOffset8 = sites[8];
            int opcodeOffset8 = sigOffset8 + RetreatQuotaOpcodeWordIndex * 4;
            int opcode8 = BitConverter.ToInt32(decompressed, opcodeOffset8);
            int value8 = BitConverter.ToInt32(decompressed, opcodeOffset8 + 4);
            bool site8Vanilla = opcode8 == SpawnBudgetOriginalOpcode && value8 == SpawnBudgetOriginalValue;

            // Site 9 (type-5 pre-retreat donation remainder: the real retreat quota).
            int sigOffset9 = sites[9];
            int opcodeOffset9 = sigOffset9 + RetreatQuotaOpcodeWordIndex * 4;
            int opcode9 = BitConverter.ToInt32(decompressed, opcodeOffset9);
            int value9 = BitConverter.ToInt32(decompressed, opcodeOffset9 + 4);

            // 第三控制點：狀態 49 的單位型別過濾 jz/jnz opcode 與位移。
            int filterOffset = BciPattern.FindBciWordPattern(decompressed, DonationTypeFilterSignature);
            if (filterOffset < 0) return PatchState.Unknown;
            int jzOpcode = BitConverter.ToInt32(decompressed, filterOffset + DonationTypeFilterJzOpcodeWordIndex * 4);
            int jzOperand = BitConverter.ToInt32(decompressed, filterOffset + DonationTypeFilterJzOperandWordIndex * 4);

            // 舊版 P9 曾把 site 8 也改成 [66,0]（導致增援只有村民）——判為 Legacy，
            // Apply 時會自動把 site 8 遷回原版。
            if (!site8Vanilla) return PatchState.Legacy;

            bool site9Ultimate = opcode9 == RetreatQuotaPatchedOpcode && value9 == RetreatQuotaPatchedValue;
            bool site9Original = opcode9 == RetreatQuotaOriginalOpcode && value9 == RetreatQuotaOriginalValue;

            if (site9Ultimate && jzOpcode == DonationTypeFilterPatchedOpcode && jzOperand == DonationTypeFilterPatchedOperand)
                return PatchState.Ultimate;
            if (site9Original && jzOpcode == DonationTypeFilterOriginalOpcode && jzOperand == DonationTypeFilterOriginalOperand)
                return PatchState.Original;

            // Legacy 涵蓋兩種出貨過的中間狀態：
            // 1. 2026-07-08 前期：site9 已歸零但型別過濾仍為原版（士兵小隊照樣撤退）。
            // 2. 2026-07-08 後期：jz 位移 92→0（全部捐贈，駄馬/平民堆積在羅馬營地）。
            // Apply 時自動遷移到 jnz 反轉版。
            return PatchState.Legacy;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            var sites = BciPattern.FindAllBciWordPatternSites(decompressed, RetreatQuotaSignature);
            if (sites.Count == 0)
            {
                if (!enabled) return false;
                throw new InvalidOperationException("P9 retreat quota signature not found.");
            }

            bool changed = false;

            if (sites.Count == ExpectedQuotaSitesCount)
            {
                // Site 8: always restore/keep vanilla spawn budget [90,6]
                // (also migrates the legacy both-sites-zeroed state).
                {
                    int sigOffset = sites[8];
                    int opcodeOffset = sigOffset + RetreatQuotaOpcodeWordIndex * 4;
                    int currentOpcode = BitConverter.ToInt32(decompressed, opcodeOffset);
                    int currentValue = BitConverter.ToInt32(decompressed, opcodeOffset + 4);

                    if (currentOpcode != SpawnBudgetOriginalOpcode || currentValue != SpawnBudgetOriginalValue)
                    {
                        BciPattern.WriteBciInt32(decompressed, opcodeOffset, currentOpcode, SpawnBudgetOriginalOpcode, "P9 spawn budget opcode at site 8 (keep vanilla)");
                        BciPattern.WriteBciInt32(decompressed, opcodeOffset + 4, currentValue, SpawnBudgetOriginalValue, "P9 spawn budget value at site 8 (keep vanilla)");
                        changed = true;
                    }
                }

                // Site 9: the real retreat quota — zero it so all units are donated.
                {
                    int sigOffset = sites[9];
                    int opcodeOffset = sigOffset + RetreatQuotaOpcodeWordIndex * 4;
                    int currentOpcode = BitConverter.ToInt32(decompressed, opcodeOffset);
                    int currentValue = BitConverter.ToInt32(decompressed, opcodeOffset + 4);

                    int targetOpcode = enabled ? RetreatQuotaPatchedOpcode : RetreatQuotaOriginalOpcode;
                    int targetValue = enabled ? RetreatQuotaPatchedValue : RetreatQuotaOriginalValue;

                    if (currentOpcode != targetOpcode || currentValue != targetValue)
                    {
                        BciPattern.WriteBciInt32(decompressed, opcodeOffset, currentOpcode, targetOpcode, "P9 retreat quota opcode at site 9");
                        BciPattern.WriteBciInt32(decompressed, opcodeOffset + 4, currentValue, targetValue, "P9 retreat quota value at site 9");
                        changed = true;
                    }
                }
            }

            // State-49 donation type filter, INVERTED (jz->jnz, offset kept at 92):
            // soldier squads (unit type != 1) fall into the quota branch and get
            // donated (quota 0 => all stay in the village); single units (type 1:
            // pack horses / civilians / leader) take the jump and follow the
            // vanilla retreat path off the map. This keeps reinforcements while
            // preventing pack-horse pileup (legacy operand-0 variant donated
            // EVERYTHING, so pack horses accumulated every wave).
            {
                int filterOffset = BciPattern.FindBciWordPattern(decompressed, DonationTypeFilterSignature);
                if (filterOffset < 0)
                {
                    if (enabled) throw new InvalidOperationException("P9 donation type filter signature not found.");
                }
                else
                {
                    int opcodeOffset = filterOffset + DonationTypeFilterJzOpcodeWordIndex * 4;
                    int operandOffset = filterOffset + DonationTypeFilterJzOperandWordIndex * 4;
                    int currentOpcode = BitConverter.ToInt32(decompressed, opcodeOffset);
                    int currentOperand = BitConverter.ToInt32(decompressed, operandOffset);
                    int targetOpcode = enabled ? DonationTypeFilterPatchedOpcode : DonationTypeFilterOriginalOpcode;
                    int targetOperand = enabled ? DonationTypeFilterPatchedOperand : DonationTypeFilterOriginalOperand;

                    if (currentOpcode != targetOpcode)
                    {
                        BciPattern.WriteBciInt32(decompressed, opcodeOffset, currentOpcode, targetOpcode, "P9 donation type filter jump opcode");
                        changed = true;
                    }
                    if (currentOperand != targetOperand)
                    {
                        BciPattern.WriteBciInt32(decompressed, operandOffset, currentOperand, targetOperand, "P9 donation type filter jump operand");
                        changed = true;
                    }
                }
            }

            return changed;
        }
    }

    // ==========================================
    // P15: restore safe settled-party terminal cleanup
    // ==========================================
    // A previous build changed these transitions to DELETE_TEAM. That can
    // delete the recipient while ak_haupthaus.bci is waiting for its per-object
    // cleanup acknowledgement, leaving the cleanup protocol stuck. Keep the
    // engine-proven DELETE_PARTY path and recognize DELETE_TEAM as legacy so
    // existing installations are migrated safely.
    public class P15_SettledPartyDeleteTeamPatch : IEndlessPatch
    {
        public string Id => "P15";
        public string TargetPattern => "MAPS/ENDL_*/SCRIPT/ak_level.bci";

        private const int DeletePartyState = 256;
        private const int DeleteTeamState = 257;
        private const int ExpectedSiteCount = 2;

        private static readonly int?[] TerminalStatePattern = new int?[] {
            71, 66, 0, 117, 16, 66, null, 91, null, 112, null,
            66, DeletePartyState, 90, 14, 96, 118
        };

        public PatchState Detect(byte[] decompressed)
        {
            List<int> sites = FindTerminalStateSites(decompressed);
            if (sites.Count != ExpectedSiteCount) return PatchState.Unknown;

            int original = 0;
            int legacy = 0;
            foreach (int site in sites)
            {
                int value = BitConverter.ToInt32(decompressed, site);
                if (value == DeletePartyState) original++;
                else if (value == DeleteTeamState) legacy++;
                else return PatchState.Unknown;
            }

            if (original == ExpectedSiteCount) return PatchState.Original;
            if (legacy == ExpectedSiteCount) return PatchState.Legacy;
            return PatchState.Legacy;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            List<int> sites = FindTerminalStateSites(decompressed);
            if (sites.Count != ExpectedSiteCount)
            {
                throw new InvalidOperationException("P15 settled terminal-state signature count mismatch.");
            }

            // This is a mandatory safety repair. Both enable and restore paths
            // converge on the original DELETE_PARTY state.
            int target = DeletePartyState;
            bool changed = false;
            foreach (int site in sites)
            {
                int current = BitConverter.ToInt32(decompressed, site);
                if (current == target) continue;
                BciPattern.WriteBciInt32(decompressed, site, current, target, "P15 settled terminal state");
                changed = true;
            }
            return changed;
        }

        private static List<int> FindTerminalStateSites(byte[] decompressed)
        {
            var sites = new List<int>();
            int patternBytes = TerminalStatePattern.Length * 4;
            for (int offset = 0; offset <= decompressed.Length - patternBytes; offset += 4)
            {
                bool matches = true;
                for (int i = 0; i < TerminalStatePattern.Length; i++)
                {
                    int? expected = TerminalStatePattern[i];
                    if (expected.HasValue &&
                        BitConverter.ToInt32(decompressed, offset + i * 4) != expected.Value)
                    {
                        matches = false;
                        break;
                    }
                }
                if (!matches) continue;

                // The same compiler shape also appears in one transient-party
                // handler, which stores through local 4.  Settled handlers use
                // locals 7 and 6 respectively.
                int stateLocal = BitConverter.ToInt32(decompressed, offset + 8 * 4);
                if (stateLocal != 6 && stateLocal != 7) continue;

                int terminalStateOffset = offset + 6 * 4;
                int value = BitConverter.ToInt32(decompressed, terminalStateOffset);
                if (value == DeletePartyState || value == DeleteTeamState)
                {
                    sites.Add(terminalStateOffset);
                }
            }
            return sites;
        }
    }

    // ==========================================
    // P13: 聚落模板開局資源
    // ==========================================
    public class P13_SettlementTemplatePatch : IEndlessPatch
    {
        public string Id => "P13";
        public string TargetPattern => "MAPS/ENDL_*/Endlos_*_Siedlung*.sdl";

        private const string OriginalResv = "0,0,0,0,0,0";
        private const string UltimateResv = "614,300,372,250,460,288";

        public PatchState Detect(byte[] decompressed)
        {
            string content = Encoding.Latin1.GetString(decompressed);
            string[] lines = content.Split('\n');

            bool inHaupthausSection = false;
            bool foundHaupthausResv = false;
            string? detectedResv = null;

            foreach (string line in lines)
            {
                string trimmed = line.TrimEnd('\r').Trim();
                if (trimmed.StartsWith("["))
                {
                    inHaupthausSection = false;
                }
                else if (trimmed.StartsWith("namedef") && trimmed.Contains("_Haupt"))
                {
                    inHaupthausSection = true;
                }
                else if (inHaupthausSection && trimmed.StartsWith("resv"))
                {
                    inHaupthausSection = false;
                    int eq = line.IndexOf('=');
                    if (eq < 0) return PatchState.Unknown;
                    string resv = line.Substring(eq + 1).TrimEnd('\r').Trim();

                    if (detectedResv != null && detectedResv != resv)
                    {
                        return PatchState.Unknown;
                    }
                    detectedResv = resv;
                    foundHaupthausResv = true;
                }
            }

            if (!foundHaupthausResv || detectedResv == null)
            {
                return PatchState.Unknown;
            }

            if (detectedResv == OriginalResv) return PatchState.Original;
            if (detectedResv == UltimateResv) return PatchState.Ultimate;
            return PatchState.Unknown;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            string content = Encoding.Latin1.GetString(decompressed);
            string[] lines = content.Split('\n');

            bool inHaupthausSection = false;
            bool foundHaupthausResv = false;
            bool changed = false;
            string targetResv = enabled ? UltimateResv : OriginalResv;

            for (int i = 0; i < lines.Length; i++)
            {
                string trimmed = lines[i].TrimEnd('\r').Trim();
                if (trimmed.StartsWith("["))
                {
                    inHaupthausSection = false;
                }
                else if (trimmed.StartsWith("namedef") && trimmed.Contains("_Haupt"))
                {
                    inHaupthausSection = true;
                }
                else if (inHaupthausSection && trimmed.StartsWith("resv"))
                {
                    inHaupthausSection = false;
                    int eq = lines[i].IndexOf('=');
                    if (eq < 0)
                    {
                        throw new InvalidDataException("Invalid SDL format on resv line.");
                    }
                    string suffix = lines[i].EndsWith("\r") ? "\r" : "";
                    string currentResv = lines[i].Substring(eq + 1).TrimEnd('\r').Trim();
                    if (currentResv != OriginalResv && currentResv != UltimateResv)
                    {
                        throw new InvalidDataException($"Unknown resv value '{currentResv}' in template.");
                    }
                    foundHaupthausResv = true;
                    if (currentResv != targetResv)
                    {
                        lines[i] = lines[i].Substring(0, eq + 1) + targetResv + suffix;
                        changed = true;
                    }
                }
            }

            if (!foundHaupthausResv)
            {
                throw new InvalidOperationException("P13 main building resv not found in template.");
            }

            if (changed)
            {
                decompressed = Encoding.Latin1.GetBytes(string.Join("\n", lines));
            }

            return changed;
        }
    }

    // ==========================================
    // P14: 強制還原已被否決的修補 (ak_npc / ak_produktion)
    // ==========================================
    public class P14_ForcedRestorePatch : IEndlessPatch
    {
        public string Id => "P14";
        public string TargetPattern => "SYSTEM/CLAK/SCRIPT/ak_npc.bci|SYSTEM/CLAK/SCRIPT/ak_produktion.bci";

        private static readonly int?[] NpcSignature = new int?[] {
            128, 43, 73, -2, 86, 66, null, 96, 99, 117, 476
        };
        private const int OriginalFreeCivilianReserve = 0;
        private const int UltimateFreeCivilianReserve = 20;

        private static readonly int?[] ProductionSignature = new int?[] {
            128, 69, 73, -2, 86, null, 56, 66, 1, 82, 46
        };
        private const int ProductionGateOriginalOpcode = 117;
        private const int ProductionGateBypassOpcode = 112;

        public PatchState Detect(byte[] decompressed)
        {
            // Since this patch targets two files, TargetPattern has multiple paths separated by '|'.
            // The Detect method is called on whichever file is currently being evaluated.
            // Let's detect based on the signature we find in the decompressed buffer.
            int npcOffset = BciPattern.FindBciWordPattern(decompressed, NpcSignature);
            if (npcOffset >= 0)
            {
                int val = BitConverter.ToInt32(decompressed, npcOffset + 6 * 4);
                if (val == OriginalFreeCivilianReserve) return PatchState.Original;
                if (val == UltimateFreeCivilianReserve) return PatchState.Legacy; // Needs repair
                return PatchState.Unknown;
            }

            int prodOffset = BciPattern.FindBciWordPattern(decompressed, ProductionSignature);
            if (prodOffset >= 0)
            {
                int val = BitConverter.ToInt32(decompressed, prodOffset + 5 * 4);
                if (val == ProductionGateOriginalOpcode) return PatchState.Original;
                if (val == ProductionGateBypassOpcode) return PatchState.Legacy; // Needs repair
                return PatchState.Unknown;
            }

            return PatchState.Unknown;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            // Note: enabled is ignored here; P14 ALWAYS restores to original.
            bool changed = false;

            int npcOffset = BciPattern.FindBciWordPattern(decompressed, NpcSignature);
            if (npcOffset >= 0)
            {
                int offset = npcOffset + 6 * 4;
                int current = BitConverter.ToInt32(decompressed, offset);
                if (current != OriginalFreeCivilianReserve)
                {
                    if (current != UltimateFreeCivilianReserve)
                    {
                        throw new InvalidOperationException("P14 civilian reserve bytes do not match a supported pattern.");
                    }
                    BciPattern.WriteBciInt32(decompressed, offset, UltimateFreeCivilianReserve, OriginalFreeCivilianReserve, "P14 civilian reserve restore");
                    changed = true;
                }
            }

            int prodOffset = BciPattern.FindBciWordPattern(decompressed, ProductionSignature);
            if (prodOffset >= 0)
            {
                int offset = prodOffset + 5 * 4;
                int current = BitConverter.ToInt32(decompressed, offset);
                if (current != ProductionGateOriginalOpcode)
                {
                    if (current != ProductionGateBypassOpcode)
                    {
                        throw new InvalidOperationException("P14 production gate bytes do not match a supported pattern.");
                    }
                    BciPattern.WriteBciInt32(decompressed, offset, ProductionGateBypassOpcode, ProductionGateOriginalOpcode, "P14 production gate restore");
                    changed = true;
                }
            }

            return changed;
        }
    }
}
