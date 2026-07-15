using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class TroopPresetCodecTests
{
    private static readonly double[] Fallback = { 100, 10, 20, 30, 2.6, 1500, 500, 900, 500 };

    [Fact]
    public void Parse_supports_legacy_four_six_and_current_nine_column_rows()
    {
        string[] lines =
        {
            "Four=110,11,21,31",
            "Six=120,12,22,32,1600,450",
            "Nine=130,13,23,33,3.2,1700,400,1000,600",
            "Broken=abc,1,2,3",
        };

        TroopPresetParseResult result = TroopPresetCodec.Parse(lines, _ => (double[])Fallback.Clone());

        Assert.Equal(1, result.SkippedLines);
        Assert.Equal(new double[] { 110, 11, 21, 31, 2.6, 1500, 500, 900, 500 }, result.Stats["Four"]);
        Assert.Equal(new double[] { 120, 12, 22, 32, 0, 1600, 450, 0, 0 }, result.Stats["Six"]);
        Assert.Equal(new double[] { 130, 13, 23, 33, 0, 1700, 400, 1000, 600 }, result.Stats["Nine"]);
    }

    [Fact]
    public void Write_and_parse_round_trip_the_six_editable_fields()
    {
        var stats = new Dictionary<string, double[]>
        {
            ["UnitA"] = new double[] { 101.5, 12.25, 33, 44, 2.6, 1550, 475, 900, 500 },
            ["UnitB"] = new double[] { 101.125, 12.3456, 33.075, 44, 2.6, 1550.5, 475.25, 900, 500 },
        };

        string text = TroopPresetCodec.Write(stats, new DateTime(2026, 7, 15, 1, 2, 3));
        TroopPresetParseResult parsed = TroopPresetCodec.Parse(text.Split('\n'), _ => (double[])Fallback.Clone());

        Assert.Contains("# Generated on: 2026-07-15 01:02:03", text);
        Assert.Equal(new double[] { 101.5, 12.25, 33, 44, 0, 1550, 475, 0, 0 }, parsed.Stats["UnitA"]);
        // 匯出不得四捨五入：超過兩位小數的自訂值必須原值往返。
        Assert.Equal(new double[] { 101.125, 12.3456, 33.075, 44, 0, 1550.5, 475.25, 0, 0 }, parsed.Stats["UnitB"]);
    }
}
