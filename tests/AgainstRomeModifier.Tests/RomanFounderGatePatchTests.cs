namespace AgainstRomeModifier.Tests;

// P17: the RoemischeGruender (type-4 Roman military settlement) spawner keeps a
// 60% probability gate (60 >= s_randRange(1,100)) before it may create the sole
// reinforcing Roman settlement. Under certain AI configurations the CPU-team pool
// may saturate before a 60% roll lands, so the founder never spawns and Rome never
// reinforces. The mandatory repair raises the threshold to 100 so the roll is
// always satisfied; disabling restores the vanilla 60.
public sealed class RomanFounderGatePatchTests
{
    private static BciLiteralPatch MakePatch() => new BciLiteralPatch(
        "P17",
        "MAPS/ENDL_*/SCRIPT/ak_level.bci",
        new int?[] { 66, null, 66, 100, 66, 1, 128, 16, 73, -2, 86, 96, 101, 117 },
        new int[] { 1 },
        new int[] { 60 },
        new int[] { 100 },
        1);

    [Fact]
    public void P17_guarantees_the_roman_founder_gate_and_restores_it()
    {
        byte[] data = BuildFixture(threshold: 60);
        var patch = MakePatch();

        Assert.Equal(PatchState.Original, patch.Detect(data));

        Assert.True(patch.Apply(ref data, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(data));
        Assert.Equal(100, GateThreshold(data));

        // Idempotent while enabled.
        Assert.False(patch.Apply(ref data, enabled: true));

        Assert.True(patch.Apply(ref data, enabled: false));
        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.Equal(60, GateThreshold(data));
    }

    [Fact]
    public void P17_reports_unknown_when_gate_signature_is_absent()
    {
        // A single altered opcode in the gate must make the site unmatched
        // rather than silently patched (unknown-state refusal convention).
        byte[] data = BuildFixture(threshold: 60);
        // Corrupt the ge opcode (word index 12 within the signature at offset 2).
        BitConverter.GetBytes(999).CopyTo(data, (2 + 12) * 4);

        Assert.Equal(PatchState.Unknown, MakePatch().Detect(data));
    }

    // Two leading filler words keep the signature off offset 0 so the word-index
    // arithmetic is exercised realistically; trailing words avoid a truncated tail.
    private const int SigStartWord = 2;

    private static byte[] BuildFixture(int threshold)
    {
        var words = new List<int>
        {
            71, 87,                                   // filler: pop / return marker
            66, threshold, 66, 100, 66, 1,            // pushlit thr; s_randRange(1,100) args
            128, 16, 73, -2, 86,                      // pushsym s_randRange; argc -2; call
            96, 101, 117, 20,                         // cmp0; ge; jz <target>
            120, 0, 86,                               // callint create; call
            91, 0, 90, 0, 87                          // arrstore; return
        };

        byte[] data = new byte[words.Count * 4];
        for (int i = 0; i < words.Count; i++) BitConverter.GetBytes(words[i]).CopyTo(data, i * 4);
        return data;
    }

    private static int GateThreshold(byte[] data) =>
        BitConverter.ToInt32(data, (SigStartWord + 1) * 4);
}
