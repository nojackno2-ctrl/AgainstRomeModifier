namespace AgainstRomeModifier.Tests;

public sealed class EndlessSchedulerDeadlinePatchTests
{
    [Fact]
    public void Save_repair_migrates_embedded_scheduler_and_reinforcement_rules_and_is_idempotent()
    {
        string root = Path.Combine(Path.GetTempPath(), "arm-endless-save-repair-" + Guid.NewGuid().ToString("N"));
        string scrPath = Path.Combine(root, "SAVE", "ESAVE_000", "CLAK", "scr.dat");
        Directory.CreateDirectory(Path.GetDirectoryName(scrPath)!);
        try
        {
            File.WriteAllBytes(scrPath, CreateEmbeddedSaveScr());
            var service = new Core.Services.EndlessSaveAiRepairService();

            Assert.Equal(Core.Services.EndlessSaveAiRepairResult.Changed, service.Repair(root, "ESAVE_000"));
            Assert.Equal(Core.Services.EndlessSaveAiRepairResult.AlreadyRepaired, service.Repair(root, "ESAVE_000"));

            byte[] decompressed = GameLZSS.DecompressPfil(File.ReadAllBytes(scrPath));
            int bciOffset = FindBytes(decompressed, "BCI0"u8.ToArray());
            Assert.True(bciOffset >= 0);
            byte[] bciTail = decompressed[bciOffset..];
            Assert.Equal(PatchState.Ultimate, new P6_LoopDelayPatch().Detect(bciTail));
            Assert.Equal(PatchState.Ultimate, new P8_ReinforcementUnitThresholdPatch().Detect(bciTail));
            Assert.Equal(PatchState.Ultimate, new P9_RetreatQuotaPatch().Detect(bciTail));
        }
        finally
        {
            if (Directory.Exists(root)) Directory.Delete(root, recursive: true);
        }
    }

    [Fact]
    public void Save_repair_rejects_path_traversal()
    {
        var service = new Core.Services.EndlessSaveAiRepairService();
        Assert.Throws<InvalidOperationException>(() => service.Repair(Path.GetTempPath(), ".."));
    }

    [Fact]
    public void P6_round_trips_all_five_local_endless_re_samples_when_available()
    {
        DirectoryInfo? root = new(AppContext.BaseDirectory);
        while (root != null && !Directory.Exists(Path.Combine(root.FullName, "re_workspace", "endless_bci")))
            root = root.Parent;
        if (root == null) return; // Public source distributions omit proprietary RE samples.

        string sampleRoot = Path.Combine(root.FullName, "re_workspace", "endless_bci");
        string[] samples = Directory.GetFiles(sampleRoot, "ENDL_*_ak_level.dec.bci");
        Assert.Equal(5, samples.Length);

        var patch = new P6_LoopDelayPatch();
        foreach (string sample in samples)
        {
            byte[] original = File.ReadAllBytes(sample);
            byte[] current = (byte[])original.Clone();
            Assert.Equal(PatchState.Original, patch.Detect(current));
            Assert.True(patch.Apply(ref current, enabled: true));
            Assert.Equal(PatchState.Ultimate, patch.Detect(current));
            Assert.True(patch.Apply(ref current, enabled: false));
            Assert.Equal(PatchState.Original, patch.Detect(current));
            Assert.Equal(original, current);
        }
    }

