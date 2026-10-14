using System.Buffers.Binary;
using System.Security.Cryptography;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

internal sealed class RequiresMarkerGameFactAttribute : FactAttribute
{
    public RequiresMarkerGameFactAttribute()
    {
        if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("ARM_GAME_PATH")))
            Skip = "Requires opt-in local native maps; no proprietary files in Git.";
    }
}

public sealed class NativeMarkerTypeTableTests
{
    [Theory]
    [InlineData("GER", 0)]
    [InlineData("kel", 1)]
    [InlineData("HUN", 2)]
    [InlineData("ROM", 3)]
    [InlineData("unknown", 0)]
    public void Team_tribe_and_banner_version_select_object_index_zero(string tribe, int tribeIndex)
    {
        string banner = $"[volk{tribeIndex:00}_obdef_bver07]\nBanner\nOtherObject\n";
        var definitions = Definitions(true);
        definitions[456] = new(456, true, 0) { Name = "OtherObject" };
        var result = NativeMarkerTypeTable.Parse(banner, $"[teamdata]\n3,{tribe},35,35,200,7", Engine(99), definitions);
        Assert.Equal(new NativeMarkerTypeSelection(3, tribeIndex, 7, "Banner", 123, 99, 123), result.Selections[3]);
        Assert.Equal(99, result.GetEffectiveType(8)); // native team8/-1 setter is a no-op.
        Assert.Equal(-1, result.GetEffectiveType(-1));
        Assert.Equal(-1, result.GetEffectiveType(9));
    }

    [Theory]
    [InlineData("none", true)]
    [InlineData("MissingName", true)]
    [InlineData("banner", false)]
    public void Invalid_candidate_preserves_saved_type_instead_of_clearing(string alias, bool active)
    {
        var result = NativeMarkerTypeTable.Parse($"[volk00_obdef_bver00]\n{alias}",
            "[teamdata]\n0,GER", Engine(812), Definitions(active));
        Assert.Equal(812, result.GetEffectiveType(0));
        Assert.Equal(alias.Equals("banner", StringComparison.OrdinalIgnoreCase) ? 123 : -1,
            result.Selections[0].CandidateType);
    }

    [Fact]
    public void Defaults_partial_rows_and_repeated_team_rows_preserve_native_field_state()
    {
        var result = NativeMarkerTypeTable.Parse("[volk03_obdef_bver02]\nBANNER",
            "[teamdata]\n1,,,ROM,,,35,35,200,2\n1\n1,ROM\n", Engine(-1), Definitions(true));
        Assert.Equal(3, result.Selections[1].Tribe);
        Assert.Equal(2, result.Selections[1].BannerVersion);
        Assert.Equal(123, result.GetEffectiveType(1));
        Assert.Equal(-1, result.GetEffectiveType(0));
    }

    [Fact]
    public void Unrelated_duplicate_names_are_allowed_requested_duplicate_is_rejected()
    {
        var definitions = Definitions(true);
        definitions[456] = new(456, true, 0) { Name = "Duplicate" };
        definitions[789] = new(789, true, 0) { Name = "duplicate" };
        Assert.Equal(123, NativeMarkerTypeTable.Parse("[volk00_obdef_bver00]\nBanner", "", Engine(-1), definitions).GetEffectiveType(0));
        Assert.Throws<InvalidDataException>(() => NativeMarkerTypeTable.Parse(
            "[volk00_obdef_bver00]\nDuplicate", "", Engine(-1), definitions));
    }

    [Theory]
    [InlineData("0,GER,0,0,0,10")]
    [InlineData("0,GER,0,0,0,-1")]
    [InlineData("0,GER,0,0,0,broken")]
    [InlineData("8,GER")]
    public void Unsafe_selection_is_rejected(string row)
        => Assert.Throws<InvalidDataException>(() => NativeMarkerTypeTable.Parse("", "[teamdata]\n" + row, Engine(0), Definitions(true)));

    [Fact]
    public void Engine_shape_and_duplicate_sections_are_rejected()
    {
        Assert.Throws<InvalidDataException>(() => NativeMarkerTypeTable.Parse("", "", new byte[93], Definitions(true)));
        byte[] engine = Engine(0); engine[0] = 2;
        Assert.Throws<InvalidDataException>(() => NativeMarkerTypeTable.Parse("", "", engine, Definitions(true)));
        Assert.Throws<InvalidDataException>(() => NativeMarkerTypeTable.Parse(
            "[volk00_obdef_bver00]\nBanner\n[VOLK00_OBDEF_BVER00]\nnone", "", Engine(0), Definitions(true)));
    }

    [Fact]
    public void Object_definition_names_survive_partial_overlay_and_remain_detached_from_input()
    {
        string[] columns = Enumerable.Repeat("0", 61).ToArray();
        columns[0] = "123"; columns[1] = "1"; columns[52] = "Banner";
        var definitions = NativeObjectDefinitions.Parse("[ObjectDefaults]\n" + string.Join(',', columns) + "\n123,0");
        Assert.Equal("Banner", definitions[123].Name);
        Assert.False(definitions[123].Active);
    }

    [RequiresMarkerGameFact]
    public void Original_map_tables_match_independent_binary_probe_and_sources_are_unchanged()
    {
        string game = Environment.GetEnvironmentVariable("ARM_GAME_PATH")!;
        var paths = Directory.GetFiles(Path.Combine(game, "SYSTEM", "DATA_MP", "DEFAULTS"), "objdef.d*")
            .Concat(new[] { Path.Combine(game, "SYSTEM", "banner.ini"), Path.Combine(game, "SYSTEM", "clengine.ini") })
            .Concat(Directory.GetFiles(Path.Combine(game, "MAPS"), "engine.dat", SearchOption.AllDirectories))
            .Concat(Directory.GetFiles(Path.Combine(game, "MAPS"), "team.dat", SearchOption.AllDirectories)).ToArray();
        var hashes = paths.ToDictionary(p => p, p => SHA256.HashData(File.ReadAllBytes(p)));
        var definitions = NativeObjectDefinitions.Load(game);
        int maps = 0, same = 0, different = 0;
        foreach (string path in Directory.GetFiles(Path.Combine(game, "MAPS"), "engine.dat", SearchOption.AllDirectories))
        {
            string data = Path.GetDirectoryName(path)!;
            if (!File.Exists(Path.Combine(data, "team.dat"))) continue;
            var table = NativeMarkerTypeTable.Load(game, data, definitions);
            Assert.Equal(0, table.GetEffectiveType(8));
            foreach (var selection in table.Selections.Take(8))
            {
                Assert.InRange(selection.CandidateType, 0, 2499);
                Assert.True(definitions[selection.CandidateType].Active);
                if (selection.CandidateType == selection.SavedType) same++; else different++;
                Assert.Equal(selection.CandidateType, selection.EffectiveType);
            }
            maps++;
        }
        Assert.Equal(71, maps); Assert.Equal(530, same); Assert.Equal(38, different);
        foreach (var (path, hash) in hashes) Assert.Equal(hash, SHA256.HashData(File.ReadAllBytes(path)));
    }

    private static Dictionary<int, NativeObjectDefinition> Definitions(bool active)
        => new() { [123] = new(123, active, 0) { Name = "Banner" } };

    private static byte[] Engine(int saved)
    {
        var bytes = new byte[94]; BinaryPrimitives.WriteInt32LittleEndian(bytes, 1);
        for (int i = 0; i < 9; i++) BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(54 + i * 4), saved);
        return bytes;
    }
}
