namespace AgainstRomeModifier.Core.Patches;

/// <summary>Against_Rome.exe 失焦暫停相容性補丁狀態。</summary>
public enum ExePatchState {
    Unknown,
    Original,
    FocusPatched
}

/// <summary>法術免祭壇需求補丁狀態。</summary>
public enum ExeSpellAltarPatchState {
    Unknown,
    Original,
    Patched
}

/// <summary>無盡模式部族選擇強制為羅馬的補丁狀態。</summary>
public enum ExeRomanEndlessPatchState {
    Unknown,
    Original,
    Patched
}

/// <summary>已淘汰的村落建造範圍候選補丁（僅用於偵測與還原舊寫入）狀態。</summary>
public enum ExeVillageRangePatchState {
    Unknown,
    Original,
    LegacyLogicOnly,
    Expanded
}

/// <summary>村落建造範圍 setter 跳板補丁狀態。</summary>
public enum ExeVillageSetterPatchState {
    Unknown,
    Original,
    Legacy2x,
    Legacy2Point5x,
    Legacy3x,
    Legacy5x,
    EntireMap
}

/// <summary>單一固定偏移寫入計畫：只有目前位元組等於 <see cref="Expected"/> 時才允許覆寫為 <see cref="Replacement"/>。</summary>
public readonly record struct ExeWriteOp(long Offset, byte[] Expected, byte[] Replacement, string PatchName);

/// <summary>
/// Against_Rome.exe 固定偏移補丁的純資料模型：集中偵測目前狀態，並依狀態規劃
/// 「預期位元組 → 取代位元組」的寫入清單。所有邏輯不依賴 UI，可單獨測試；
/// 呼叫端（<c>ModifierForm</c>）只負責日誌、在地化與 exeModified 旗標。
/// </summary>
public static class ExePatchModel {
    // === 失焦暫停相容性 ===
    public static readonly byte[] FocusOriginalBytes = { 0x89, 0x15, 0xC4, 0x7D, 0x9E, 0x02 };
    public static readonly byte[] FocusPatchedBytes = { 0x90, 0x90, 0x90, 0x90, 0x90, 0x90 };
    public const long FocusPatchOffset = 0x161a88;
    public const long FocusPatchRequiredLength = 0x161a8e;

    // === 無盡模式 dlg_volk 部族選擇強制為羅馬 ===
    public const long RomanEndlessPatchOffset = 0x5bd60;
    public static readonly byte[] RomanEndlessOriginalBytes = {
        0x53, 0x8B, 0x5C, 0x24, 0x08, 0x53, 0xE8, 0x25, 0x3B, 0xFE, 0xFF,
        0x83, 0xC4, 0x04, 0x53, 0x89, 0x1D, 0x78, 0x74, 0x73, 0x00
    };
    public static readonly byte[] RomanEndlessPatchedBytes = {
        0x53, 0x6A, 0x03, 0x5B, 0x90, 0x53, 0xE8, 0x25, 0x3B, 0xFE, 0xFF,
        0x83, 0xC4, 0x04, 0x53, 0x89, 0x1D, 0x78, 0x74, 0x73, 0x00
    };

    // === 法術免祭壇需求（各族群 12 處特徵）===
    public static readonly (long Offset, byte[] Original, byte[] Patched)[] SpellAltarPatchSites = new[] {
        // Germans
        (0x4A112L, new byte[] { 0x83, 0xFE, 0x01, 0x0F, 0x8C, 0x54, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x54, 0xFF, 0xFF, 0xFF }),
        (0x4A136L, new byte[] { 0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x58, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x58, 0xFF, 0xFF, 0xFF }),
        (0x4A15AL, new byte[] { 0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x5C, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x5C, 0xFF, 0xFF, 0xFF }),
        (0x4A0E3L, new byte[] { 0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x92, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x92, 0x00, 0x00, 0x00 }),

        // Celts
        (0x4A1CCL, new byte[] { 0x83, 0xFE, 0x01, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }),
        (0x4A293L, new byte[] { 0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }),
        (0x4A2B7L, new byte[] { 0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }),
        (0x4A249L, new byte[] { 0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }),

        // Huns
        (0x4A329L, new byte[] { 0x83, 0xFE, 0x01, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }),
        (0x4A3F0L, new byte[] { 0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }),
        (0x4A414L, new byte[] { 0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }),
        (0x4A3A6L, new byte[] { 0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }),
    };

