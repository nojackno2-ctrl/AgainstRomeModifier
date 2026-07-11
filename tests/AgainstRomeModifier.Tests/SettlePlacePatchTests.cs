namespace AgainstRomeModifier.Tests;

// P18/P19: settle-place eligibility fix for the long-standing "defeated CPUs stop
// respawning" endless-mode bug. Village (type-1) and Roman founder (type-4)
// re-creation both call the free-settle-place finder (fn 0x9904), which vetoes a
// place when fn 0xA54 finds a village center OR any team's units within radius
// 2500. Late-game the player's expansion (units counted by fn 0x858, whose team
// loop starts at team 0 = the player) plus dead-team leftovers permanently veto
// all 8 places, so create silently deletes the fresh party and returns 0 forever.
// P18 raises the unit-scan comparand 0 -> 1 so player units no longer veto a
// place; P19 shrinks the veto radius 2500 -> 800 so only genuine on-site
// occupation blocks it.
public sealed class SettlePlacePatchTests
{
    private static BciLiteralPatch MakeUnitScanPatch() => new BciLiteralPatch(
        "P18",
        "MAPS/ENDL_*/SCRIPT/ak_level.bci",
        new int?[] { 120, -636, 73, -3, 86, 66, null, 96, 101, 117, 20, 66, 0, 87 },
        new int[] { 6 },
        new int[] { 0 },
        new int[] { 1 },
        1);

    private static BciLiteralPatch MakeRadiusPatch() => new BciLiteralPatch(
        "P19",
        "MAPS/ENDL_*/SCRIPT/ak_level.bci",
        new int?[] { 66, null, 90, 1, 90, 0, 120, -36800, 73, -3, 86 },
        new int[] { 1 },
        new int[] { 2500 },
        new int[] { 800 },
        1);

    [Fact]
    public void P18_exempts_player_units_and_restores()
    {
        byte[] data = BuildUnitScanFixture(comparand: 0);
        var patch = MakeUnitScanPatch();

        Assert.Equal(PatchState.Original, patch.Detect(data));

        Assert.True(patch.Apply(ref data, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(data));
        Assert.Equal(1, UnitScanComparand(data));

        Assert.False(patch.Apply(ref data, enabled: true));

        Assert.True(patch.Apply(ref data, enabled: false));
        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.Equal(0, UnitScanComparand(data));
    }

    [Fact]
    public void P19_shrinks_place_veto_radius_and_restores()
    {
        byte[] data = BuildRadiusFixture(radius: 2500);
        var patch = MakeRadiusPatch();

        Assert.Equal(PatchState.Original, patch.Detect(data));

        Assert.True(patch.Apply(ref data, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(data));
        Assert.Equal(800, PlaceRadius(data));

        Assert.False(patch.Apply(ref data, enabled: true));

        Assert.True(patch.Apply(ref data, enabled: false));
        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.Equal(2500, PlaceRadius(data));
    }

    [Fact]
    public void P18_reports_unknown_when_signature_absent()
    {
        byte[] data = BuildUnitScanFixture(comparand: 0);
        // Corrupt the callint operand so the anchor no longer matches.
        BitConverter.GetBytes(-999).CopyTo(data, (SigStartWord + 1) * 4);

        Assert.Equal(PatchState.Unknown, MakeUnitScanPatch().Detect(data));
    }

    [Fact]
    public void P19_reports_unknown_when_signature_absent()
    {
        byte[] data = BuildRadiusFixture(radius: 2500);
        BitConverter.GetBytes(-999).CopyTo(data, (SigStartWord + 7) * 4);

        Assert.Equal(PatchState.Unknown, MakeRadiusPatch().Detect(data));
    }

    // Two leading filler words keep the signature off offset 0 so the word-index
    // arithmetic is exercised realistically; trailing words avoid a truncated tail.
    private const int SigStartWord = 2;

    private static byte[] BuildUnitScanFixture(int comparand)
    {
        var words = new List<int>
        {
            71, 87,                       // filler
            120, -636, 73, -3, 86,        // callint fn0x858; argc -3; call
            66, comparand,                // pushlit <comparand>
            96, 101, 117, 20,             // cmp0; ge; jz +20
            66, 0, 87,                    // pushlit 0; return (place blocked)
            112, 20, 66, 1, 87            // jmp; pushlit 1; return (place clear)
        };
        return ToBytes(words);
    }

    private static byte[] BuildRadiusFixture(int radius)
    {
        var words = new List<int>
        {
            71, 87,                       // filler
            66, radius,                   // pushlit <radius>
            90, 1, 90, 0,                 // push locals x/y
            120, -36800, 73, -3, 86,      // callint fnA54; argc -3; call
            91, 5, 87                     // arrstore; return
        };
        return ToBytes(words);
    }

    private static byte[] ToBytes(List<int> words)
    {
        byte[] data = new byte[words.Count * 4];
        for (int i = 0; i < words.Count; i++) BitConverter.GetBytes(words[i]).CopyTo(data, i * 4);
        return data;
    }

    private static int UnitScanComparand(byte[] data) =>
        BitConverter.ToInt32(data, (SigStartWord + 6) * 4);

    private static int PlaceRadius(byte[] data) =>
        BitConverter.ToInt32(data, (SigStartWord + 1) * 4);
}
