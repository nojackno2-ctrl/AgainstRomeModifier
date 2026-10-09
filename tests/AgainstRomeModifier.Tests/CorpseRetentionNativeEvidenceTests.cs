using System.Security.Cryptography;
using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// Regression and native evidence tests for CorpseRetentionPatchOffset.
/// Uses hardcoded native instruction window fixtures and optional read-only verification
/// against Against_Rome.exe from ARM_GAME_PATH without writing to disk.
/// </summary>
public sealed class CorpseRetentionNativeEvidenceTests {
    // 獨立固定的真實 native 記憶體/二進位地址，不以 ExePatchModel 的常數或 OriginalBytes 來 seed
    private const long NativeWindowBaseOffset = 0x1108F0;
    private const long NativeReserveOpcodeOffset = 0x110906; // opcode `mov ebx, imm32` 起點 (BB)
    private const string ExpectedOriginalExeSha256 = "6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf";

    // 0x1108f0 處 40 bytes 原生指令 window
    // 0x1108F0: 53 56 57 55 31 F6 56 BB B0 36 00 00 E8 9F DB F9 FF 29 C3 53 31 ED
    // 0x110906: BB F4 01 00 00  (mov ebx, 0x1F4 = 500)
    // 0x11090B: E8 50 D3 00 00  (call)
    // 0x110910: 29 C3 83 C4 08 89 D8 85
    private static readonly byte[] HardcodedNative40ByteWindow = new byte[] {
        0x53, 0x56, 0x57, 0x55, 0x31, 0xF6, 0x56, 0xBB, 0xB0, 0x36, 0x00, 0x00, 0xE8, 0x9F, 0xDB, 0xF9, 0xFF, 0x29, 0xC3, 0x53, 0x31, 0xED,
        0xBB, 0xF4, 0x01, 0x00, 0x00, // [22..26] = 0x110906..0x11090A (BB F4 01 00 00)
        0xE8, 0x50, 0xD3, 0x00, 0x00, // [27..31] = 0x11090B (call neighbour)
        0x29, 0xC3, 0x83, 0xC4, 0x08, 0x89, 0xD8, 0x85
    };

    [Fact]
    public void Corpse_retention_window_fixture_detects_original_and_applies_safely() {
        // 建立獨立 buffer 容納 0x1108f0 + 40 bytes
        byte[] exeBuffer = new byte[NativeWindowBaseOffset + HardcodedNative40ByteWindow.Length + 0x100];
        Buffer.BlockCopy(HardcodedNative40ByteWindow, 0, exeBuffer, (int)NativeWindowBaseOffset, HardcodedNative40ByteWindow.Length);
        byte[] pristine = exeBuffer.ToArray();

        // 1. 驗證原始狀態辨識
        Assert.Equal(ExeCorpseRetentionPatchState.Original, ExePatchModel.GetCorpseRetentionPatchState(exeBuffer));

        // 2. 規劃套用
        var plan = ExePatchModel.PlanCorpseRetention(true, ExePatchModel.GetCorpseRetentionPatchState(exeBuffer));
        var op = Assert.Single(plan);
        Assert.Equal(NativeReserveOpcodeOffset, op.Offset);
        Assert.Equal(new byte[] { 0xBB, 0xF4, 0x01, 0x00, 0x00 }, op.Expected);
        Assert.Equal(new byte[] { 0xBB, 0x32, 0x00, 0x00, 0x00 }, op.Replacement);

        // 3. 套用補丁
        ExePatchModel.Apply(exeBuffer, plan);
        Assert.Equal(ExeCorpseRetentionPatchState.Patched, ExePatchModel.GetCorpseRetentionPatchState(exeBuffer));

        // 4. 驗證在 40-byte window 內，僅 reserve 立即值兩 bytes (0x110907..0x110908, F4 01 -> 32 00) 變化，
        //    opcode BB、高位 00 00，以及鄰居 call (E8 50 D3 00 00) 完全保留
        for (int i = 0; i < HardcodedNative40ByteWindow.Length; i++) {
            long currentOffset = NativeWindowBaseOffset + i;
            if (currentOffset == NativeReserveOpcodeOffset + 1) { // 0x110907
                Assert.Equal(0x32, exeBuffer[currentOffset]);
            } else if (currentOffset == NativeReserveOpcodeOffset + 2) { // 0x110908
                Assert.Equal(0x00, exeBuffer[currentOffset]);
            } else {
                Assert.Equal(HardcodedNative40ByteWindow[i], exeBuffer[currentOffset]);
            }
        }

        // 5. 規劃還原並 round-trip
        var restorePlan = ExePatchModel.PlanCorpseRetention(false, ExePatchModel.GetCorpseRetentionPatchState(exeBuffer));
        Assert.Single(restorePlan);
        ExePatchModel.Apply(exeBuffer, restorePlan);

        Assert.Equal(ExeCorpseRetentionPatchState.Original, ExePatchModel.GetCorpseRetentionPatchState(exeBuffer));
        Assert.Equal(pristine, exeBuffer);
    }