    // === 已淘汰的村落建造範圍候選（保留以偵測並還原舊寫入）===
    public static readonly byte[] VillageRangeXOriginalBytes = { 0xC1, 0xE2, 0x06 };
    public static readonly byte[] VillageRangeZOriginalBytes = { 0xC1, 0xE1, 0x06 };
    public static readonly byte[] VillageRangeXPatchedBytes = { 0xC1, 0xE2, 0x07 };
    public static readonly byte[] VillageRangeZPatchedBytes = { 0xC1, 0xE1, 0x07 };
    public static readonly byte[] VillageFrameXOriginalBytes = { 0xC1, 0xE6, 0x06 };
    public static readonly byte[] VillageFrameZOriginalBytes = { 0xC1, 0xE7, 0x06 };
    public static readonly byte[] VillageFrameXPatchedBytes = { 0xC1, 0xE6, 0x07 };
    public static readonly byte[] VillageFrameZPatchedBytes = { 0xC1, 0xE7, 0x07 };
    public const long VillageRangeXPatchOffset = 0x1366c4;
    public const long VillageRangeZPatchOffset = 0x1366cd;
    public const long VillageFrameXPatchOffset = 0x0d722c;
    public const long VillageFrameZPatchOffset = 0x0d723b;
    public const long VillageRangePatchRequiredLength = 0x1366d0;

    // === 村落建造範圍 setter 跳板 ===
    public static readonly byte[] VillageSetterHookOriginalBytes = {
        0x85, 0xF6, 0x7C, 0xA6, 0x85, 0xFF, 0x7C, 0xA2
    };
    public static readonly byte[] VillageSetterHookPatchedBytes = {
        0xE9, 0xC9, 0xC0, 0x02, 0x00, 0x90, 0x90, 0x90
    };
    public static readonly byte[] VillageSetterCaveOriginalBytes = new byte[39];
    // 舊版 modifier 安裝 33 位元組的 2x 跳板並留下 6 位元組零填補；保留辨識以便
    // Apply 能把已修補的執行檔遷移到目前的 2.5x 版本。
    public static readonly byte[] VillageSetterCaveLegacy2xBytes = {
        0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
        0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
        0xD1, 0xE6, 0xD1, 0xE7, 0x57, 0x56, 0x50, 0xE8,
        0x55, 0xE3, 0xF5, 0xFF, 0xE9, 0x21, 0x3F, 0xFD, 0xFF,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00
    };
    public static readonly byte[] VillageSetterCaveLegacy3xBytes = {
        0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
        0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
        0x8D, 0x34, 0x76, 0x90, 0x90,
        0x8D, 0x3C, 0x7F, 0x90, 0x90,
        0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
        0xE9, 0x1B, 0x3F, 0xFD, 0xFF
    };
    public static readonly byte[] VillageSetterCaveLegacy5xBytes = {
        0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
        0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
        0x8D, 0x34, 0xB6, 0x90, 0x90,
        0x8D, 0x3C, 0xBF, 0x90, 0x90,
        0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
        0xE9, 0x1B, 0x3F, 0xFD, 0xFF
    };
    public static readonly byte[] VillageSetterCavePatchedBytes = {
        0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
        0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
        0xBE, 0x30, 0x75, 0x00, 0x00, // mov esi, 30000 (0x7530)
        0xBF, 0x30, 0x75, 0x00, 0x00, // mov edi, 30000 (0x7530)
        0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
        0xE9, 0x1B, 0x3F, 0xFD, 0xFF
    };
    public static readonly byte[] VillageSetterCaveLegacy2Point5xBytes = {
        0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
        0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
        0x8D, 0x34, 0xB6, 0xD1, 0xEE,
        0x8D, 0x3C, 0xBF, 0xD1, 0xEF,
        0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
        0xE9, 0x1B, 0x3F, 0xFD, 0xFF
    };
    public const long VillageSetterHookOffset = 0x1364c1;
    public const long VillageSetterCaveOffset = 0x16258f;
    public const long VillageSetterPatchRequiredLength = 0x1625b6;

