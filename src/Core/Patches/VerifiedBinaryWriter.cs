namespace AgainstRomeModifier.Core.Patches;

/// <summary>固定偏移寫入的共同防護：只有目前位元組符合預期狀態時才允許覆寫。</summary>
public static class VerifiedBinaryWriter {
    public static void WriteBytes(byte[] buffer, long offset, ReadOnlySpan<byte> expected, ReadOnlySpan<byte> replacement, string patchName) {
        ArgumentNullException.ThrowIfNull(buffer);
        if (offset < 0 || offset > int.MaxValue || offset + expected.Length > buffer.Length || expected.Length != replacement.Length) {
            throw new InvalidDataException($"{patchName} 的固定偏移 0x{offset:X} 或寫入長度無效，已中止補丁。");
        }
        ReadOnlySpan<byte> current = buffer.AsSpan((int)offset, expected.Length);
        if (!current.SequenceEqual(expected)) {
            throw new InvalidDataException($"{patchName} 在偏移 0x{offset:X} 的位元組不符合預期；預期 {Convert.ToHexString(expected)}，實際 {Convert.ToHexString(current)}。檔案可能是不同版本，已中止補丁。");
        }
        replacement.CopyTo(buffer.AsSpan((int)offset, replacement.Length));
    }

    public static void WriteInt32(byte[] buffer, int offset, int expected, int replacement, string patchName) {
        Span<byte> expectedBytes = stackalloc byte[sizeof(int)];
        Span<byte> replacementBytes = stackalloc byte[sizeof(int)];
        BitConverter.TryWriteBytes(expectedBytes, expected);
        BitConverter.TryWriteBytes(replacementBytes, replacement);
        WriteBytes(buffer, offset, expectedBytes, replacementBytes, patchName);
    }
}
