using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Tests;

public sealed class VerifiedPatchTests {
    [Fact]
    public void Fixed_offset_writer_rejects_unknown_bytes_without_modifying_buffer() {
        byte[] buffer = { 0x10, 0x20, 0x30, 0x40 };
        byte[] before = buffer.ToArray();

        InvalidDataException error = Assert.Throws<InvalidDataException>(() =>
            VerifiedBinaryWriter.WriteBytes(buffer, 1, new byte[] { 0x99, 0x30 }, new byte[] { 0xAA, 0xBB }, "synthetic exe patch"));

        Assert.Contains("0x1", error.Message);
        Assert.Contains("不符合預期", error.Message);
        Assert.Equal(before, buffer);
    }

    [Fact]
    public void Bci_writer_checks_expected_int32_before_writing() {
        byte[] buffer = new byte[12];
        BitConverter.GetBytes(20).CopyTo(buffer, 4);

        BciPattern.WriteBciInt32(buffer, 4, 20, 40, "synthetic bci patch");
        Assert.Equal(40, BitConverter.ToInt32(buffer, 4));
        Assert.Throws<InvalidDataException>(() => BciPattern.WriteBciInt32(buffer, 4, 20, 60, "synthetic bci patch"));
        Assert.Equal(40, BitConverter.ToInt32(buffer, 4));
    }

    [Fact]
    public void Rollback_scope_restores_an_already_written_file_after_failure() {
        string path = Path.Combine(Path.GetTempPath(), "AgainstRomeModifierTests_" + Guid.NewGuid().ToString("N") + ".bin");
        try {
            File.WriteAllBytes(path, new byte[] { 1, 2, 3 });
            using (var rollback = new FileRollbackScope()) {
                rollback.TrackFile(path);
                File.WriteAllBytes(path, new byte[] { 9, 9, 9 });
                rollback.RestoreAll(null);
            }
            Assert.Equal(new byte[] { 1, 2, 3 }, File.ReadAllBytes(path));
        } finally {
            if (File.Exists(path)) File.Delete(path);
        }
    }
}
