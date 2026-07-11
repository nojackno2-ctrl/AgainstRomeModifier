using System.Globalization;

namespace AgainstRomeModifier.Core.Patches;

/// <summary>
/// cl_epara.ini 補丁：拋射物瞄準參數。
/// [ProjectileVarianceOnMove] 是對移動目標預判提前量的隨機散布倍率（0.5 = 最多 0.5×3×移動速度的偏差）；
/// 遠程命中強化將其設為 0.0，預判完全精準。詳見 docs/reverse-engineering/projectile-ballistics.md。
/// </summary>
public static class EparaPatcher {
    internal const string VarianceSection = "[ProjectileVarianceOnMove]";
    internal const string AccuracyVarianceValue = "0.0";

    public static byte[] GetPatchedBytes(byte[] original, bool rangedAccuracy) {
        ArgumentNullException.ThrowIfNull(original);
        if (!rangedAccuracy) return original;

        var (text, lineEnding, lines) = PatchText.Read(original);
        bool patched = false;
        for (int i = 0; i < lines.Length && !patched; i++) {
            if (!lines[i].Trim().Equals(VarianceSection, StringComparison.OrdinalIgnoreCase)) continue;
            for (int j = i + 1; j < lines.Length; j++) {
                string trimmed = lines[j].Trim();
                if (trimmed.Length == 0 || trimmed.StartsWith(";", StringComparison.Ordinal)) continue;
                if (trimmed.StartsWith("[", StringComparison.Ordinal)) break; // 區段內沒有值行
                if (!double.TryParse(trimmed, NumberStyles.Any, CultureInfo.InvariantCulture, out _))
                    throw new InvalidDataException($"cl_epara.ini {VarianceSection} 的值行無法解析: '{trimmed}'，已取消整次套用。");
                lines[j] = lines[j].Replace(trimmed, AccuracyVarianceValue);
                patched = true;
                break;
            }
        }
        if (!patched)
            throw new InvalidDataException($"cl_epara.ini 找不到 {VarianceSection} 區段的值行，已取消整次套用。");
        return PatchText.Write(text, lineEnding, lines, original);
    }
}
