using System.Globalization;

namespace AgainstRomeModifier.Core.Patches;

public sealed record ObjdefOptions(
    bool Balance,
    bool HousingCapacity20x,
    bool StorageCapacity10x,
    bool FastBuildUpgradeRepair,
    IReadOnlyDictionary<string, double[]> LeaderGlory,
    IReadOnlyDictionary<string, double[]> UnitStats);

public static class ObjdefPatcher {
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
            if (options.LeaderGlory.TryGetValue(name, out double[]? glory)) {
                int[] indices = { 148, 149, 150, 161, 162, 153 };
                for (int i = 0; i < indices.Length && i < glory.Length; i++) SetValueIfFits(cols, indices[i], glory[i].ToString(CultureInfo.InvariantCulture));
            }
            if (options.HousingCapacity20x) MultiplyOriginalInt(cols, source, (int)ObjdefIndex.HousingCapacity, 20, name, "housing capacity");
            if (options.StorageCapacity10x && name.StartsWith("Bau", StringComparison.Ordinal) && (name.Contains("Hau", StringComparison.Ordinal) || name.Contains("Lag", StringComparison.Ordinal))) MultiplyOriginalInt(cols, source, (int)ObjdefIndex.StorageCapacity, 10, name, "storage capacity");
            if (options.FastBuildUpgradeRepair && name.StartsWith("Bau", StringComparison.Ordinal)) {
                DivideOriginalInt(cols, source, 73, 10, name, "建造時間");
                DivideOriginalInt(cols, source, 74, 10, name, "升級時間");
            }
            if (TroopConfig.UnitMeta.TryGetValue(name, out var meta) && options.UnitStats.TryGetValue(name, out double[]? stats) && stats.Length >= 8) PatchUnit(cols, source, name, meta.Item3, stats);
            else if (name is "FigZivMan00_Zivilist" or "FigZivWei00_Zivilistin" or "FigTiePac00_Packpferd") PatchCivilianSpeed(cols, source, name, options.Balance ? 2.0 : 1.0);
            lines[row] = PatchText.ToCsvString(cols);
        }
        string newText = string.Join(lineEnding, lines);
        if (text.EndsWith(lineEnding, StringComparison.Ordinal) && !newText.EndsWith(lineEnding, StringComparison.Ordinal)) newText += lineEnding;
        if (newText.Length != text.Length) throw new InvalidDataException($"objdef.dau 長度不匹配！原始長度: {text.Length}, 修改後長度: {newText.Length}");
        return GameLZSS.CompressPfil(PatchText.GameEncoding.GetBytes(newText), original);
    }

    private static void PatchUnit(string[] cols, string[] source, string name, string type, double[] stats) {
        for (int i = 0; i < source.Length; i++) source[i] = source[i].Trim();
        double moves = Read(source, (int)ObjdefIndex.Moves), movsf = Read(source, (int)ObjdefIndex.Movsf), bmovs = Read(source, (int)ObjdefIndex.Bmovs);
        GetDamage(source, type, out double meleeDamage, out double rangedDamage);
        double primaryDamage = type is "ranged_inf" or "ranged_cav" ? rangedDamage : meleeDamage;
        if (type == "siege") primaryDamage = Math.Max(meleeDamage, rangedDamage);
        double originalRange = GetMaxRange(source);
        GetReload(source, type, out double meleeReload, out double rangedReload);
        double primaryReload = type is "ranged_inf" or "ranged_cav" ? rangedReload : type == "siege" ? Math.Max(meleeReload, rangedReload) : meleeReload;
        double speedScale = moves > 0 ? stats[4] / (moves * 2.0) : 1.0;
        double rangeScale = originalRange > 0 ? stats[7] / originalRange : 1.0;
        double reloadScale = primaryReload > 0 ? stats[6] / primaryReload : 1.0;
        double damageScale = primaryDamage > 0 ? stats[1] / primaryDamage : 1.0;
        if (moves > 0) SetValue(cols, (int)ObjdefIndex.Moves, (moves * speedScale).ToString("F2", CultureInfo.InvariantCulture), name, "移動速度");
        if (movsf > 0) SetValue(cols, (int)ObjdefIndex.Movsf, (movsf * speedScale).ToString("F2", CultureInfo.InvariantCulture), name, "移動速度");
        if (bmovs > 0) SetValue(cols, (int)ObjdefIndex.Bmovs, (bmovs * speedScale).ToString("F2", CultureInfo.InvariantCulture), name, "移動速度");
        SetValue(cols, (int)ObjdefIndex.Sirad, ((int)stats[5]).ToString(CultureInfo.InvariantCulture), name, "視野");
        SetValue(cols, (int)ObjdefIndex.Hp, ((int)stats[0]).ToString(CultureInfo.InvariantCulture), name, "生命值");
        SetValue(cols, (int)ObjdefIndex.Aw, ((int)stats[3]).ToString(CultureInfo.InvariantCulture), name, "戰鬥");
        SetValue(cols, (int)ObjdefIndex.Vw, ((int)stats[2]).ToString(CultureInfo.InvariantCulture), name, "防禦");
        for (int w = 1; w <= 8; w++) {
            // 射程欄位是 active+2 (RangeMin, w*_rad1) 與 active+3 (RangeMax, w*_rad2)；
            // active+4 是 Weapon*Angle（角度），依 objdef-fields.csv 絕不可當射程縮放。
            int active = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8, damage = active + 1, min = active + 2, max = active + 3, reload = active + 6;
            if (max >= source.Length || source[active] != "1") continue;
            foreach (int index in new[] { min, max }) if (Read(source, index) is double range && range > 0) SetValue(cols, index, (range * rangeScale).ToString("F2", CultureInfo.InvariantCulture), name, "射程");
            double weaponDamage = Read(source, damage);
            double newDamage = type is "ranged_inf" or "ranged_cav" && w == 1 ? weaponDamage : weaponDamage * damageScale;
            SetValue(cols, damage, newDamage.ToString("F2", CultureInfo.InvariantCulture), name, "傷害");
            SetValue(cols, reload, ((int)Math.Round(Read(source, reload) * reloadScale)).ToString(CultureInfo.InvariantCulture), name, "攻擊冷卻");
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
    private static double GetMaxRange(string[] cols) { double result = 0; for (int w = 1; w <= 8; w++) { int active = (int)ObjdefIndex.Weapon1Akti + (w - 1) * 8, min = active + 2, max = active + 3; if (max >= cols.Length || cols[active] != "1") continue; result = Math.Max(result, Math.Max(Read(cols, min), Read(cols, max))); } return result; }
}
