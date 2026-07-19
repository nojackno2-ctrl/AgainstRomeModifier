using System.Globalization;

namespace AgainstRomeModifier.Core.Patches;

public sealed record ObjdefOptions(
    bool Balance,
    bool HousingCapacity20x,
    bool StorageCapacity10x,
    bool FastBuildUpgradeRepair,
    bool HqHp10x,
    bool LeaderGlory5x,
    bool RangedRange3x,
    bool UnitMovementSpeed2x,
    bool SpellEntireMap,
    bool SpellRange3x,
    bool ProjectileArcHeight,
    IReadOnlyDictionary<string, double[]> UnitStats,
    bool AllUnitsEntireMapVision = false,
    bool VillagerMovementSpeed5x = false,
    bool NoRunHpLoss = false);

public static class ObjdefPatcher {
    /// <summary>LeaderGlory5x 影響的首領列與欄位（攻擊成長、防禦成長、傷害成長、士氣光環），偵測邏輯（FeatureDetector）共用同一份清單。</summary>
    internal static readonly string[] GloryLeaders = { "FigRomAnf00_Anfuehrer", "FigGerAnf00_Anfuehrer", "FigKelAnf00_Anfuehrer", "FigHunAnf00_Anfuehrer" };
    internal static readonly int[] GloryColumns = { 148, 149, 150, 161 };
    internal const int GloryMultiplier = 5;

    /// <summary>拋射彈道增高倍率：w*_emit（拋射垂直初速，16.16 定點整數）乘以此倍率；
    /// 必須與 PartgeoPatcher.ArcYsubMultiplier（重力 ysub）同倍率，落點與飛行時間才不會改變，弧頂高度按同倍率增加。
    /// 機制已於 2026-07-12 以 10 倍實機驗證成功，正式版依使用者要求採 2 倍。
    /// 原版所有拋射 emit 值 ×10 以內皆不超出欄位寬度（已對全 objdef 驗證）。</summary>
    internal const double ArcEmitMultiplier = 2.0;
    /// <summary>遠程命中修正倍率：w*_drad（落點傷害半徑）乘以此倍率，讓近失彈也算命中；
    /// 已整合為射程 3 倍（RangedRange3x）的一部分，用來修正拉遠射程後打不中的問題。</summary>
    internal const int AccuracyDradMultiplier = 2;
    internal const int EntireMapSight = 30000;

    internal static bool SupportsEntireMapVision(string name) =>
        TroopConfig.UnitMeta.ContainsKey(name) ||
        name is "FigZivMan00_Zivilist" or "FigZivWei00_Zivilistin" or "FigTiePac00_Packpferd";

