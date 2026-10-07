using System.Buffers.Binary;
using System.Numerics;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// 從原生 APT 二進位資料中讀取 extraA（光源點）與 extraB（特效/煙霧點）的內部輔助類別。
/// 依據 APAT v2/v3 規格與 native 0x4E5710 / 0x4C82A0 解析點陣列。
/// 不修改 NativeAptDocument.cs。
/// </summary>
internal static class NativeAptLightPoints
{
    public sealed record AptLightPoint(int ScreenX, int ScreenY, int DeltaAnchorX, int DeltaAnchorY);

    /// <summary>
    /// 從 APT 二進位位元組中提取 extraA 光源點清單（包含相對於 anchor 的螢幕位移）。
    /// </summary>
    public static IReadOnlyList<AptLightPoint> ExtractLightPoints(ReadOnlySpan<byte> aptBytes)
    {
        if (aptBytes.Length < 112)
            throw new InvalidDataException("APT source is too short for header.");

        uint magic = BinaryPrimitives.ReadUInt32LittleEndian(aptBytes[0..4]);
        if (magic != 0x54415041) // "APAT"
            throw new InvalidDataException("Missing APAT signature.");

        uint version = BinaryPrimitives.ReadUInt32LittleEndian(aptBytes[4..8]);
        if (version is < 2 or > 3)
            throw new NotSupportedException($"APT version {version} is not supported; expected 2..3.");

        uint groupVariants = BinaryPrimitives.ReadUInt32LittleEndian(aptBytes[36..40]); // header[9]
        uint tableOffset = BinaryPrimitives.ReadUInt32LittleEndian(aptBytes[12..16]); // header[3]

        // 跳過 rowOffsets (31 words = 124 bytes) 與 rowWidths (31 words = 124 bytes)
        int offset = (int)tableOffset + 248;
        if (offset + 24 > aptBytes.Length)
            throw new InvalidDataException("APT header offsets out of bounds.");

        // First anchor pair (8 bytes)
        offset += 8;

        int anchorX = BinaryPrimitives.ReadInt32LittleEndian(aptBytes[offset..(offset + 4)]);
        int anchorY = BinaryPrimitives.ReadInt32LittleEndian(aptBytes[(offset + 4)..(offset + 8)]);
        offset += 8;

        uint extraA = BinaryPrimitives.ReadUInt32LittleEndian(aptBytes[offset..(offset + 4)]);
        uint extraB = BinaryPrimitives.ReadUInt32LittleEndian(aptBytes[(offset + 4)..(offset + 8)]);
        offset += 8;

        int groups = unchecked((int)BinaryPrimitives.ReadUInt32LittleEndian(aptBytes[offset..(offset + 4)]));
        offset += 4;

        int perVariantCount = version == 3 ? (int)groupVariants : 1;
        int groupHeaderBytes = (2 + 2 + perVariantCount + 2) * 4;
        offset += groups * groupHeaderBytes;

        long requiredBytes = ((long)extraA + extraB) * 8;
        if (offset + requiredBytes > aptBytes.Length)
            throw new InvalidDataException("APT extra points extend beyond source length.");

        var points = new List<AptLightPoint>((int)extraA);
        for (int i = 0; i < extraA; i++)
        {
            int ptX = BinaryPrimitives.ReadInt32LittleEndian(aptBytes[offset..(offset + 4)]);
            int ptY = BinaryPrimitives.ReadInt32LittleEndian(aptBytes[(offset + 4)..(offset + 8)]);
            offset += 8;
            points.Add(new AptLightPoint(ptX, ptY, ptX - anchorX, ptY - anchorY));
        }

        return points;
    }

    /// <summary>
    /// 計算建築物所有 extraA 光源點在世界空間中的坐標清單。
    /// 給定建築物世界基準點 (buildingWorldX, buildingWorldZ)、地面高度 groundY 與 objdef 指定之 aptlh 高度偏移。
    /// 依 2:1 等角逆向公式：
    /// DeltaWorldX = DeltaScreenX + 2 * DeltaScreenY
    /// DeltaWorldZ = 2 * DeltaScreenY - DeltaScreenX
    /// </summary>
    public static IReadOnlyList<Vector3> GetBuildingWorldLightPositions(
        ReadOnlySpan<byte> aptBytes,
        float buildingWorldX,
        float buildingWorldZ,
        float groundY,
        float aptHeightOffset)
    {
        var lightPoints = ExtractLightPoints(aptBytes);
        var worldPositions = new List<Vector3>(lightPoints.Count);

        float lightWorldY = groundY + aptHeightOffset;
        foreach (var pt in lightPoints)
        {
            Vector3 worldDelta = NativeLightCalculator.IsometricAptOffsetToWorldDelta(pt.DeltaAnchorX, pt.DeltaAnchorY, 0f);
            worldPositions.Add(new Vector3(
                buildingWorldX + worldDelta.X,
                lightWorldY,
                buildingWorldZ + worldDelta.Z));
        }

        return worldPositions;
    }
}
