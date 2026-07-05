using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Tests;

public sealed class PatcherRoundTripTests {
    private static readonly IReadOnlyDictionary<string, double> NoSkills = new Dictionary<string, double>();
    private static readonly IReadOnlyDictionary<string, double[]> NoStats = new Dictionary<string, double[]>();

    [Fact]
    public void Objdef_capacity_and_build_time_patch_are_exact_and_restore_from_fixture() {
        string[] columns = Enumerable.Repeat("     0", 200).ToArray();
        columns[42] = "   100";
        columns[52] = "BauHau00_Haus".PadLeft(20);
        columns[73] = "  1000";
        columns[74] = "   500";
        columns[156] = "    10";
        byte[] original = SyntheticFixture.Pfil("header1\r\nheader2\r\n" + string.Join(',', columns) + "\r\n");
        var enabled = new ObjdefOptions(false, true, true, true, NoStats, NoStats);

        byte[] patched = ObjdefPatcher.GetPatchedBytes(original, enabled);
        string[] result = SyntheticFixture.Text(patched).Split("\r\n")[2].Split(',');

        Assert.Equal("200", result[156].Trim());
        Assert.Equal("1000", result[42].Trim());
        Assert.Equal("100", result[73].Trim());
        Assert.Equal("50", result[74].Trim());
        Assert.Equal(original, ObjdefPatcher.GetPatchedBytes(original, new(false, false, false, false, NoStats, NoStats)));
    }

    [Fact]
    public void Ress_cost_patch_zeros_only_selected_fields_and_restores() {
        string building = "BauHau00," + string.Join(',', Enumerable.Repeat("10", 12));
        string unit = "FigGerPri00_Priester," + string.Join(',', Enumerable.Repeat("5", 28));
        byte[] original = SyntheticFixture.Pfil("[objres]\n" + building + "\n" + unit + "\n");

        byte[] patched = RessPatcher.GetPatchedBytes(original, new(true, true, true));
        string[] lines = SyntheticFixture.Text(patched).Split('\n');
        Assert.All(lines[1].Split(',').Skip(1), value => Assert.Equal("0", value));
        Assert.All(lines[2].Split(',').Skip(13).Take(6), value => Assert.Equal("0", value));
        Assert.All(lines[2].Split(',').Skip(25).Take(4), value => Assert.Equal("0", value));
        Assert.Equal(original, RessPatcher.GetPatchedBytes(original, new(false, false, false)));
    }

    [Fact]
    public void ClScript_patch_changes_expected_values_and_restore_uses_canonical_fixture() {
        const string text = "CiviDelay  =GER, 5000      ; c\nRadius     =KEL, Spell0, 500       ; r\nValue      =KEL, Spell1, 2         ; h\nMoralsDecFlee=GER, 10; m\n";
        byte[] original = SyntheticFixture.Pfil(text);
        var options = new ClScriptOptions(true, true, true, NoSkills, 1, 2.5, 1, original);

        string patched = SyntheticFixture.Text(ClScriptPatcher.GetPatchedBytes(original, options));
        Assert.Contains("GER, 500", patched);
        Assert.Contains("KEL, Spell0, 1250", patched);
        Assert.Contains("KEL, Spell1, 100", patched);
        Assert.Contains("MoralsDecFlee=GER, 0", patched);
        Assert.Equal(original, ClScriptPatcher.GetPatchedBytes(original, new(false, false, false, NoSkills, 1, 1, 1, original)));
    }

    [Fact]
    public void TeamDat_patch_sets_positive_population_and_restores() {
        byte[] original = SyntheticFixture.Pfil("[teamdata]\nA,B,C,D,80,F\nA,B,C,D,0,F\n");
        string patched = SyntheticFixture.Text(TeamDatPatcher.GetPatchedBytes(original, new(true)));
        Assert.Contains("A,B,C,D,1600,F", patched);
        Assert.Contains("A,B,C,D,0,F", patched);
        Assert.Equal(original, TeamDatPatcher.GetPatchedBytes(original, new(false)));
    }
}
