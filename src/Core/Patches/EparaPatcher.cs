using System.Globalization;

namespace AgainstRomeModifier.Core.Patches;

/// <summary>
/// cl_epara.ini 補丁：拋射物瞄準參數。
/// [ProjectileVarianceOnMove] 是對移動目標預判提前量的隨機散布倍率（0.5 = 最多 0.5×3×移動速度的偏差）；
/// 遠程命中強化將其設為 0.0，預判完全精準。
/// [ProjectileInitSpeedFactor]（ISF）是拋射物水平初速倍率，水平抵達時間為常數 4/ISF 秒（原版 1.5 → 2.67 秒）。
/// 拋射物的垂直飛行時間為 2×emit/ysub（弓箭約 2.47 秒），與水平抵達時間無關，因此每箭都固定落在目標距離的
/// T_fall/T_arrival ≈ 92.5%，形成「射短 ≈ 7.5%×距離」的系統性偏差。原版射程下 7.5% 尚在傷害半徑內，
/// 但開啟三倍射程後射短距離同步三倍、遠超傷害半徑，弓箭因此「根本射不到」站著不動的敵人。
/// RangedRange3x 啟用時將 ISF 乘以 <see cref="InitSpeedReachMultiplier"/>（1.5 → 1.62），使水平抵達時間
/// 4/ISF ≈ 2.47 秒對齊垂直飛行時間，射短比例趨近 0，拋射物即可覆蓋放大後的射程。
/// 詳見 docs/reverse-engineering/projectile-ballistics.md。
/// </summary>
public static class EparaPatcher {
    internal const string VarianceSection = "[ProjectileVarianceOnMove]";
    internal const string AccuracyVarianceValue = "0.0";
    internal const string InitSpeedSection = "[ProjectileInitSpeedFactor]";
    /// <summary>三倍射程時的 ISF 放大倍率。1.5×1.08=1.62；由 4/ISF = 2×emit/ysub（弓箭 2×110/89≈2.47 秒）解得
    /// ISF≈1.618，取 1.08 使射短比例趨近 0。與 ArcEmitMultiplier 相同屬靜態逆向推導，正式倍率仍待實機校準。</summary>
    internal const double InitSpeedReachMultiplier = 1.08;

    public static byte[] GetPatchedBytes(byte[] original, bool rangedAccuracy, bool rangedRange3x) {
        ArgumentNullException.ThrowIfNull(original);
        if (!rangedAccuracy && !rangedRange3x) return original;

        var (text, lineEnding, lines) = PatchText.Read(original);
        if (rangedAccuracy)
            PatchSectionValue(lines, VarianceSection, _ => AccuracyVarianceValue);
        if (rangedRange3x)
            PatchSectionValue(lines, InitSpeedSection,
                value => (value * InitSpeedReachMultiplier).ToString("0.###", CultureInfo.InvariantCulture));
        return PatchText.Write(text, lineEnding, lines, original);
    }

    /// <summary>找到區段後的第一個值行，以 <paramref name="transform"/> 的結果取代原值（沿用原行的前後文字）。
    /// 找不到區段或值行、或值行無法解析為數字時擲出，中止整次套用。</summary>
    private static void PatchSectionValue(string[] lines, string section, Func<double, string> transform) {
        for (int i = 0; i < lines.Length; i++) {
            if (!lines[i].Trim().Equals(section, StringComparison.OrdinalIgnoreCase)) continue;
            for (int j = i + 1; j < lines.Length; j++) {
                string trimmed = lines[j].Trim();
                if (trimmed.Length == 0 || trimmed.StartsWith(";", StringComparison.Ordinal)) continue;
                if (trimmed.StartsWith("[", StringComparison.Ordinal)) break; // 區段內沒有值行
                if (!double.TryParse(trimmed, NumberStyles.Any, CultureInfo.InvariantCulture, out double value))
                    throw new InvalidDataException($"cl_epara.ini {section} 的值行無法解析: '{trimmed}'，已取消整次套用。");
                lines[j] = lines[j].Replace(trimmed, transform(value));
                return;
            }
        }
        throw new InvalidDataException($"cl_epara.ini 找不到 {section} 區段的值行，已取消整次套用。");
    }
}
