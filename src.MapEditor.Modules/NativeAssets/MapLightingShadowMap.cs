using System.Buffers.Binary;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>shadows.dat 解壓後布局；128 張 256×256、LSB-first 的逐時段地形遮蔽 bitplane。</summary>
internal sealed class MapLightingShadowMap
{
    private readonly byte[] _bits;
    private MapLightingShadowMap(uint[] header, byte[] bits)
    {
        Header = Array.AsReadOnly(header); _bits = bits;
    }

    public IReadOnlyList<uint> Header { get; }
    public const int Dimension = 256;
    public const int SliceCount = 128;
    public const int DecodedLength = 32 + SliceCount * Dimension * Dimension / 8;

    /// <summary>傳入已解壓 bytes；刻意不使用既有會容忍截斷的 PFIL decoder。</summary>
    public static MapLightingShadowMap ParseDecoded(ReadOnlySpan<byte> source)
    {
        if (source.Length != DecodedLength) throw new InvalidDataException("Invalid decoded shadows.dat extent.");
        uint[] header = new uint[8];
        for (int i = 0; i < header.Length; i++) header[i] = BinaryPrimitives.ReadUInt32LittleEndian(source[(i * 4)..]);
        if (header[0] != Dimension || header[1] != Dimension || header[2] != SliceCount)
            throw new NotSupportedException("Only 256x256, 128-slice shadow meshes are supported.");
        // 後五欄是 cache 設定／height sum，保留原值，不冒充版本或像素色值。
        return new MapLightingShadowMap(header, source[32..].ToArray());
    }

    public bool IsOccluded(int slice, int x, int z)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(slice);
        ArgumentOutOfRangeException.ThrowIfGreaterThanOrEqual(slice, SliceCount);
        ArgumentOutOfRangeException.ThrowIfNegative(x);
        ArgumentOutOfRangeException.ThrowIfGreaterThanOrEqual(x, Dimension);
        ArgumentOutOfRangeException.ThrowIfNegative(z);
        ArgumentOutOfRangeException.ThrowIfGreaterThanOrEqual(z, Dimension);
        return (_bits[slice * 8192 + z * 32 + (x >> 3)] & (1 << (x & 7))) != 0;
    }

    /// <summary>0x49A780 的整數時段插值：一天 86400 秒，每 slice 675 秒。</summary>
    public int OcclusionAt(int secondsOfDay, int x, int z)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(secondsOfDay);
        ArgumentOutOfRangeException.ThrowIfGreaterThanOrEqual(secondsOfDay, 86400);
        int slice = secondsOfDay * SliceCount / 86400;
        int weight = (secondsOfDay % 675) * 256 / 675;
        return ((IsOccluded(slice, x, z) ? 255 : 0) * (256 - weight)
            + (IsOccluded((slice + 1) % SliceCount, x, z) ? 255 : 0) * weight) >> 8;
    }

    /// <summary>0x49A620 的 fixed-point 空間取樣；world X/Z 每 64 單位一格。</summary>
    public int SampleOcclusion(int secondsOfDay, int worldX, int worldZ)
    {
        // Native 32-bit shifts 會溢位；此介面僅接受安全且與地圖相關的座標。
        ArgumentOutOfRangeException.ThrowIfLessThan(worldX, -32768);
        ArgumentOutOfRangeException.ThrowIfGreaterThan(worldX, 32767);
        ArgumentOutOfRangeException.ThrowIfLessThan(worldZ, -32768);
        ArgumentOutOfRangeException.ThrowIfGreaterThan(worldZ, 32767);
        int fx = worldX << 10, fz = worldZ << 10;
        int x0 = Math.Clamp(fx >> 16, 0, 255), x1 = Math.Min(x0 + 1, 255);
        int z0 = Math.Clamp(fz >> 16, 0, 255), z1 = Math.Min(z0 + 1, 255);
        int wx = fx & 65535, wz = fz & 65535;
        int left = (OcclusionAt(secondsOfDay, x0, z0) * (65535 - wz) + OcclusionAt(secondsOfDay, x0, z1) * wz) >> 9;
        int right = (OcclusionAt(secondsOfDay, x1, z0) * (65535 - wz) + OcclusionAt(secondsOfDay, x1, z1) * wz) >> 9;
        return (left * (65535 - wx) + right * wx) >> 23;
    }

    public float ShadowFactor(int secondsOfDay, int worldX, int worldZ, float strength)
    {
        if (!float.IsFinite(strength) || strength is < 0 or > 1) throw new ArgumentOutOfRangeException(nameof(strength));
        return 1 - SampleOcclusion(secondsOfDay, worldX, worldZ) * (1f / 256) * strength;
    }
}
