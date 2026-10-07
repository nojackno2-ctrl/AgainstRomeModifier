namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// Native ALR/APT palette DWORDs store red in the low byte (0x00BBGGRR). Decoded
/// real assets confirmed this: unswapped gerhau02 rendered cyan torch flames and
/// blue thatch, while the swapped order yields yellow flames and straw-coloured roofs.
/// </summary>
internal static class NativePaletteColor
{
    public static uint ToArgb(uint stored)
        => 0xFF000000 | ((stored & 0xFF) << 16) | (stored & 0xFF00) | ((stored >> 16) & 0xFF);
}