    // === 遊戲整體時脈加速（主時脈常數縮放）===
    // 主時脈函式（VA 0x55e530）回傳「奈秒級的當前時間(double)」，全遊戲模擬（移動／
    // 生產／戰鬥／AI）都透過它取得時間並以差值推進。它有兩條路徑，各用一個 rodata
    // double 常數換算單位：
    //   QPC 路徑（高精度計時器）：scale = 1e9 / freq，常數 1e9 位於 VA 0x604214／檔案偏移 0x204214
    //   timeGetTime 路徑（退回）：time = (elapsed_ms) * 1e6，常數 1e6 位於 VA 0x60424c／檔案偏移 0x20424c
    // 兩個常數各自「只被時脈碼引用一次、單一用途」（經 xref 確認），因此同步把兩者乘上
    // 相同倍率 s，即可讓整個遊戲時脈以 s 倍速前進——不論玩家機器走哪條路徑。差值運算在
    // 遊戲自身邏輯中先減去基準再乘常數，故無 32-bit 溢位風險，且時間仍由 ~0 開始、無跳變。
    // 完全可逆：把常數改回原值即還原。
    public const long GameSpeedQpcConstOffset = 0x204214; // 原版 double 1e9
    public const long GameSpeedTgtConstOffset = 0x20424c; // 原版 double 1e6
    private const double GameSpeedQpcBase = 1_000_000_000.0;
    private const double GameSpeedTgtBase = 1_000_000.0;
    /// <summary>工具提供的加速倍率選項（1 = 原版／關閉，最高 10 倍）。</summary>
    public static readonly int[] GameSpeedSupportedMultipliers = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 };

    /// <summary>
    /// 偵測目前時脈倍率：回傳 1（原版）／2／3／4…；若兩條路徑常數不一致、非整數倍或超出範圍，
    /// 回傳 0 表示 Unknown（呼叫端不應改寫）。
    /// </summary>
    public static int GetGameSpeedMultiplier(byte[] exeBytes) {
        if (exeBytes.Length < GameSpeedQpcConstOffset + 8 || exeBytes.Length < GameSpeedTgtConstOffset + 8) {
            return 0;
        }
        double qpc = BitConverter.ToDouble(exeBytes, (int)GameSpeedQpcConstOffset);
        double tgt = BitConverter.ToDouble(exeBytes, (int)GameSpeedTgtConstOffset);
        if (qpc <= 0 || tgt <= 0) return 0;
        double sQpc = qpc / GameSpeedQpcBase;
        double sTgt = tgt / GameSpeedTgtBase;
        int rq = (int)Math.Round(sQpc);
        int rt = (int)Math.Round(sTgt);
        if (rq != rt || rq < 1) return 0;
        if (Math.Abs(sQpc - rq) > 1e-6 || Math.Abs(sTgt - rt) > 1e-6) return 0;
        return rq;
    }

    /// <summary>
    /// 規劃把時脈倍率從 <paramref name="currentMultiplier"/> 切換為 <paramref name="desiredMultiplier"/>
    /// 的寫入清單。currentMultiplier 為 0（Unknown）時不動作；倍率相同時不動作。
    /// desiredMultiplier = 1 代表還原為原版。
    /// </summary>
    public static IReadOnlyList<ExeWriteOp> PlanGameSpeed(int desiredMultiplier, int currentMultiplier) {
        if (currentMultiplier <= 0) return Array.Empty<ExeWriteOp>();
        if (desiredMultiplier < 1) desiredMultiplier = 1;
        if (desiredMultiplier == currentMultiplier) return Array.Empty<ExeWriteOp>();

        byte[] expectedQpc = BitConverter.GetBytes(GameSpeedQpcBase * currentMultiplier);
        byte[] expectedTgt = BitConverter.GetBytes(GameSpeedTgtBase * currentMultiplier);
        byte[] newQpc = BitConverter.GetBytes(GameSpeedQpcBase * desiredMultiplier);
        byte[] newTgt = BitConverter.GetBytes(GameSpeedTgtBase * desiredMultiplier);
        string name = desiredMultiplier == 1 ? "遊戲加速還原" : $"遊戲加速 {desiredMultiplier}×";
        return new[] {
            new ExeWriteOp(GameSpeedQpcConstOffset, expectedQpc, newQpc, name + "（QPC 路徑）"),
            new ExeWriteOp(GameSpeedTgtConstOffset, expectedTgt, newTgt, name + "（timeGetTime 路徑）"),
        };
    }

    // === 狀態偵測 ===
    public static ExePatchState GetExePatchState(byte[] exeBytes) {
        if (exeBytes.Length < FocusPatchRequiredLength) {
            return ExePatchState.Unknown;
        }
        byte[] bytes = new byte[FocusOriginalBytes.Length];
        Buffer.BlockCopy(exeBytes, (int)FocusPatchOffset, bytes, 0, bytes.Length);
        if (bytes.SequenceEqual(FocusOriginalBytes)) return ExePatchState.Original;
        if (bytes.SequenceEqual(FocusPatchedBytes)) return ExePatchState.FocusPatched;
        return ExePatchState.Unknown;
    }

    public static ExeSpellAltarPatchState GetSpellAltarPatchState(byte[] exeBytes) {
        bool allOriginal = true;
        bool allPatched = true;

        foreach (var site in SpellAltarPatchSites) {
            if (exeBytes.Length < site.Offset + site.Original.Length) {
                return ExeSpellAltarPatchState.Unknown;
            }
            byte[] current = new byte[site.Original.Length];
            Buffer.BlockCopy(exeBytes, (int)site.Offset, current, 0, current.Length);

            if (!current.SequenceEqual(site.Original)) {
                allOriginal = false;
            }
            if (!current.SequenceEqual(site.Patched)) {
                allPatched = false;
            }
        }

        if (allOriginal) return ExeSpellAltarPatchState.Original;
        if (allPatched) return ExeSpellAltarPatchState.Patched;
        return ExeSpellAltarPatchState.Unknown;
    }

    public static ExeRomanEndlessPatchState GetRomanEndlessPatchState(byte[] exeBytes) {
        if (exeBytes.Length < RomanEndlessPatchOffset + RomanEndlessOriginalBytes.Length) {
            return ExeRomanEndlessPatchState.Unknown;
        }
        byte[] bytes = ReadSpan(exeBytes, RomanEndlessPatchOffset, RomanEndlessOriginalBytes.Length);
        if (bytes.SequenceEqual(RomanEndlessOriginalBytes)) return ExeRomanEndlessPatchState.Original;
        if (bytes.SequenceEqual(RomanEndlessPatchedBytes)) return ExeRomanEndlessPatchState.Patched;
        return ExeRomanEndlessPatchState.Unknown;
    }

    public static ExeVillageRangePatchState GetVillageBuildRangePatchState(byte[] exeBytes) {
        if (exeBytes.Length < VillageRangePatchRequiredLength) {
            return ExeVillageRangePatchState.Unknown;
        }

        byte[] xBytes = ReadSpan(exeBytes, VillageRangeXPatchOffset, VillageRangeXOriginalBytes.Length);
        byte[] zBytes = ReadSpan(exeBytes, VillageRangeZPatchOffset, VillageRangeZOriginalBytes.Length);
        byte[] frameXBytes = ReadSpan(exeBytes, VillageFrameXPatchOffset, VillageFrameXOriginalBytes.Length);
        byte[] frameZBytes = ReadSpan(exeBytes, VillageFrameZPatchOffset, VillageFrameZOriginalBytes.Length);

        bool original = xBytes.SequenceEqual(VillageRangeXOriginalBytes) &&
            zBytes.SequenceEqual(VillageRangeZOriginalBytes) &&
            frameXBytes.SequenceEqual(VillageFrameXOriginalBytes) &&
            frameZBytes.SequenceEqual(VillageFrameZOriginalBytes);
        bool legacyLogicOnly = xBytes.SequenceEqual(VillageRangeXPatchedBytes) &&
            zBytes.SequenceEqual(VillageRangeZPatchedBytes) &&
            frameXBytes.SequenceEqual(VillageFrameXOriginalBytes) &&
            frameZBytes.SequenceEqual(VillageFrameZOriginalBytes);
        bool expanded = xBytes.SequenceEqual(VillageRangeXPatchedBytes) &&
            zBytes.SequenceEqual(VillageRangeZPatchedBytes) &&
            frameXBytes.SequenceEqual(VillageFrameXPatchedBytes) &&
            frameZBytes.SequenceEqual(VillageFrameZPatchedBytes);
        if (original) return ExeVillageRangePatchState.Original;
        if (legacyLogicOnly) return ExeVillageRangePatchState.LegacyLogicOnly;
        if (expanded) return ExeVillageRangePatchState.Expanded;
        return ExeVillageRangePatchState.Unknown;
    }

    public static ExeVillageSetterPatchState GetVillageSetterPatchState(byte[] exeBytes) {
        if (exeBytes.Length < VillageSetterPatchRequiredLength) {
            return ExeVillageSetterPatchState.Unknown;
        }

        byte[] hookBytes = ReadSpan(exeBytes, VillageSetterHookOffset, VillageSetterHookOriginalBytes.Length);
        byte[] caveBytes = ReadSpan(exeBytes, VillageSetterCaveOffset, VillageSetterCaveOriginalBytes.Length);

        bool original = hookBytes.SequenceEqual(VillageSetterHookOriginalBytes) &&
            caveBytes.SequenceEqual(VillageSetterCaveOriginalBytes);
        bool legacy2x = hookBytes.SequenceEqual(VillageSetterHookPatchedBytes) &&
            caveBytes.SequenceEqual(VillageSetterCaveLegacy2xBytes);
        bool legacy2Point5x = hookBytes.SequenceEqual(VillageSetterHookPatchedBytes) &&
            caveBytes.SequenceEqual(VillageSetterCaveLegacy2Point5xBytes);
        bool legacy3x = hookBytes.SequenceEqual(VillageSetterHookPatchedBytes) &&
            caveBytes.SequenceEqual(VillageSetterCaveLegacy3xBytes);
        bool legacy5x = hookBytes.SequenceEqual(VillageSetterHookPatchedBytes) &&
            caveBytes.SequenceEqual(VillageSetterCaveLegacy5xBytes);
        bool entireMap = hookBytes.SequenceEqual(VillageSetterHookPatchedBytes) &&
            caveBytes.SequenceEqual(VillageSetterCavePatchedBytes);
        if (original) return ExeVillageSetterPatchState.Original;
        if (legacy2x) return ExeVillageSetterPatchState.Legacy2x;
        if (legacy2Point5x) return ExeVillageSetterPatchState.Legacy2Point5x;
        if (legacy3x) return ExeVillageSetterPatchState.Legacy3x;
        if (legacy5x) return ExeVillageSetterPatchState.Legacy5x;
        if (entireMap) return ExeVillageSetterPatchState.EntireMap;
        return ExeVillageSetterPatchState.Unknown;
    }

    // === 依狀態規劃寫入（核心：state → 預期/取代位元組選擇）===
    public static IReadOnlyList<ExeWriteOp> PlanFocus(bool enabled, ExePatchState state) {
        if (enabled && state == ExePatchState.Original) {
            return new[] { new ExeWriteOp(FocusPatchOffset, FocusOriginalBytes, FocusPatchedBytes, "失焦暫停相容性") };
        }
        if (!enabled && state == ExePatchState.FocusPatched) {
            return new[] { new ExeWriteOp(FocusPatchOffset, FocusPatchedBytes, FocusOriginalBytes, "失焦暫停相容性還原") };
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanSpellAltar(bool enabled, ExeSpellAltarPatchState state) {
        if (enabled && state == ExeSpellAltarPatchState.Original) {
            return SpellAltarPatchSites
                .Select(site => new ExeWriteOp(site.Offset, site.Original, site.Patched, "法術免祭壇需求"))
                .ToArray();
        }
        if (!enabled && state == ExeSpellAltarPatchState.Patched) {
            return SpellAltarPatchSites
                .Select(site => new ExeWriteOp(site.Offset, site.Patched, site.Original, "法術免祭壇需求還原"))
                .ToArray();
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanRomanEndless(bool enabled, ExeRomanEndlessPatchState state) {
        if (enabled && state == ExeRomanEndlessPatchState.Original) {
            return new[] { new ExeWriteOp(RomanEndlessPatchOffset, RomanEndlessOriginalBytes, RomanEndlessPatchedBytes, "無盡模式羅馬陣營") };
        }
        if (!enabled && state == ExeRomanEndlessPatchState.Patched) {
            return new[] { new ExeWriteOp(RomanEndlessPatchOffset, RomanEndlessPatchedBytes, RomanEndlessOriginalBytes, "無盡模式羅馬陣營還原") };
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanVillageRangeRestore(ExeVillageRangePatchState state) {
        if (state != ExeVillageRangePatchState.Expanded &&
            state != ExeVillageRangePatchState.LegacyLogicOnly) {
            return Array.Empty<ExeWriteOp>();
        }

        byte[] expectedFrameX = state == ExeVillageRangePatchState.Expanded ? VillageFrameXPatchedBytes : VillageFrameXOriginalBytes;
        byte[] expectedFrameZ = state == ExeVillageRangePatchState.Expanded ? VillageFrameZPatchedBytes : VillageFrameZOriginalBytes;
        return new[] {
            new ExeWriteOp(VillageRangeXPatchOffset, VillageRangeXPatchedBytes, VillageRangeXOriginalBytes, "舊版村落建造範圍 X 還原"),
            new ExeWriteOp(VillageRangeZPatchOffset, VillageRangeZPatchedBytes, VillageRangeZOriginalBytes, "舊版村落建造範圍 Z 還原"),
            new ExeWriteOp(VillageFrameXPatchOffset, expectedFrameX, VillageFrameXOriginalBytes, "舊版村落框架 X 還原"),
            new ExeWriteOp(VillageFrameZPatchOffset, expectedFrameZ, VillageFrameZOriginalBytes, "舊版村落框架 Z 還原"),
        };
    }

    public static IReadOnlyList<ExeWriteOp> PlanVillageSetter(bool enabled, ExeVillageSetterPatchState state) {
        if (enabled) {
            if (state == ExeVillageSetterPatchState.Original ||
                state == ExeVillageSetterPatchState.Legacy2x ||
                state == ExeVillageSetterPatchState.Legacy2Point5x ||
                state == ExeVillageSetterPatchState.Legacy3x ||
                state == ExeVillageSetterPatchState.Legacy5x) {
                byte[] expectedCave = state == ExeVillageSetterPatchState.Original ? VillageSetterCaveOriginalBytes
                    : state == ExeVillageSetterPatchState.Legacy2x ? VillageSetterCaveLegacy2xBytes
                    : state == ExeVillageSetterPatchState.Legacy2Point5x ? VillageSetterCaveLegacy2Point5xBytes
                    : state == ExeVillageSetterPatchState.Legacy3x ? VillageSetterCaveLegacy3xBytes
                    : VillageSetterCaveLegacy5xBytes;
                byte[] expectedHook = state == ExeVillageSetterPatchState.Original ? VillageSetterHookOriginalBytes : VillageSetterHookPatchedBytes;
                return new[] {
                    new ExeWriteOp(VillageSetterCaveOffset, expectedCave, VillageSetterCavePatchedBytes, "村落建造範圍程式碼洞"),
                    new ExeWriteOp(VillageSetterHookOffset, expectedHook, VillageSetterHookPatchedBytes, "村落建造範圍跳板"),
                };
            }
            return Array.Empty<ExeWriteOp>();
        }

        if (state == ExeVillageSetterPatchState.Legacy2x ||
            state == ExeVillageSetterPatchState.Legacy2Point5x ||
            state == ExeVillageSetterPatchState.Legacy3x ||
            state == ExeVillageSetterPatchState.Legacy5x ||
            state == ExeVillageSetterPatchState.EntireMap) {
            byte[] expectedCave = state == ExeVillageSetterPatchState.Legacy2x ? VillageSetterCaveLegacy2xBytes
                : state == ExeVillageSetterPatchState.Legacy2Point5x ? VillageSetterCaveLegacy2Point5xBytes
                : state == ExeVillageSetterPatchState.Legacy3x ? VillageSetterCaveLegacy3xBytes
                : state == ExeVillageSetterPatchState.Legacy5x ? VillageSetterCaveLegacy5xBytes
                : VillageSetterCavePatchedBytes;
            return new[] {
                new ExeWriteOp(VillageSetterHookOffset, VillageSetterHookPatchedBytes, VillageSetterHookOriginalBytes, "村落建造範圍跳板還原"),
                new ExeWriteOp(VillageSetterCaveOffset, expectedCave, VillageSetterCaveOriginalBytes, "村落建造範圍程式碼洞還原"),
            };
        }
        return Array.Empty<ExeWriteOp>();
    }

    /// <summary>逐一套用寫入計畫；每筆寫入前先驗證目前位元組，任一不符即丟例外中止。
    /// 注意：先前已套用的項目會留在緩衝區中——呼叫端捕捉到例外後不得再使用該緩衝區寫檔。</summary>
    public static void Apply(byte[] exeBytes, IEnumerable<ExeWriteOp> ops) {
        foreach (ExeWriteOp op in ops) {
            VerifiedBinaryWriter.WriteBytes(exeBytes, op.Offset, op.Expected, op.Replacement, op.PatchName);
        }
    }

    private static byte[] ReadSpan(byte[] source, long offset, int length) {
        byte[] buffer = new byte[length];
        Buffer.BlockCopy(source, (int)offset, buffer, 0, length);
        return buffer;
    }
}