    [Fact]
    public void Corpse_retention_rejects_unknown_tampered_bytes() {
        byte[] exeBuffer = new byte[NativeWindowBaseOffset + HardcodedNative40ByteWindow.Length + 0x100];
        Buffer.BlockCopy(HardcodedNative40ByteWindow, 0, exeBuffer, (int)NativeWindowBaseOffset, HardcodedNative40ByteWindow.Length);

        // 破壞 opcode BB
        exeBuffer[NativeReserveOpcodeOffset] = 0x90;
        Assert.Equal(ExeCorpseRetentionPatchState.Unknown, ExePatchModel.GetCorpseRetentionPatchState(exeBuffer));
        Assert.Empty(ExePatchModel.PlanCorpseRetention(true, ExeCorpseRetentionPatchState.Unknown));
        Assert.Empty(ExePatchModel.PlanCorpseRetention(false, ExeCorpseRetentionPatchState.Unknown));
    }

    [Fact]
    public void Real_game_exe_read_only_in_memory_verification() {
        string? gamePath = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrWhiteSpace(gamePath)) return;
        string targetExe = Path.Combine(gamePath, "Against_Rome.exe");
        Assert.True(File.Exists(targetExe), "ARM_GAME_PATH must contain Against_Rome.exe.");

        byte[] originalDiskBytes = File.ReadAllBytes(targetExe);
        string diskSha256 = Convert.ToHexString(SHA256.HashData(originalDiskBytes)).ToLowerInvariant();
        Assert.Equal(ExpectedOriginalExeSha256, diskSha256);
        Assert.Equal(HardcodedNative40ByteWindow,
            originalDiskBytes.AsSpan((int)NativeWindowBaseOffset, HardcodedNative40ByteWindow.Length).ToArray());
        byte[] memoryCopy = (byte[])originalDiskBytes.Clone();
        Assert.Equal(ExeCorpseRetentionPatchState.Original, ExePatchModel.GetCorpseRetentionPatchState(memoryCopy));
        var plan = ExePatchModel.PlanCorpseRetention(true, ExePatchModel.GetCorpseRetentionPatchState(memoryCopy));
        Assert.Single(plan);
        ExePatchModel.Apply(memoryCopy, plan);
        Assert.Equal(ExeCorpseRetentionPatchState.Patched, ExePatchModel.GetCorpseRetentionPatchState(memoryCopy));
        Assert.Empty(ExePatchModel.PlanCorpseRetention(true, ExePatchModel.GetCorpseRetentionPatchState(memoryCopy)));

        byte[] expectedPatched = (byte[])originalDiskBytes.Clone();
        expectedPatched[NativeReserveOpcodeOffset + 1] = 0x32;
        expectedPatched[NativeReserveOpcodeOffset + 2] = 0x00;
        Assert.Equal(expectedPatched, memoryCopy); // All bytes outside the immediate remain identical.

        var restorePlan = ExePatchModel.PlanCorpseRetention(false, ExePatchModel.GetCorpseRetentionPatchState(memoryCopy));
        Assert.Single(restorePlan);
        ExePatchModel.Apply(memoryCopy, restorePlan);
        Assert.Equal(ExeCorpseRetentionPatchState.Original, ExePatchModel.GetCorpseRetentionPatchState(memoryCopy));
        Assert.Equal(originalDiskBytes, memoryCopy);

        // 確保磁碟檔案絕無被寫入或變更
        byte[] finalDiskBytes = File.ReadAllBytes(targetExe);
        string finalDiskSha256 = Convert.ToHexString(SHA256.HashData(finalDiskBytes)).ToLowerInvariant();
        Assert.Equal(diskSha256, finalDiskSha256);
    }
}
