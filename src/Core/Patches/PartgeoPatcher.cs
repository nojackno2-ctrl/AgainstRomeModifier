using System.Globalization;

namespace AgainstRomeModifier.Core.Patches;

public sealed record PartgeoOptions(bool ProjectileArcHeight);

/// <summary>
/// partgeo.dau ([ParticleGeometrieDefault]) 補丁：拋射物粒子的 ysub 欄（每秒下墜加速度，16.16 定點整數）。
/// 拋射物飛行由發射端（objdef w*_emit 垂直初速）與此處 ysub 重力構成拋物線；
/// 兩者以相同倍率放大時弧頂增高 (≈ emit²/(2·ysub))、飛行時間與落點不變。
/// 詳見 docs/reverse-engineering/projectile-ballistics.md。
/// </summary>
public static class PartgeoPatcher {
    /// <summary>武器拋射物使用的 partgeo 條目（由 objdef Par* 拋射物物件的 pageo 欄反查而得），
    /// 偵測邏輯（FeatureDetector）共用同一份清單。</summary>
    internal static readonly string[] ProjectileGeoNames = { "Wurfspeer00", "Wurfaxt00", "Katapultstein00", "Katapultstein01", "Pfeil00", "Spiess00", "Fackel" };
    /// <summary>與 ObjdefPatcher.ArcEmitMultiplier 必須一致（同倍率縮放才能維持落點）。</summary>
    internal const double ArcYsubMultiplier = ObjdefPatcher.ArcEmitMultiplier;
    /// <summary>partgeo 資料列的欄位索引：0=idx, 1=activ, 2=name, ..., 12=ysub。</summary>
    internal const int YsubColumn = 12;

    public static byte[] GetPatchedBytes(byte[] original, PartgeoOptions options) {
        ArgumentNullException.ThrowIfNull(original);
        ArgumentNullException.ThrowIfNull(options);
        if (!options.ProjectileArcHeight) return original;

        var (text, lineEnding, lines) = PatchText.Read(original);
        int patchedCount = 0;
        for (int row = 0; row < lines.Length; row++) {
            string[] cols = PatchText.ParseCsvLine(lines[row]);
            if (cols.Length <= YsubColumn) continue;
            string name = cols[2].Trim();
            if (!ProjectileGeoNames.Contains(name, StringComparer.Ordinal)) continue;
            if (!long.TryParse(cols[YsubColumn].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out long ysub) || ysub <= 0)
                throw new InvalidDataException($"partgeo.dau 條目 {name} 的 ysub 值無法解析或非正值，已取消整次套用。");
            string newValue = ((long)Math.Round(ysub * ArcYsubMultiplier)).ToString(CultureInfo.InvariantCulture);
            int width = cols[YsubColumn].Length;
            if (newValue.Length > width)
                throw new InvalidDataException($"partgeo.dau 條目 {name} 的 ysub 新值 {newValue} 超出欄位寬度 {width}，已取消整次套用。");
            cols[YsubColumn] = newValue.PadLeft(width);
            lines[row] = PatchText.ToCsvString(cols);
            patchedCount++;
        }
        if (patchedCount != ProjectileGeoNames.Length)
            throw new InvalidDataException($"partgeo.dau 拋射物條目數不符：預期 {ProjectileGeoNames.Length} 筆、實得 {patchedCount} 筆，已取消整次套用。");

        byte[] result = PatchText.Write(text, lineEnding, lines, original);
        return result;
    }
}