    public static byte[] GetPatchedBytes(byte[] original, ObjdefOptions options) {
        ArgumentNullException.ThrowIfNull(original);
        ArgumentNullException.ThrowIfNull(options);
        var (text, lineEnding, lines) = PatchText.Read(original);
        string[] originalLines = (string[])lines.Clone();
        for (int row = 2; row < lines.Length; row++) {
            if (lines[row].Length < 100) continue;
            string[] cols = PatchText.ParseCsvLine(lines[row]);
            if (cols.Length < 192) continue;
            string[] source = PatchText.ParseCsvLine(originalLines[row]);
            string name = cols[52].Trim();
            if (options.LeaderGlory5x && GloryLeaders.Contains(name)) {
                foreach (int index in GloryColumns) {
                    double value = Read(source, index);
                    if (value > 0) SetValueIfFits(cols, index, (value * GloryMultiplier).ToString(CultureInfo.InvariantCulture));
                }
            }
            if (options.HousingCapacity20x) MultiplyOriginalInt(cols, source, (int)ObjdefIndex.HousingCapacity, 20, name, "housing capacity");
            if (options.StorageCapacity10x && name.StartsWith("Bau", StringComparison.Ordinal) && (name.Contains("Hau", StringComparison.Ordinal) || name.Contains("Lag", StringComparison.Ordinal))) MultiplyOriginalInt(cols, source, (int)ObjdefIndex.StorageCapacity, 10, name, "storage capacity");
            if (options.HqHp10x && name.StartsWith("Bau", StringComparison.Ordinal) && name.Contains("Hau", StringComparison.Ordinal)) MultiplyOriginalInt(cols, source, (int)ObjdefIndex.Hp, 10, name, "hq hp");
            if (options.FastBuildUpgradeRepair && name.StartsWith("Bau", StringComparison.Ordinal)) {
                DivideOriginalInt(cols, source, 73, 10, name, "建造時間");
                DivideOriginalInt(cols, source, 74, 10, name, "升級時間");
            }
            if (options.ProjectileArcHeight || options.RangedRange3x) PatchProjectileWeapons(cols, source, name, options);
            if (TroopConfig.UnitMeta.TryGetValue(name, out var meta) && options.UnitStats.TryGetValue(name, out double[]? stats) && stats.Length >= 8) PatchUnit(cols, source, name, meta.UnitType, stats, options);
            else if (name is "FigZivMan00_Zivilist" or "FigZivWei00_Zivilistin" or "FigTiePac00_Packpferd") {
                double civMult = 1.0;
                if (options.VillagerMovementSpeed5x) civMult *= 5.0;
                PatchCivilianSpeed(cols, source, name, civMult);
            }
            if (options.NoRunHpLoss && (TroopConfig.UnitMeta.ContainsKey(name) || name is "FigZivMan00_Zivilist" or "FigZivWei00_Zivilistin" or "FigTiePac00_Packpferd")) {
                double originalLpsub = Read(source, (int)ObjdefIndex.Lpsub);
                if (Math.Abs(originalLpsub - 1.00) < 0.01) {
                    SetValue(cols, (int)ObjdefIndex.Lpsub, "0.00", name, "lpsub");
                }
            }
            // Final shared-field override. RangedRange3x and SpellEntireMap are
            // composed first; when this feature is selected, it owns Sirad last.
            if (options.AllUnitsEntireMapVision && SupportsEntireMapVision(name)) {
                SetValue(cols, (int)ObjdefIndex.Sirad, EntireMapSight.ToString(CultureInfo.InvariantCulture), name, "視野");
            }
            lines[row] = PatchText.ToCsvString(cols);
        }
        string newText = string.Join(lineEnding, lines);
        if (text.EndsWith(lineEnding, StringComparison.Ordinal) && !newText.EndsWith(lineEnding, StringComparison.Ordinal)) newText += lineEnding;
        if (newText.Length != text.Length) throw new InvalidDataException($"objdef.dau 長度不匹配！原始長度: {text.Length}, 修改後長度: {newText.Length}");
        return GameLZSS.CompressPfil(PatchText.GameEncoding.GetBytes(newText), original);
    }

