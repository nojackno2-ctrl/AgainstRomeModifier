using System.Buffers.Binary;

namespace AgainstRomeModifier.Maps;

public sealed partial class LevelObjectStore
{
    /// <summary>
    /// Reads this object's own serialized animation state only while its slot is
    /// active and still has the expected full UID. Missing, inactive, or corrupt
    /// animation links return null so callers can retain their legacy preview.
    /// Child objects use their own link; no parent fallback or pool slot allocation occurs.
    /// </summary>
    public NativeAnimationVisualState? AnimationVisualStateAt(int slot, uint expectedUid)
    {
        if (slot < 0 || slot >= _count || UidAt(slot) != expectedUid || _anim is null)
            return null;

        // objects core +71 is loaded as a signed runtime animation slot (+0x12).
        int animSlot = BinaryPrimitives.ReadInt16LittleEndian(_objects.AsSpan(RecordOffset(slot) + 71, 2));
        if (animSlot < 0 || animSlot >= _count || RecordReferenced(71, animSlot, slot))
            return null;

        int offset = 8 + animSlot * NativeAnimationVisualState.RecordSize;
        if (offset < 8 || offset > _anim.Length - NativeAnimationVisualState.RecordSize)
            return null;

        return NativeAnimationVisualState.ParseRecord(_anim.AsSpan(offset, NativeAnimationVisualState.RecordSize));
    }
}
