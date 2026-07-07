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
        private const int AcceleratedLoopCount = 6;

        public PatchState Detect(byte[] decompressed)
        {
            int expectedIndex = 0;
            bool allOriginal = true;
            bool allUltimate = true;

            for (int offset = 0; offset <= decompressed.Length - 24 && expectedIndex < LoopDelayRanges.Length; offset += 4)
            {
                if (BitConverter.ToInt32(decompressed, offset) != 0x42 ||
                    BitConverter.ToInt32(decompressed, offset + 8) != 0x42 ||
                    BitConverter.ToInt32(decompressed, offset + 16) != 0x80 ||
                    BitConverter.ToInt32(decompressed, offset + 20) != 16)
                {
                    continue;
                }

                (int originalUpperMs, int originalLowerMs) = LoopDelayRanges[expectedIndex];
                int currentUpperMs = BitConverter.ToInt32(decompressed, offset + 4);
                int currentLowerMs = BitConverter.ToInt32(decompressed, offset + 12);

                bool matchesOriginal = currentUpperMs == originalUpperMs && currentLowerMs == originalLowerMs;
                bool matchesUltimate = currentUpperMs == UltimateLoopDelayUpperMs && currentLowerMs == UltimateLoopDelayLowerMs;
                bool matchesLegacy = currentUpperMs == LegacyLoopDelayUpperMs && currentLowerMs == LegacyLoopDelayLowerMs;

                if (matchesOriginal)
                {
                    allUltimate = false;
                    expectedIndex++;
                }
                else if (matchesUltimate)
                {
                    allOriginal = false;
                    expectedIndex++;
                }
                else if (matchesLegacy)
                {
                    allOriginal = false;
                    allUltimate = false;
                    expectedIndex++;
                }
            }

            if (expectedIndex != LoopDelayRanges.Length)
            {
                return PatchState.Unknown;
            }

            if (allOriginal) return PatchState.Original;
            if (allUltimate) return PatchState.Ultimate;
            return PatchState.Legacy;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            bool changed = false;
            int delaySiteIndex = 0;

            for (int offset = 0; offset <= decompressed.Length - 24; offset += 4)
            {
                if (BitConverter.ToInt32(decompressed, offset) != 0x42 ||
                    BitConverter.ToInt32(decompressed, offset + 8) != 0x42 ||
                    BitConverter.ToInt32(decompressed, offset + 16) != 0x80 ||
                    BitConverter.ToInt32(decompressed, offset + 20) != 16)
                {
                    continue;
                }

                if (delaySiteIndex >= LoopDelayRanges.Length)
                {
                    continue;
                }

                (int originalUpperMs, int originalLowerMs) = LoopDelayRanges[delaySiteIndex];
                int currentUpperMs = BitConverter.ToInt32(decompressed, offset + 4);
                int currentLowerMs = BitConverter.ToInt32(decompressed, offset + 12);

                bool matchesOriginal = currentUpperMs == originalUpperMs && currentLowerMs == originalLowerMs;
                bool matchesUltimate = currentUpperMs == UltimateLoopDelayUpperMs && currentLowerMs == UltimateLoopDelayLowerMs;
                bool matchesLegacy = currentUpperMs == LegacyLoopDelayUpperMs && currentLowerMs == LegacyLoopDelayLowerMs;

                if (!matchesOriginal && !matchesUltimate && !matchesLegacy)
                {
                    continue;
                }

                bool accelerateSchedulerLoop = enabled && delaySiteIndex < AcceleratedLoopCount;
                int targetUpperMs = accelerateSchedulerLoop ? UltimateLoopDelayUpperMs : originalUpperMs;
                int targetLowerMs = accelerateSchedulerLoop ? UltimateLoopDelayLowerMs : originalLowerMs;

                if (currentUpperMs != targetUpperMs || currentLowerMs != targetLowerMs)
                {
                    BciPattern.WriteBciInt32(decompressed, offset + 4, currentUpperMs, targetUpperMs, "P6 scheduler upper delay");
                    BciPattern.WriteBciInt32(decompressed, offset + 12, currentLowerMs, targetLowerMs, "P6 scheduler lower delay");
                    changed = true;
                }
                delaySiteIndex++;
            }

            if (delaySiteIndex != LoopDelayRanges.Length)
            {
                throw new InvalidOperationException("P6 site count mismatch during Apply.");
            }

            return changed;
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
    public class P8_ActiveLimitPatch : IEndlessPatch
    {
        public string Id => "P8";
        public string TargetPattern => "MAPS/ENDL_*/SCRIPT/ak_level.bci";

        private const int OriginalActivePartyLimit = 4;
        private const int LegacyActivePartyLimit = 8;
        private const int UltimateActivePartyLimit = 40;

        // Original gate: 5× pushlit 0 (66,0) + pushsym 6 (90,6) + cmp (102) + jmp (117) + done (32)
        // Total 15 words at offset +32 from sequence start.
        // We keep these as the canonical "original" values to restore to.
        private static readonly int[] OriginalGateWords = {
            66, 0, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32
        };

        // Legacy BoundedGate variants introduced in experimental builds.
        // All share this shape: pushsym,X, jmp,Y repeated 3× then jmprel,Z, done.
        // We recognize them generically by checking the first word is 90 (pushsym).
        private const int BoundedGateLeadOpcode = 90;

        public PatchState Detect(byte[] decompressed)
        {
            int sequenceOffset = FindActiveLimitSequenceOffset(decompressed);
            if (sequenceOffset < 0) return PatchState.Unknown;

            int currentLimit = BitConverter.ToInt32(decompressed, sequenceOffset + 12);

            // Check gate region: starts at offset +32, first word indicates gate type
            int gateStart = sequenceOffset + 32;
            int firstGateWord = BitConverter.ToInt32(decompressed, gateStart);

            bool isOriginalGate = IsOriginalGate(decompressed, gateStart);
            bool isBoundedGate = firstGateWord == BoundedGateLeadOpcode && !isOriginalGate;

            if (currentLimit == OriginalActivePartyLimit && isOriginalGate)
            {
                return PatchState.Original;
            }
            if (currentLimit == UltimateActivePartyLimit && isOriginalGate)
            {
                return PatchState.Ultimate;
            }
            // Any non-original gate or legacy limit values → Legacy (can be migrated)
            if (isBoundedGate || currentLimit == LegacyActivePartyLimit)
            {
                return PatchState.Legacy;
            }
            // Any other limit value with any gate — treat as Legacy to allow migration
            return PatchState.Legacy;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            int sequenceOffset = FindActiveLimitSequenceOffset(decompressed);
            if (sequenceOffset < 0)
            {
                if (!enabled) return false;
                throw new InvalidOperationException("P8 active limit sequence not found.");
            }

            bool changed = false;
            int targetLimit = enabled ? UltimateActivePartyLimit : OriginalActivePartyLimit;
            int limitOffset = sequenceOffset + 12;

            int currentLimit = BitConverter.ToInt32(decompressed, limitOffset);
            if (currentLimit != targetLimit)
            {
                BciPattern.WriteBciInt32(decompressed, limitOffset, currentLimit, targetLimit, "P8 active party limit");
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
        {
            if (gateOffset + OriginalGateWords.Length * 4 > data.Length) return false;
            for (int i = 0; i < OriginalGateWords.Length; i++)
            {
                if (BitConverter.ToInt32(data, gateOffset + i * 4) != OriginalGateWords[i])
                    return false;
            }
            return true;
        }

        private static int FindActiveLimitSequenceOffset(byte[] decompressedBci)
        {
            // Use null wildcards for the entire gate region (15 words at offset 8..22)
            // so we can match both the original gate (66,0 series) and any legacy
            // BoundedGate variant (90,11,117,... jump instructions).
            // This 23-word pattern is unique in all ak_level.bci files, so no tail anchor is required.
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

        private static readonly int?[] RetreatQuotaSignature = new int?[] {
            81, 57, 90, -3, 90, 14, 164, 81, 56, 90, -3, null, null, 164, 81, 61
        };

        private const int RetreatQuotaOriginalOpcode = 90;
        private const int RetreatQuotaOriginalValue = 15;
        private const int RetreatQuotaPatchedOpcode = 66;
        private const int RetreatQuotaPatchedValue = 0;
        private const int RetreatQuotaOpcodeWordIndex = 11;

        public PatchState Detect(byte[] decompressed)
        {
            int sigOffset = BciPattern.FindBciWordPattern(decompressed, RetreatQuotaSignature);
            if (sigOffset < 0) return PatchState.Unknown;

            int opcodeOffset = sigOffset + RetreatQuotaOpcodeWordIndex * 4;
            int opcode = BitConverter.ToInt32(decompressed, opcodeOffset);
            int value = BitConverter.ToInt32(decompressed, opcodeOffset + 4);

            if (opcode == RetreatQuotaOriginalOpcode && value == RetreatQuotaOriginalValue)
            {
                return PatchState.Original;
            }
            if (opcode == RetreatQuotaPatchedOpcode && value == RetreatQuotaPatchedValue)
            {
                return PatchState.Ultimate;
            }
            // Any other value combination (e.g., from experimental builds) is Legacy;
            // Apply will safely overwrite to the correct target values.
            return PatchState.Legacy;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            int sigOffset = BciPattern.FindBciWordPattern(decompressed, RetreatQuotaSignature);
            if (sigOffset < 0)
            {
                if (!enabled) return false;
                throw new InvalidOperationException("P9 retreat quota signature not found.");
            }

            int opcodeOffset = sigOffset + RetreatQuotaOpcodeWordIndex * 4;
            int targetOpcode = enabled ? RetreatQuotaPatchedOpcode : RetreatQuotaOriginalOpcode;
            int targetValue = enabled ? RetreatQuotaPatchedValue : RetreatQuotaOriginalValue;

            bool changed = false;
            if (BitConverter.ToInt32(decompressed, opcodeOffset) != targetOpcode ||
                BitConverter.ToInt32(decompressed, opcodeOffset + 4) != targetValue)
            {
                int currentOpcode = BitConverter.ToInt32(decompressed, opcodeOffset);
                int currentValue = BitConverter.ToInt32(decompressed, opcodeOffset + 4);
                BciPattern.WriteBciInt32(decompressed, opcodeOffset, currentOpcode, targetOpcode, "P9 retreat quota opcode");
                BciPattern.WriteBciInt32(decompressed, opcodeOffset + 4, currentValue, targetValue, "P9 retreat quota value");
                changed = true;
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