    private static void PatchUnit(string[] cols, string[] source, string name, string type, double[] stats, ObjdefOptions options) {
        for (int i = 0; i < source.Length; i++) source[i] = source[i].Trim();
        double moves = Read(source, (int)ObjdefIndex.Moves), movsf = Read(source, (int)ObjdefIndex.Movsf), bmovs = Read(source, (int)ObjdefIndex.Bmovs);
        GetDamage(source, type, out double meleeDamage, out double rangedDamage);
        double primaryDamage = type is "ranged_inf" or "ranged_cav" ? rangedDamage : meleeDamage;
        if (type == "siege") primaryDamage = Math.Max(meleeDamage, rangedDamage);
        double originalRange = GetMaxRange(source, type);
        GetReload(source, type, out double meleeReload, out double rangedReload);
        double primaryReload = type is "ranged_inf" or "ranged_cav" ? rangedReload : type == "siege" ? Math.Max(meleeReload, rangedReload) : meleeReload;
        double speedScale = options.UnitMovementSpeed2x ? 2.0 : 1.0;
        double rangeScale = originalRange > 0 ? stats[7] / originalRange : 1.0;
        bool supportsRangedRange3x = TroopConfig.SupportsRangedRange3x(type);
        if (supportsRangedRange3x && options.RangedRange3x) {
            rangeScale *= 3.0;
        }
        bool isPriest = type == "priest";
        double reloadScale = primaryReload > 0 ? stats[6] / primaryReload : 1.0;
        double damageScale = primaryDamage > 0 ? stats[1] / primaryDamage : 1.0;

        // 僅在有實際數值變動時才寫入檔案，以保持原始格式防範偵測誤判
        if (moves > 0 && Math.Abs(speedScale - 1.0) > 0.001) {
            SetValue(cols, (int)ObjdefIndex.Moves, (moves * speedScale).ToString("F2", CultureInfo.InvariantCulture), name, "移動速度");
            if (movsf > 0) SetValue(cols, (int)ObjdefIndex.Movsf, (movsf * speedScale).ToString("F2", CultureInfo.InvariantCulture), name, "移動速度");
            if (bmovs > 0) SetValue(cols, (int)ObjdefIndex.Bmovs, (bmovs * speedScale).ToString("F2", CultureInfo.InvariantCulture), name, "移動速度");
        }

        // 遠程攻擊的目標取得仍受 Sirad（視野）限制。僅擴大 w*_rad1/w*_rad2
        // 會讓單位無法自行鎖定新射程外的目標，因此射程 3 倍時必須同步保證
        // 視野至少涵蓋新的最遠武器射程。
        double sight = options.SpellEntireMap && isPriest ? EntireMapSight : stats[5];
        if (supportsRangedRange3x && options.RangedRange3x) {
            sight = Math.Max(sight, originalRange * rangeScale);
        }
        if (Math.Abs(sight - Read(source, (int)ObjdefIndex.Sirad)) > 0.01) {
            SetValue(cols, (int)ObjdefIndex.Sirad, ((int)sight).ToString(CultureInfo.InvariantCulture), name, "視野");
        }
        if (Math.Abs(stats[0] - Read(source, (int)ObjdefIndex.Hp)) > 0.01) {
            SetValue(cols, (int)ObjdefIndex.Hp, ((int)stats[0]).ToString(CultureInfo.InvariantCulture), name, "生命值");
        }
        if (Math.Abs(stats[3] - Read(source, (int)ObjdefIndex.Aw)) > 0.01) {
            SetValue(cols, (int)ObjdefIndex.Aw, ((int)stats[3]).ToString(CultureInfo.InvariantCulture), name, "戰鬥");
        }
        if (Math.Abs(stats[2] - Read(source, (int)ObjdefIndex.Vw)) > 0.01) {
            SetValue(cols, (int)ObjdefIndex.Vw, ((int)stats[2]).ToString(CultureInfo.InvariantCulture), name, "防禦");
        }

        for (int w = 1; w <= 8; w++) {
            // 射程欄位是 active+2 (RangeMin, w*_rad1) 與 active+3 (RangeMax, w*_rad2)；
            // active+4 是 Weapon*Angle（角度），依 objdef-fields.csv 絕不可當射程縮放。
            int active = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8, damage = active + 1, max = active + 3, reload = active + 6;
            if (max >= source.Length || source[active] != "1") continue;
            if (isPriest) {
                double priestWeaponDamage = Read(source, damage);
                double newPriestDmg = priestWeaponDamage * damageScale;
                if (Math.Abs(newPriestDmg - priestWeaponDamage) > 0.01) {
                    SetValue(cols, damage, newPriestDmg.ToString("F2", CultureInfo.InvariantCulture), name, "damage");
                }
                double origPriestReload = Read(source, reload);
                double newPriestReload = Math.Round(origPriestReload * reloadScale);
                if (Math.Abs(newPriestReload - origPriestReload) > 0.01) {
                    SetValue(cols, reload, ((int)newPriestReload).ToString(CultureInfo.InvariantCulture), name, "reload");
                }
                continue;
            }

            // 只縮放遠程武器（dtyp 1~4；攻城武器全部），近戰副武器與 dtyp 5/6 特殊武器
            // 若跟著縮放，弓兵會從三倍距離外揮刀。rad1（最小射程）放大會等比擴大
            // 近身死區使單位在近距離無法開火，因此僅放大 rad2（最大射程）。
            int damageTypeIndex = (int)ObjdefIndex.Weapon1Dtyp + (w - 1);
            bool isRangedWeapon = type == "siege" || (damageTypeIndex < source.Length && source[damageTypeIndex] is "1" or "2" or "3" or "4");
            if (isRangedWeapon && Math.Abs(rangeScale - 1.0) > 0.001 && Read(source, max) is double range && range > 0) {
                SetValue(cols, max, (range * rangeScale).ToString("F2", CultureInfo.InvariantCulture), name, "射程");
            }

            double weaponDamage = Read(source, damage);
            double newDamage = type is "ranged_inf" or "ranged_cav" && w == 1 ? weaponDamage : weaponDamage * damageScale;
            if (Math.Abs(newDamage - weaponDamage) > 0.01) {
                SetValue(cols, damage, newDamage.ToString("F2", CultureInfo.InvariantCulture), name, "傷害");
            }

            double origReload = Read(source, reload);
            double newReload = Math.Round(origReload * reloadScale);
            if (Math.Abs(newReload - origReload) > 0.01) {
                SetValue(cols, reload, ((int)newReload).ToString(CultureInfo.InvariantCulture), name, "攻擊冷卻");
            }
        }
    }