    [Fact]
    public void P6_round_trip_adds_bounded_save_load_deadline_repair()
    {
        byte[] original = CreateOriginalFixture(out int schedulerOffset, out _);
        byte[] current = (byte[])original.Clone();
        var patch = new P6_LoopDelayPatch();

        Assert.Equal(PatchState.Original, patch.Detect(current));
        Assert.True(patch.Apply(ref current, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(current));
        Assert.False(patch.Apply(ref current, enabled: true));

        int[] repaired = P6_LoopDelayPatch.UltimateSchedulerBlockWords.ToArray();
        Assert.Equal(repaired, ReadWords(current, schedulerOffset, repaired.Length));
        Assert.Equal(P6_LoopDelayPatch.DeadlineRollbackThresholdMs, repaired[23]);
        Assert.Equal(30000, repaired[35]);
        Assert.Equal(118, repaired[15]); // now >= v16 -> reset branch
        Assert.Equal(118, repaired[27]); // far-future v16 -> reset branch
        Assert.Equal(120, repaired[63]); // original settlement-spawner call offset retained
        Assert.Equal(-9748, repaired[64]);

        Assert.True(patch.Apply(ref current, enabled: false));
        Assert.Equal(PatchState.Original, patch.Detect(current));
        Assert.Equal(original, current);
    }

    [Fact]
    public void P6_migrates_old_accelerated_loops_without_rollback_guard()
    {
        byte[] current = CreateOriginalFixture(out int schedulerOffset, out int[] loopOffsets);
        foreach (int offset in loopOffsets)
        {
            WriteInt32(current, offset + 4, 30000);
            WriteInt32(current, offset + 12, 30000);
        }

        // The legacy P6 implementation changed both scheduler randRange pairs to 30000
        // but retained the direct now >= v16 guard.
        foreach (int wordIndex in new[] { 32, 34, 45, 47 })
            WriteInt32(current, schedulerOffset + (wordIndex * 4), 30000);

        var patch = new P6_LoopDelayPatch();
        Assert.Equal(PatchState.Legacy, patch.Detect(current));
        Assert.True(patch.Apply(ref current, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(current));
        Assert.Equal(P6_LoopDelayPatch.UltimateSchedulerBlockWords.ToArray(),
            ReadWords(current, schedulerOffset, P6_LoopDelayPatch.UltimateSchedulerBlockWords.Length));
    }

    [Theory]
    [InlineData(1_000_000, 1_030_000, false)] // normal 30-second pending deadline
    [InlineData(1_030_000, 1_030_000, true)]  // normally reached
    [InlineData(1_000_000, 1_060_001, true)]  // save/load rollback beyond the bound
    public void P6_guard_preserves_normal_throttle_and_repairs_only_large_future_gaps(
        int now, int deadline, bool expectedDue)
    {
        bool due = now >= deadline || deadline > now + P6_LoopDelayPatch.DeadlineRollbackThresholdMs;
        Assert.Equal(expectedDue, due);
    }

    private static byte[] CreateOriginalFixture(out int schedulerOffset, out int[] loopOffsets)
    {
        (int Upper, int Lower)[] ranges = {
            (960000, 480000),
            (960000, 480000),
            (360000, 240000),
            (120000, 60000)
        };
        var words = new List<int>();
        var offsets = new List<int>();
        foreach ((int upper, int lower) in ranges)
        {
            offsets.Add(words.Count * 4);
            words.AddRange(new[] { 66, upper, 66, lower, 128, 16, 112, 0 });
        }

        schedulerOffset = words.Count * 4;
        words.AddRange(P6_LoopDelayPatch.OriginalSchedulerBlockWords.ToArray());
        loopOffsets = offsets.ToArray();

        byte[] result = new byte[words.Count * 4];
        for (int i = 0; i < words.Count; i++) WriteInt32(result, i * 4, words[i]);
        return result;
    }

    private static byte[] CreateEmbeddedSaveScr()
    {
        byte[] scheduler = CreateOriginalFixture(out _, out _);
        byte[] reinforcement = CreateLegacyReinforcementFixture();
        byte[] fixture = new byte[scheduler.Length + reinforcement.Length];
        Buffer.BlockCopy(scheduler, 0, fixture, 0, scheduler.Length);
        Buffer.BlockCopy(reinforcement, 0, fixture, scheduler.Length, reinforcement.Length);
        byte[] bci = new byte[36 + fixture.Length];
        "BCI0"u8.CopyTo(bci);
        Buffer.BlockCopy(fixture, 0, bci, 36, fixture.Length);

        byte[] path = System.Text.Encoding.ASCII.GetBytes("MAPS/ENDL_002/SCRIPT/ak_level\0");
        byte[] decompressed = new byte[64 + path.Length + bci.Length + 32];
        Buffer.BlockCopy(path, 0, decompressed, 64, path.Length);
        Buffer.BlockCopy(bci, 0, decompressed, 64 + path.Length, bci.Length);

        byte[] header = new byte[64];
        "PFIL"u8.CopyTo(header);
        return GameLZSS.CompressPfil(decompressed, header);
    }

    private static byte[] CreateLegacyReinforcementFixture()
    {
        const int gap = unchecked((int)0x6F6F6F6F);
        var words = new List<int>
        {
            90, 0, 66, 40, 96, 98, 91, 11,
            66, 0, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32,
            gap, gap
        };
        for (int i = 0; i < 10; i++)
        {
            int opcode = i is 8 or 9 ? 90 : 66;
            int value = i == 8 ? 6 : i == 9 ? 15 : 0;
            if (i == 9)
            {
                opcode = 66;
                value = 0;
            }
            words.AddRange(new[] { 81, 56, 90, -3, opcode, value, 164, gap, gap });
        }
        words.AddRange(new[] { 128, 214, 73, -2, 86, 66, 1, 96, 102, 117, 0 });

        byte[] result = new byte[words.Count * sizeof(int)];
        for (int i = 0; i < words.Count; i++) WriteInt32(result, i * sizeof(int), words[i]);
        return result;
    }

    private static int FindBytes(byte[] data, byte[] pattern)
    {
        for (int i = 0; i <= data.Length - pattern.Length; i++)
            if (data.AsSpan(i, pattern.Length).SequenceEqual(pattern)) return i;
        return -1;
    }

    private static int[] ReadWords(byte[] bytes, int offset, int count)
    {
        var result = new int[count];
        for (int i = 0; i < count; i++) result[i] = BitConverter.ToInt32(bytes, offset + (i * 4));
        return result;
    }

    private static void WriteInt32(byte[] bytes, int offset, int value) =>
        BitConverter.GetBytes(value).CopyTo(bytes, offset);
}
