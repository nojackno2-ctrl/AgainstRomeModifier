namespace AgainstRomeModifier.Tests;

/// <summary>
/// Tests for P8_ReinforcementUnitThresholdPatch, which controls the
/// s_searchTeamUnits(team) threshold for sending another reinforcement wave.
/// The current P8 uses threshold=30 with the original condition tail as Ultimate state.
/// The former limit=40 state remains recognized as Legacy and migrates on apply.
/// The search pattern expects: 90,0,66,{limit},96,98,91,11, {2 gate words}, 66,0,66,0,66,0,66,0, 90,6,102,117,32
/// So the full original gate is: 66,0, 66,0, 66,0, 66,0, 66,0, 90,6, 102,117,32 (5×pushlit_0 + pushsym_6 + ...)
/// </summary>
public sealed class ReinforcementGatePatchTests
{
    // The original gate has 5 pushlit-0 pairs followed by pushsym-6, cmp, jmp, done
    // Pattern: null,null at word 8-9, then 66,0,66,0,66,0,66,0 at word 10-17, then 90,6,102,117,32
    // So the full original gate occupies word 8..22, that is 15 words:
    //   66,0, 66,0, 66,0, 66,0, 66,0, 90,6, 102,117,32
    private static readonly int[] OriginalGate = {
        66, 0, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32
    };