    /// <summary>拋射武器判定：啟用 (akti=1) 且 w*_emit &gt; 0（垂直初速只有拋射物才有值）。
    /// ProjectileArcHeight 將 emit 乘 ArcEmitMultiplier（重力由 PartgeoPatcher 同倍率放大，弧頂增高、落點不變）；
    /// RangedRange3x 將 w*_drad（落點傷害半徑）乘 AccuracyDradMultiplier（命中修正，已整合進射程 3 倍）。</summary>
    private static void PatchProjectileWeapons(string[] cols, string[] source, string name, ObjdefOptions options) {
        for (int w = 1; w <= 8; w++) {
            int active = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8;
            int emit = (int)ObjdefIndex.Weapon1Emit + (w - 1) * 8;
            int drad = (int)ObjdefIndex.Weapon1Drad + (w - 1) * 2;
            if (emit >= source.Length || source[active].Trim() != "1") continue;
            if (!long.TryParse(source[emit].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out long emitValue) || emitValue <= 0) continue;
            if (options.ProjectileArcHeight) SetValue(cols, emit, ((long)Math.Round(emitValue * ArcEmitMultiplier)).ToString(CultureInfo.InvariantCulture), name, "拋射垂直初速");
            if (options.RangedRange3x && drad < source.Length && int.TryParse(source[drad].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int dradValue) && dradValue > 0)
                SetValue(cols, drad, checked(dradValue * AccuracyDradMultiplier).ToString(CultureInfo.InvariantCulture), name, "落點傷害半徑");
        }
    }

    private static void PatchCivilianSpeed(string[] cols, string[] source, string name, double multiplier) { foreach (int index in new[] { (int)ObjdefIndex.Moves, (int)ObjdefIndex.Movsf, (int)ObjdefIndex.Bmovs }) { double value = Read(source, index); if (value > 0) SetValue(cols, index, (value * multiplier).ToString("F2", CultureInfo.InvariantCulture), name, "移動速度"); } }
    private static void MultiplyOriginalInt(string[] cols, string[] source, int index, int multiplier, string name, string label) { if (index < cols.Length && index < source.Length && int.TryParse(source[index].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int value) && value > 0) SetValue(cols, index, checked(value * multiplier).ToString(CultureInfo.InvariantCulture), name, label); }
    private static void DivideOriginalInt(string[] cols, string[] source, int index, int divisor, string name, string label) { if (index < cols.Length && index < source.Length && int.TryParse(source[index].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int value) && value > 0) SetValueIfFits(cols, index, Math.Max(1, value / divisor).ToString(CultureInfo.InvariantCulture)); }
    private static void SetValueIfFits(string[] cols, int index, string value) { int length = cols[index].Length; if (PatchText.CheckLength(value, length, out string final)) cols[index] = final.PadLeft(length); }
    private static void SetValue(string[] cols, int index, string value, string name, string label) { int length = cols[index].Length; if (!PatchText.CheckLength(value, length, out string final)) throw new InvalidDataException($"單位 {name} 的 {label} 數值 {value} 超出 objdef.dau 欄位長度 {length}；已取消整次套用。"); cols[index] = final.PadLeft(length); }
    private static double Read(string[] cols, int index) => index < cols.Length && double.TryParse(cols[index].Trim(), NumberStyles.Any, CultureInfo.InvariantCulture, out double value) ? value : 0;
    private static void GetDamage(string[] cols, string type, out double melee, out double ranged) { melee = ranged = 0; for (int w = 1; w <= 8; w++) { int active = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8, damage = active + 1, damageType = (int)ObjdefIndex.Weapon1Dtyp + (w - 1); if (damageType >= cols.Length || cols[active] != "1") continue; double value = Read(cols, damage); if (damageType < cols.Length && (cols[damageType] is "1" or "2" or "3" or "4" || type == "siege")) ranged = Math.Max(ranged, value); else melee = Math.Max(melee, value); } }
    private static void GetReload(string[] cols, string type, out double melee, out double ranged) { melee = ranged = 0; for (int w = 1; w <= 8; w++) { int active = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8, reload = active + 6, damageType = (int)ObjdefIndex.Weapon1Dtyp + (w - 1); if (damageType >= cols.Length || cols[active] != "1") continue; double value = Read(cols, reload); bool isRanged = cols[damageType] is "1" or "2" or "3" or "4" || type == "siege"; if (isRanged) { if (value > 0 && (ranged == 0 || value < ranged)) ranged = value; } else if (value > 0 && (melee == 0 || value < melee)) melee = value; } }
    private static double GetMaxRange(string[] cols, string type = "") {
        if (type == "priest") return Read(cols, (int)ObjdefIndex.Sirad);
        double result = 0;
        for (int w = 1; w <= 8; w++) { int active = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8, min = active + 2, max = active + 3; if (max >= cols.Length || cols[active] != "1") continue; result = Math.Max(result, Math.Max(Read(cols, min), Read(cols, max))); }
        return result;
    }
}
