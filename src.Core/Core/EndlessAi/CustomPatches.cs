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
        // 歷代出貨過的 Ultimate 門檻（8 → 40 → 30 → 40），全部視為 Legacy，
        // 下次套用時自動遷移到現行值。
        private static readonly int[] LegacyUnitThresholds = { 8, 30, 40 };
        // 2026-07-17 應使用者要求改為 70。上界試算：70 隊 × 20 人 = 1400，
        // 仍低於 EXE 全圖 1600 人口上限，但已相當接近——若含玩家人口爆滿
        // 導致增援停擺，優先檢查這裡。
        private const int UltimateUnitThreshold = 70;

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
            Array.IndexOf(LegacyUnitThresholds, value) >= 0;

        private static bool IsRecognizedUnitThreshold(int value) =>
            value == OriginalUnitThreshold ||
            value == UltimateUnitThreshold ||
            IsLegacyUnitThreshold(value);

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
        // 單位（平民、駄馬等單體單位）進入配額/捐贈分支，士兵小隊（type != 1）
        // 一律留在撤退陣列走回地圖出口。
        //
        // 2026-07-18 新局實測仍見士兵撤退，因此依使用者要求改為完全移交：
        // Ultimate 使用 jz(117)+0 吃掉型別分流，讓士兵、駄馬、平民全部進入
        // quota=0 的 donation 分支，再由第四控制點先切回 script mode 0，
        // 最後套用 recipient mark。舊 jz+0 的問題是 direct s_setObjMark，並非
        // 全單位 fall-through 本身；加上 release helper 後才是現行 Ultimate。
        // 簽章的跳躍 opcode 字（index 9）與位移字（index 10）都是萬用碼，
        // 否則已套用檔（118）或 Legacy 檔（位移 0）會比對不到。
        private static readonly int?[] DonationTypeFilterSignature = new int?[] {
            128, 214, 73, -2, 86, 66, 1, 96, 102, null, null
        };
        private const int DonationTypeFilterJzOpcodeWordIndex = 9;
        private const int DonationTypeFilterJzOperandWordIndex = 10;
        private const int DonationTypeFilterOriginalOpcode = 117;  // jz
        private const int DonationTypeFilterOriginalOperand = 92;
        private const int DonationTypeFilterPatchedOpcode = 117;   // jz
        private const int DonationTypeFilterPatchedOperand = 0;    // fall through: donate every unit
        private const int DonationTypeFilterLegacySplitOpcode = 118;
        private const int DonationTypeFilterLegacySplitOperand = 92;

        // Runtime testing on 2026-07-18 proved that a direct s_setObjMark leaves
        // donated soldier squads in reinforcement-party script mode and they
        // disappear at the village. Replace that external call with an internal
        // three-argument helper of the same call-site size. The helper releases
        // the squad (script mode 0) before applying the recipient mark.
        private static readonly int?[] DonationActionSignature = new int?[] {
            90, 42, 117, 56,
            90, 41, 90, 4, 90, 3,
            null, null, null, null, 86, 71,
            112, 116
        };
        private const int DonationActionCallOpcodeWordIndex = 10;
        private const int DonationActionCallOperandWordIndex = 11;
        private const int ExternalSymbolOpcode = 128;
        private const int InternalCallOpcode = 120;
        // The first helper-backed build accidentally emitted opcode 160 here.
        // VM dispatcher case 0xA0 is arrCreate(type, count), not an internal
        // function reference. Recognize that exact broken output only so Apply
        // can migrate installed maps and embedded saves to opcode 120.
        private const int BrokenArrayCreateOpcode = 160;
        private const int SetObjMarkSymbol = 141;

        internal static readonly int[] ReleaseAndRemarkHelperWords = new int[]
        {
            74, 94, 73, 3,
            66, 0,
            90, -4, 90, -3,
            128, 86, 73, -3, 86, 71,
            90, -5, 90, -4, 90, -3,
            128, 141, 73, -3, 86, 71,
            66, 1, 87, 112, 0,
            95, 75, 121
        };

        public PatchState Detect(byte[] decompressed)
        {
            var sites = BciPattern.FindAllBciWordPatternSites(decompressed, RetreatQuotaSignature);
            if (sites.Count == 0) return PatchState.Unknown;

            if (sites.Count != ExpectedQuotaSitesCount)
            {
                return PatchState.Unknown;
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

            bool site8LegacyZeroed = opcode8 == RetreatQuotaPatchedOpcode && value8 == RetreatQuotaPatchedValue;
            if (!site8Vanilla && !site8LegacyZeroed) return PatchState.Unknown;

            bool site9Ultimate = opcode9 == RetreatQuotaPatchedOpcode && value9 == RetreatQuotaPatchedValue;
            bool site9Original = opcode9 == RetreatQuotaOriginalOpcode && value9 == RetreatQuotaOriginalValue;
            if (!site9Ultimate && !site9Original) return PatchState.Unknown;

            bool filterOriginal = jzOpcode == DonationTypeFilterOriginalOpcode &&
                                  jzOperand == DonationTypeFilterOriginalOperand;
            bool filterUltimate = jzOpcode == DonationTypeFilterPatchedOpcode &&
                                  jzOperand == DonationTypeFilterPatchedOperand;
            bool filterLegacySplit = jzOpcode == DonationTypeFilterLegacySplitOpcode &&
                                     jzOperand == DonationTypeFilterLegacySplitOperand;
            if (!filterOriginal && !filterUltimate && !filterLegacySplit) return PatchState.Unknown;

            int actionOffset = FindDonationActionOffset(decompressed);
            if (actionOffset < 0) return PatchState.Unknown;
            int callOpcodeOffset = actionOffset + DonationActionCallOpcodeWordIndex * sizeof(int);
            int callOpcode = BitConverter.ToInt32(decompressed, callOpcodeOffset);
            int callOperand = BitConverter.ToInt32(
                decompressed,
                actionOffset + DonationActionCallOperandWordIndex * sizeof(int));

            bool hasHelper = TryLocateReleaseHelper(decompressed, out int helperOffset, out _);
            bool directRemark = callOpcode == ExternalSymbolOpcode && callOperand == SetObjMarkSymbol;
            bool releaseAndRemark = hasHelper &&
                                    callOpcode == InternalCallOpcode &&
                                    callOperand == CalculateInternalCallOperand(callOpcodeOffset, helperOffset);
            bool brokenArrayCreate = hasHelper &&
                                     callOpcode == BrokenArrayCreateOpcode &&
                                     callOperand == CalculateInternalCallOperand(callOpcodeOffset, helperOffset);
            if (!directRemark && !releaseAndRemark && !brokenArrayCreate) return PatchState.Unknown;
            if (directRemark && hasHelper) return PatchState.Unknown;

            if (site8Vanilla && site9Ultimate && filterUltimate && releaseAndRemark)
                return PatchState.Ultimate;
            if (site8Vanilla && site9Original && filterOriginal && directRemark && !hasHelper)
                return PatchState.Original;

            // Recognized shipped combinations converge on complete handoff with
            // the release helper, including old jz+0 direct-mark and jnz split states.
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

            if (Detect(decompressed) == PatchState.Unknown)
                throw new InvalidOperationException("P9 state is unknown; refusing to rewrite reinforcement control flow.");

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

            // Eat the type-filter jump so every reinforcement object enters the
            // donation branch. The action below releases party script mode before
            // applying the recipient mark; direct mark alone made squads disappear.
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

            changed |= enabled
                ? InstallReleaseHelper(ref decompressed)
                : RemoveReleaseHelper(ref decompressed);

            return changed;
        }

        private static bool InstallReleaseHelper(ref byte[] decompressed)
        {
            int actionOffset = FindDonationActionOffset(decompressed);
            if (actionOffset < 0)
                throw new InvalidOperationException("P9 donation action signature not found.");

            int callOpcodeOffset = actionOffset + DonationActionCallOpcodeWordIndex * sizeof(int);
            int callOperandOffset = actionOffset + DonationActionCallOperandWordIndex * sizeof(int);
            int callOpcode = BitConverter.ToInt32(decompressed, callOpcodeOffset);
            int callOperand = BitConverter.ToInt32(decompressed, callOperandOffset);

            bool changed = false;

            if (TryLocateReleaseHelper(decompressed, out int existingHelperOffset, out _))
            {
                int expectedOperand = CalculateInternalCallOperand(callOpcodeOffset, existingHelperOffset);
                if (callOperand != expectedOperand ||
                    (callOpcode != InternalCallOpcode && callOpcode != BrokenArrayCreateOpcode))
                    throw new InvalidOperationException("P9 release helper exists but its call target is inconsistent.");

                if (callOpcode != InternalCallOpcode)
                {
                    BciPattern.WriteBciInt32(
                        decompressed, callOpcodeOffset, callOpcode, InternalCallOpcode,
                        "P9 migrate broken arrCreate opcode to internal call");
                    changed = true;
                }
            }
            else
            {
                if (callOpcode != ExternalSymbolOpcode || callOperand != SetObjMarkSymbol)
                    throw new InvalidOperationException("P9 donation action is not the recognized direct-mark legacy state.");

                int helperOffset = InsertReleaseHelper(ref decompressed);
                int internalOperand = CalculateInternalCallOperand(callOpcodeOffset, helperOffset);
                BciPattern.WriteBciInt32(
                    decompressed, callOpcodeOffset, callOpcode, InternalCallOpcode,
                    "P9 donation release helper call opcode");
                BciPattern.WriteBciInt32(
                    decompressed, callOperandOffset, callOperand, internalOperand,
                    "P9 donation release helper call target");
                changed = true;
            }

            int bypassOpcodeOffset = callOpcodeOffset + 2 * sizeof(int);
            int bypassOperandOffset = callOpcodeOffset + 3 * sizeof(int);
            int currentBypassOpcode = BitConverter.ToInt32(decompressed, bypassOpcodeOffset);
            int currentBypassOperand = BitConverter.ToInt32(decompressed, bypassOperandOffset);

            if (currentBypassOpcode != 112 || currentBypassOperand != 4)
            {
                BciPattern.WriteBciInt32(
                    decompressed, bypassOpcodeOffset, currentBypassOpcode, 112,
                    "P9 bypass trailing external call opcode");
                BciPattern.WriteBciInt32(
                    decompressed, bypassOperandOffset, currentBypassOperand, 4,
                    "P9 bypass trailing external call operand");
                changed = true;
            }

            return changed;
        }

        private static bool RemoveReleaseHelper(ref byte[] decompressed)
        {
            int actionOffset = FindDonationActionOffset(decompressed);
            if (actionOffset < 0)
                throw new InvalidOperationException("P9 donation action signature not found.");

            int callOpcodeOffset = actionOffset + DonationActionCallOpcodeWordIndex * sizeof(int);
            int callOperandOffset = actionOffset + DonationActionCallOperandWordIndex * sizeof(int);
            int callOpcode = BitConverter.ToInt32(decompressed, callOpcodeOffset);
            int callOperand = BitConverter.ToInt32(decompressed, callOperandOffset);

            if (!TryLocateReleaseHelper(decompressed, out int helperOffset, out bool hasBciHeader))
            {
                int currentBypassOpcode = BitConverter.ToInt32(decompressed, callOpcodeOffset + 2 * sizeof(int));
                int currentBypassOperand = BitConverter.ToInt32(decompressed, callOpcodeOffset + 3 * sizeof(int));
                if (callOpcode == ExternalSymbolOpcode && callOperand == SetObjMarkSymbol &&
                    currentBypassOpcode == 73 && currentBypassOperand == -3)
                    return false;
                throw new InvalidOperationException("P9 donation call references an unknown helper state.");
            }

            int expectedOperand = CalculateInternalCallOperand(callOpcodeOffset, helperOffset);
            if (callOperand != expectedOperand ||
                (callOpcode != InternalCallOpcode && callOpcode != BrokenArrayCreateOpcode))
                throw new InvalidOperationException("P9 release helper call target is inconsistent.");

            BciPattern.WriteBciInt32(
                decompressed, callOpcodeOffset, callOpcode, ExternalSymbolOpcode,
                "P9 restore direct s_setObjMark opcode");
            BciPattern.WriteBciInt32(
                decompressed, callOperandOffset, callOperand, SetObjMarkSymbol,
                "P9 restore direct s_setObjMark symbol");

            int bypassOpcodeOffset = callOpcodeOffset + 2 * sizeof(int);
            int bypassOperandOffset = callOpcodeOffset + 3 * sizeof(int);
            int currentBypassOpcodeVal = BitConverter.ToInt32(decompressed, bypassOpcodeOffset);
            int currentBypassOperandVal = BitConverter.ToInt32(decompressed, bypassOperandOffset);
            BciPattern.WriteBciInt32(
                decompressed, bypassOpcodeOffset, currentBypassOpcodeVal, 73,
                "P9 restore trailing external call opcode");
            BciPattern.WriteBciInt32(
                decompressed, bypassOperandOffset, currentBypassOperandVal, -3,
                "P9 restore trailing external call operand");

            RemoveBytes(ref decompressed, helperOffset, ReleaseAndRemarkHelperWords.Length * sizeof(int), hasBciHeader);
            return true;
        }

        private static int FindDonationActionOffset(byte[] decompressed)
        {
            List<int> sites = BciPattern.FindAllBciWordPatternSites(decompressed, DonationActionSignature);
            return sites.Count == 1 ? sites[0] : -1;
        }

        private static int InsertReleaseHelper(ref byte[] decompressed)
        {
            bool hasBciHeader = TryGetBciCodeEnd(decompressed, out int codeEnd);
            if (!hasBciHeader) codeEnd = decompressed.Length;

            byte[] helper = WordsToBytes(ReleaseAndRemarkHelperWords);
            byte[] expanded = new byte[decompressed.Length + helper.Length];
            Buffer.BlockCopy(decompressed, 0, expanded, 0, codeEnd);
            Buffer.BlockCopy(helper, 0, expanded, codeEnd, helper.Length);
            Buffer.BlockCopy(decompressed, codeEnd, expanded, codeEnd + helper.Length, decompressed.Length - codeEnd);
            if (hasBciHeader)
            {
                int oldCodeSize = BitConverter.ToInt32(decompressed, 8);
                BitConverter.GetBytes(oldCodeSize + helper.Length).CopyTo(expanded, 8);
            }
            decompressed = expanded;
            return codeEnd;
        }

        private static void RemoveBytes(ref byte[] decompressed, int offset, int count, bool hasBciHeader)
        {
            byte[] reduced = new byte[decompressed.Length - count];
            Buffer.BlockCopy(decompressed, 0, reduced, 0, offset);
            Buffer.BlockCopy(decompressed, offset + count, reduced, offset, decompressed.Length - offset - count);
            if (hasBciHeader)
            {
                int oldCodeSize = BitConverter.ToInt32(decompressed, 8);
                BitConverter.GetBytes(oldCodeSize - count).CopyTo(reduced, 8);
            }
            decompressed = reduced;
        }

        private static bool TryLocateReleaseHelper(byte[] decompressed, out int helperOffset, out bool hasBciHeader)
        {
            hasBciHeader = TryGetBciCodeEnd(decompressed, out int codeEnd);
            if (!hasBciHeader) codeEnd = decompressed.Length;

            int helperLength = ReleaseAndRemarkHelperWords.Length * sizeof(int);
            helperOffset = codeEnd - helperLength;
            if (helperOffset < 0) return false;
            for (int i = 0; i < ReleaseAndRemarkHelperWords.Length; i++)
            {
                if (BitConverter.ToInt32(decompressed, helperOffset + i * sizeof(int)) != ReleaseAndRemarkHelperWords[i])
                    return false;
            }
            return true;
        }

        private static bool TryGetBciCodeEnd(byte[] decompressed, out int codeEnd)
        {
            codeEnd = 0;
            if (decompressed.Length < 44 ||
                decompressed[0] != (byte)'B' || decompressed[1] != (byte)'C' ||
                decompressed[2] != (byte)'I' || decompressed[3] != (byte)'0')
                return false;

            int codeSize = BitConverter.ToInt32(decompressed, 8);
            if (codeSize <= 0) return false;
            codeEnd = 0x24 + codeSize;
            if (codeEnd < 0x24 || codeEnd + 8 > decompressed.Length) return false;
            return decompressed.AsSpan(codeEnd, 8).SequenceEqual("SYMBCONS"u8);
        }

        private static int CalculateInternalCallOperand(int callOpcodeOffset, int helperOffset) =>
            helperOffset - callOpcodeOffset - 8;

        private static byte[] WordsToBytes(IReadOnlyList<int> words)
        {
            byte[] bytes = new byte[words.Count * sizeof(int)];
            for (int i = 0; i < words.Count; i++)
                BitConverter.GetBytes(words[i]).CopyTo(bytes, i * sizeof(int));
            return bytes;
        }
    }

    // ==========================================
    // P20: village garrison quota multiplier
    // ==========================================
    public sealed class P20_VillageGarrisonQuota3xPatch : IEndlessPatch
    {
        public string Id => "P20";
        public string TargetPattern => "SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci";

        private const int ExternalSymbolOpcode = 128;
        private const int InternalCallOpcode = 120;
        private const int SearchImportantPosSymbol = 151;
        private const int CallOpcodeWordIndex = 4;
        private const int CallOperandWordIndex = 5;

        private static readonly int?[][] CallSiteSignatures =
        {
            new int?[] { 66, 1, 81, 8, null, null, 73, -2, 86, 91, 59 },
            new int?[] { 66, 2, 81, 10, null, null, 73, -2, 86, 91, 61 },
            new int?[] { 66, 3, 81, 12, null, null, 73, -2, 86, 91, 63 },
            new int?[] { 66, 4, 81, 14, null, null, 73, -2, 86, 91, 65 },
        };

        // int quota3x(int importantPosType, int outputArray) {
        //     return s_searchImportantPos(importantPosType, outputArray) * 3;
        // }
        // Opcode 34 is the VM's signed int multiplication handler. The helper
        // is appended to the code section and all four same-size call sites are
        // redirected to it, so no existing branch offsets move.
        internal static readonly int[] Quota3xHelperWords =
        {
            74, 94, 73, 2,
            90, -4, 90, -3,
            128, SearchImportantPosSymbol, 73, -2, 86,
            66, 3, 34,
            87, 112, 0,
            95, 75, 121,
        };

        public PatchState Detect(byte[] decompressed)
        {
            if (!TryFindCallSites(decompressed, out int[] sites))
                return PatchState.Unknown;

            bool hasHelper = TryLocateHelper(decompressed, out int helperOffset, out _);
            bool allOriginal = true;
            bool allUltimate = hasHelper;

            foreach (int site in sites)
            {
                int callOpcodeOffset = site + CallOpcodeWordIndex * sizeof(int);
                int callOpcode = BitConverter.ToInt32(decompressed, callOpcodeOffset);
                int callOperand = BitConverter.ToInt32(
                    decompressed,
                    site + CallOperandWordIndex * sizeof(int));

                bool original = callOpcode == ExternalSymbolOpcode &&
                                callOperand == SearchImportantPosSymbol;
                bool ultimate = hasHelper &&
                                callOpcode == InternalCallOpcode &&
                                callOperand == CalculateInternalCallOperand(callOpcodeOffset, helperOffset);
                if (!original && !ultimate) return PatchState.Unknown;

                allOriginal &= original;
                allUltimate &= ultimate;
            }

            if (allOriginal && !hasHelper) return PatchState.Original;
            if (allUltimate) return PatchState.Ultimate;
            return PatchState.Unknown;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            PatchState state = Detect(decompressed);
            if (state == PatchState.Unknown)
                throw new InvalidOperationException("P20 village-garrison quota state is unknown; refusing to rewrite Dorfverteidigung control flow.");
            if (enabled && state == PatchState.Ultimate) return false;
            if (!enabled && state == PatchState.Original) return false;

            if (!TryFindCallSites(decompressed, out int[] sites))
                throw new InvalidOperationException("P20 village-garrison quota call sites are not unique.");

            if (enabled)
            {
                int helperOffset = InsertHelper(ref decompressed);
                foreach (int site in sites)
                {
                    int callOpcodeOffset = site + CallOpcodeWordIndex * sizeof(int);
                    int callOperandOffset = site + CallOperandWordIndex * sizeof(int);
                    BciPattern.WriteBciInt32(
                        decompressed, callOpcodeOffset, ExternalSymbolOpcode, InternalCallOpcode,
                        "P20 village quota helper call opcode");
                    BciPattern.WriteBciInt32(
                        decompressed, callOperandOffset, SearchImportantPosSymbol,
                        CalculateInternalCallOperand(callOpcodeOffset, helperOffset),
                        "P20 village quota helper call target");
                }
                return true;
            }

            if (!TryLocateHelper(decompressed, out int existingHelperOffset, out bool hasBciHeader))
                throw new InvalidOperationException("P20 village-garrison quota helper is missing.");

            foreach (int site in sites)
            {
                int callOpcodeOffset = site + CallOpcodeWordIndex * sizeof(int);
                int callOperandOffset = site + CallOperandWordIndex * sizeof(int);
                int expectedOperand = CalculateInternalCallOperand(callOpcodeOffset, existingHelperOffset);
                BciPattern.WriteBciInt32(
                    decompressed, callOpcodeOffset, InternalCallOpcode, ExternalSymbolOpcode,
                    "P20 restore s_searchImportantPos call opcode");
                BciPattern.WriteBciInt32(
                    decompressed, callOperandOffset, expectedOperand, SearchImportantPosSymbol,
                    "P20 restore s_searchImportantPos symbol");
            }
            RemoveBytes(ref decompressed, existingHelperOffset, Quota3xHelperWords.Length * sizeof(int), hasBciHeader);
            return true;
        }

        private static bool TryFindCallSites(byte[] decompressed, out int[] sites)
        {
            sites = new int[CallSiteSignatures.Length];
            int codeStart = HasBciHeader(decompressed) ? 0x24 : 0;
            int codeEnd = TryGetBciCodeEnd(decompressed, out int parsedCodeEnd)
                ? parsedCodeEnd
                : decompressed.Length;

            for (int i = 0; i < CallSiteSignatures.Length; i++)
            {
                List<int> matches = FindPatternSitesInRange(
                    decompressed, CallSiteSignatures[i], codeStart, codeEnd);
                if (matches.Count != 1) return false;
                sites[i] = matches[0];
            }
            return true;
        }

        private static List<int> FindPatternSitesInRange(
            byte[] data, int?[] pattern, int start, int end)
        {
            var sites = new List<int>();
            int byteLength = pattern.Length * sizeof(int);
            for (int offset = start; offset <= end - byteLength; offset += sizeof(int))
            {
                bool match = true;
                for (int i = 0; i < pattern.Length; i++)
                {
                    int? expected = pattern[i];
                    if (expected.HasValue &&
                        BitConverter.ToInt32(data, offset + i * sizeof(int)) != expected.Value)
                    {
                        match = false;
                        break;
                    }
                }
                if (match) sites.Add(offset);
            }
            return sites;
        }

        private static int InsertHelper(ref byte[] decompressed)
        {
            bool hasBciHeader = TryGetBciCodeEnd(decompressed, out int codeEnd);
            if (!hasBciHeader) codeEnd = decompressed.Length;

            byte[] helper = WordsToBytes(Quota3xHelperWords);
            byte[] expanded = new byte[decompressed.Length + helper.Length];
            Buffer.BlockCopy(decompressed, 0, expanded, 0, codeEnd);
            Buffer.BlockCopy(helper, 0, expanded, codeEnd, helper.Length);
            Buffer.BlockCopy(decompressed, codeEnd, expanded, codeEnd + helper.Length, decompressed.Length - codeEnd);
            if (hasBciHeader)
            {
                int oldCodeSize = BitConverter.ToInt32(decompressed, 8);
                BitConverter.GetBytes(oldCodeSize + helper.Length).CopyTo(expanded, 8);
            }
            decompressed = expanded;
            return codeEnd;
        }

        private static bool TryLocateHelper(byte[] decompressed, out int helperOffset, out bool hasBciHeader)
        {
            hasBciHeader = TryGetBciCodeEnd(decompressed, out int codeEnd);
            if (!hasBciHeader) codeEnd = decompressed.Length;

            int helperLength = Quota3xHelperWords.Length * sizeof(int);
            helperOffset = codeEnd - helperLength;
            if (helperOffset < 0) return false;
            for (int i = 0; i < Quota3xHelperWords.Length; i++)
            {
                if (BitConverter.ToInt32(decompressed, helperOffset + i * sizeof(int)) != Quota3xHelperWords[i])
                    return false;
            }
            return true;
        }

        private static bool HasBciHeader(byte[] decompressed) =>
            decompressed.Length >= 4 &&
            decompressed[0] == (byte)'B' && decompressed[1] == (byte)'C' &&
            decompressed[2] == (byte)'I' && decompressed[3] == (byte)'0';

        private static bool TryGetBciCodeEnd(byte[] decompressed, out int codeEnd)
        {
            codeEnd = 0;
            if (!HasBciHeader(decompressed) || decompressed.Length < 44) return false;
            int codeSize = BitConverter.ToInt32(decompressed, 8);
            if (codeSize <= 0) return false;
            codeEnd = 0x24 + codeSize;
            return codeEnd >= 0x24 && codeEnd + 8 <= decompressed.Length &&
                   decompressed.AsSpan(codeEnd, 8).SequenceEqual("SYMBCONS"u8);
        }

        private static void RemoveBytes(ref byte[] decompressed, int offset, int count, bool hasBciHeader)
        {
            byte[] reduced = new byte[decompressed.Length - count];
            Buffer.BlockCopy(decompressed, 0, reduced, 0, offset);
            Buffer.BlockCopy(decompressed, offset + count, reduced, offset, decompressed.Length - offset - count);
            if (hasBciHeader)
            {
                int oldCodeSize = BitConverter.ToInt32(decompressed, 8);
                BitConverter.GetBytes(oldCodeSize - count).CopyTo(reduced, 8);
            }
            decompressed = reduced;
        }

        private static int CalculateInternalCallOperand(int callOpcodeOffset, int helperOffset) =>
            helperOffset - callOpcodeOffset - 8;

        private static byte[] WordsToBytes(IReadOnlyList<int> words)
        {
            byte[] bytes = new byte[words.Count * sizeof(int)];
            for (int i = 0; i < words.Count; i++)
                BitConverter.GetBytes(words[i]).CopyTo(bytes, i * sizeof(int));
            return bytes;
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
