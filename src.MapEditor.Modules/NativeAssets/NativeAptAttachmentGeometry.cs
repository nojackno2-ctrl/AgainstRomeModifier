using System.Numerics;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>Static default projection (626928=1), not all runtime camera matrices.</summary>
internal static class NativeAptAttachmentGeometry
{
    public static NativeAptPoint Project(NativeAptDocument document, NativeAptPoint point)
    {
        // 4e5420: subtract the first anchor; integer floor division on each absolute point.
        int x = unchecked(point.X - document.ProjectionOrigin.X), y = unchecked(point.Y - document.ProjectionOrigin.Y);
        return new(unchecked((y << 5) + (x << 4)) >> 10, unchecked((y << 5) - (x << 4)) >> 10);
    }

    private static int WorldGrid(float value)
    {
        // 419c80 + 49c2a0: biased FISTP under the default nearest-even x87 mode, then SAR6.
        double rounded = Math.Round(value - Math.CopySign(0.49999991059303284, value), MidpointRounding.ToEven);
        if (!float.IsFinite(value) || rounded < int.MinValue || rounded > int.MaxValue)
            throw new ArgumentOutOfRangeException(nameof(value), "APT parent coordinate exceeds signed native bounds.");
        return (int)rounded >> 6;
    }

    public static float WorldGridOrigin(float value) => unchecked(WorldGrid(value) << 6);

    public static Vector3 Place(NativeAptDocument document, NativeAptPoint point, float parentX, float parentZ, float height)
    {
        NativeAptPoint anchor = Project(document, new(document.AnchorX, document.AnchorY));
        NativeAptPoint projected = Project(document, point);
        int x = unchecked(((WorldGrid(parentX) - anchor.X + projected.X) << 6) + 32);
        int z = unchecked(((WorldGrid(parentZ) - anchor.Y + projected.Y) << 6) + 32);
        return new(x, height, z);
    }
}
