using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class StampCategoryTests
{
    [Theory]
    [InlineData("WEG_H_ROM", 0)]
    [InlineData("H_WEG_ROM", 0)]
    [InlineData("V_WEG_ROM", 0)]
    [InlineData("PFAD_01", 0)]
    [InlineData("PFLASTER_01", 0)]
    [InlineData("LUXUSWEG_01", 0)]
    [InlineData("PLATZ_01", 0)]
    [InlineData("FLUSS_01", 1)]
    [InlineData("ERDEFLUSS_01", 1)]
    [InlineData("fels01_00", 2)]
    [InlineData("Fels_AA_002", 2)]
    [InlineData("ita_fels1", 2)]
    [InlineData("MARMOR_01", 3)]
    [InlineData("STEINBODEN_01", 3)]
    [InlineData("STADT_01", 3)]
    public void Representative_names_have_expected_category_regardless_of_case(string name, int expectedCategory)
    {
        Assert.Equal(expectedCategory, MapEditorForm.StampCategory(name));
        Assert.Equal(expectedCategory, MapEditorForm.StampCategory(name.ToLowerInvariant()));
        Assert.Equal(expectedCategory, MapEditorForm.StampCategory(name.ToUpperInvariant()));
    }

    [Theory]
    [InlineData("", 4)]
    [InlineData("GRAS_01", 4)]
    [InlineData("OTHER_WEG_H_ROM", 4)]
    [InlineData("OTHER_FLUSS_01", 4)]
    [InlineData("OTHER_FELS_01", 4)]
    [InlineData("OTHER_MARMOR_01", 4)]
    [InlineData("L", 4)]
    [InlineData("1", 4)]
    [InlineData("L2_WEG_H_ROM", 5)]
    [InlineData("l05_FELS_01", 5)]
    [InlineData("4U13", 6)]
    public void Other_regional_and_blend_names_do_not_match_embedded_category_prefixes(string name, int expectedCategory)
    {
        Assert.Equal(expectedCategory, MapEditorForm.StampCategory(name));
    }
}
