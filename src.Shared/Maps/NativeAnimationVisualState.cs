using System.Buffers.Binary;

namespace AgainstRomeModifier.Maps;

/// <summary>
/// Detached serialized animation state for one native object. This is a snapshot,
/// not a simulation of native ticks, construction progress, or parent animation.
/// </summary>
public sealed record NativeAnimationVisualState
{
    public const int RecordSize = 21;

    public NativeAnimationVisualState(short alrFrame, short aptAxis0, short aptAxis1, short aptAxis2, short aptAxis3)
    {
        AlrFrame = alrFrame;
        AptAxis0 = aptAxis0;
        AptAxis1 = aptAxis1;
        AptAxis2 = aptAxis2;
        AptAxis3 = aptAxis3;
    }

    public short AlrFrame { get; }
    public short AptAxis0 { get; }
    public short AptAxis1 { get; }
    public short AptAxis2 { get; }
    public short AptAxis3 { get; }

    /// <summary>
    /// Reads exactly one unpacked anim.dat record; inactive or incorrectly sized
    /// records return null. Signed values are preserved without clamping or defaults.
    /// </summary>
    public static NativeAnimationVisualState? ParseRecord(ReadOnlySpan<byte> record)
    {
        if (record.Length != RecordSize || record[0] == 0) return null;

        // Shipped EXE SHA256 6ac85239…: 48c250 maps disk +5 to runtime +8.
        // 4b53c0's frame=-1 fallback sign-extends that word with SAR 16.
        // 4b4790 -> 4cbf70 passes runtime +12/+16/+14/+18 (hex):
        // disk +13/+17/+15/+19. Disk +11 (runtime +10) is NOT axis0.
        return new(
            BinaryPrimitives.ReadInt16LittleEndian(record.Slice(5, 2)),
            BinaryPrimitives.ReadInt16LittleEndian(record.Slice(13, 2)),
            BinaryPrimitives.ReadInt16LittleEndian(record.Slice(17, 2)),
            BinaryPrimitives.ReadInt16LittleEndian(record.Slice(15, 2)),
            BinaryPrimitives.ReadInt16LittleEndian(record.Slice(19, 2)));
    }
}