    [Fact]
    public void P8_applies_a_unit_count_only_bounded_gate_and_restores_it()
    {
        byte[] data = BuildFixture(limit: 4, legacyUnbounded: false);
        var patch = new P8_ReinforcementUnitThresholdPatch();

        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.True(patch.Apply(ref data, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(data));
        Assert.Equal(30, ReadWord(data, 3));
        // Gate remains original (66,0 series) in this version of P8
        Assert.Equal(OriginalGate, ReadWords(data, 8, OriginalGate.Length));

        Assert.False(patch.Apply(ref data, enabled: true));
        Assert.True(patch.Apply(ref data, enabled: false));
        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.Equal(4, ReadWord(data, 3));
        Assert.Equal(OriginalGate, ReadWords(data, 8, OriginalGate.Length));
    }

    [Fact]
    public void P8_migrates_limit40_original_gate_and_old_unbounded_gate()
    {
        var patch = new P8_ReinforcementUnitThresholdPatch();

        // The former limit=40 with original gate is Legacy and migrates to 30.
        byte[] current = BuildFixture(limit: 40, legacyUnbounded: false);
        Assert.Equal(PatchState.Legacy, patch.Detect(current));
        Assert.True(patch.Apply(ref current, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(current));
        Assert.Equal(30, ReadWord(current, 3));

        // Old unbounded gate (112,272) is Legacy; Apply migrates it to Ultimate
        byte[] unbounded = BuildFixture(limit: 8, legacyUnbounded: true);
        Assert.Equal(PatchState.Legacy, patch.Detect(unbounded));
        Assert.True(patch.Apply(ref unbounded, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(unbounded));
        Assert.Equal(30, ReadWord(unbounded, 3));
        // Gate is restored to original (66,0 series)
        Assert.Equal(OriginalGate, ReadWords(unbounded, 8, OriginalGate.Length));
    }

    [Fact]
    public void P8_migrates_bounded_gate_from_experimental_build()
    {
        var patch = new P8_ReinforcementUnitThresholdPatch();

        // BoundedGate: 90,11,117,252, 90,11,117,236, 90,11,117,220, 112,224,32
        // This was used in experimental builds and causes the "unknown state" error
        // if not recognized. With the former limit=40, this should be detected as Legacy.
        int[] boundedGate = {
            90, 11, 117, 252,
            90, 11, 117, 236,
            90, 11, 117, 220,
            112, 224, 32
        };
        byte[] data = BuildFixtureWithGate(limit: 40, boundedGate);
        Assert.Equal(PatchState.Legacy, patch.Detect(data));
        Assert.True(patch.Apply(ref data, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(data));
        Assert.Equal(30, ReadWord(data, 3));
        Assert.Equal(OriginalGate, ReadWords(data, 8, OriginalGate.Length));

        // Also test restore to original
        Assert.True(patch.Apply(ref data, enabled: false));
        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.Equal(4, ReadWord(data, 3));
    }

    [Fact]
    public void P8_migrates_both_exact_bounded_gate_variants()
    {
        var patch = new P8_ReinforcementUnitThresholdPatch();
        int[][] gates = {
            new[] { 90, 11, 117, 252, 90, 11, 117, 236, 90, 11, 117, 220, 112, 224, 32 },
            new[] { 90, 11, 117, 252, 90, 11, 117, 236, 90, 11, 117, 220, 112, 232, 32 }
        };

        foreach (int[] gate in gates)
        {
            byte[] data = BuildFixtureWithGate(limit: 40, gate);
            Assert.Equal(PatchState.Legacy, patch.Detect(data));
            Assert.True(patch.Apply(ref data, enabled: true));
            Assert.Equal(PatchState.Ultimate, patch.Detect(data));
            Assert.Equal(30, ReadWord(data, 3));
            Assert.Equal(OriginalGate, ReadWords(data, 8, OriginalGate.Length));
        }
    }

    [Fact]
    public void P8_rejects_unknown_threshold_without_writing()
    {
        var patch = new P8_ReinforcementUnitThresholdPatch();
        byte[] data = BuildFixture(limit: 31, legacyUnbounded: false);
        byte[] before = (byte[])data.Clone();

        Assert.Equal(PatchState.Unknown, patch.Detect(data));
        Assert.Throws<InvalidOperationException>(() => patch.Apply(ref data, enabled: true));
        Assert.Equal(before, data);
        Assert.False(patch.Apply(ref data, enabled: false));
        Assert.Equal(before, data);
    }

    [Fact]
    public void P8_rejects_unknown_gate_without_writing()
    {
        var patch = new P8_ReinforcementUnitThresholdPatch();
        int[] unknownGate = {
            90, 11, 117, 251,
            90, 11, 117, 236,
            90, 11, 117, 220,
            112, 224, 32
        };
        byte[] data = BuildFixtureWithGate(limit: 40, unknownGate);
        byte[] before = (byte[])data.Clone();

        Assert.Equal(PatchState.Unknown, patch.Detect(data));
        Assert.Throws<InvalidOperationException>(() => patch.Apply(ref data, enabled: true));
        Assert.Equal(before, data);
        Assert.False(patch.Apply(ref data, enabled: false));
        Assert.Equal(before, data);
    }

    private static byte[] BuildFixture(int limit, bool legacyUnbounded)
    {
        // Legacy unbounded gate: 112,272 followed by the same 5 pushlit-0's + pushsym etc.
        int[] gate = legacyUnbounded
            ? new int[] { 112, 272, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32 }
            : OriginalGate;
        return BuildFixtureWithGate(limit, gate);
    }

    private static byte[] BuildFixtureWithGate(int limit, int[] gate)
    {
        var words = new List<int> {
            90, 0, 66, limit, 96, 98, 91, 11
        };
        words.AddRange(gate);
        words.AddRange(new int[] {
            66, 0, 87,
            120, 0, 86, 91, 13, 90, 13, 87
        });

        byte[] data = new byte[words.Count * 4];
        for (int i = 0; i < words.Count; i++) BitConverter.GetBytes(words[i]).CopyTo(data, i * 4);
        return data;
    }

    private static int ReadWord(byte[] data, int wordIndex) => BitConverter.ToInt32(data, wordIndex * 4);

    private static int[] ReadWords(byte[] data, int wordIndex, int count) =>
        Enumerable.Range(0, count).Select(i => ReadWord(data, wordIndex + i)).ToArray();
}
