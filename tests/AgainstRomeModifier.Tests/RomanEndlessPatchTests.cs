using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Features.Map;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class RomanEndlessPatchTests
{
    private const string OriginalText =
        "[teamdata]\r\n" +
        "0,GER,35,35,1600,0\r\n" +
        "1,HUN,35,35,1200,1\r\n" +
        "2,KEL,35,35,1000,2\r\n" +
        "3,GER,35,35,800,3\r\n" +
        "4,HUN,35,35,600,4\r\n" +
        "5,KEL,35,35,400,5\r\n" +
        "6,GER,35,35,200,6\r\n" +
        "7,HUN,35,35,0,7\r\n" +
        "[teamskills_0]\r\n" +
        "foo=bar\r\n" +
        "[maxteamobjgenerell]\r\n" +
        "baz=qux\r\n";

    [Fact]
    public void Roman_player_changes_only_team_zero_faction_and_preserves_other_sections()
    {
        byte[] original = SyntheticFixture.Pfil(OriginalText);
        byte[] patched = TeamDatPatcher.GetPatchedBytes(original, new TeamDatOptions(false, RomanPlayer: true));
        string text = SyntheticFixture.Text(patched);

        Assert.Equal(OriginalText.Replace("0,GER,35,35,1600,0", "0,ROM,35,35,1600,0", StringComparison.Ordinal), text);
        Assert.Contains("1,HUN,35,35,1200,1", text, StringComparison.Ordinal);
        Assert.Contains("2,KEL,35,35,1000,2", text, StringComparison.Ordinal);
        Assert.Contains("[teamskills_0]\r\nfoo=bar\r\n[maxteamobjgenerell]\r\nbaz=qux", text, StringComparison.Ordinal);
    }

    [Fact]
    public void Disabled_options_are_byte_exact_idempotent()
    {
        byte[] original = SyntheticFixture.Pfil(OriginalText);
        byte[] result = TeamDatPatcher.GetPatchedBytes(original, new TeamDatOptions(false, RomanPlayer: false));

        Assert.Equal(original, result);
    }

    [Fact]
    public void Roman_player_and_max_population_compose_in_one_patch()
    {
        byte[] original = SyntheticFixture.Pfil(OriginalText);
        string text = SyntheticFixture.Text(TeamDatPatcher.GetPatchedBytes(original,
            new TeamDatOptions(true, RomanPlayer: true)));

        Assert.Contains("0,ROM,35,35,1600,0", text, StringComparison.Ordinal);
        Assert.Contains("1,HUN,35,35,1600,1", text, StringComparison.Ordinal);
        Assert.Contains("6,GER,35,35,1600,6", text, StringComparison.Ordinal);
        Assert.Contains("7,HUN,35,35,0,7", text, StringComparison.Ordinal);
    }

    [Theory]
    [InlineData(false, false)]
    [InlineData(true, false)]
    [InlineData(false, true)]
    [InlineData(true, true)]
    public void Detector_recovers_each_valid_combination(bool maxPopulation, bool romanPlayer)
    {
        byte[] original = SyntheticFixture.Pfil(OriginalText);
        byte[] current = TeamDatPatcher.GetPatchedBytes(original,
            new TeamDatOptions(maxPopulation, RomanPlayer: romanPlayer));

        Assert.True(TeamDatFeatureDetector.TryDetectOptions(original, current, isEndless: true,
            out bool detectedMaxPopulation, out bool detectedRomanPlayer));
        Assert.Equal(maxPopulation, detectedMaxPopulation);
        Assert.Equal(romanPlayer, detectedRomanPlayer);
    }

    [Fact]
    public void Builder_does_not_apply_roman_player_to_non_endless_maps()
    {
        byte[] original = SyntheticFixture.Pfil(OriginalText);
        var backup = new BackupManager(new NullLogger());
        backup.SetBackupFile("MAPS/ENDL_000/DATA/team.dat", original);
        backup.SetBackupFile("MAPS/HIST_000/DATA/team.dat", original);

        Dictionary<string, byte[]> result = MaxPopulationFeature.Build(backup, maxPopulation: false, romanEndless: true);

        Assert.NotEqual(original, result["MAPS/ENDL_000/DATA/team.dat"]);
        Assert.Equal(original, result["MAPS/HIST_000/DATA/team.dat"]);
    }

    [RequiresBackupZipFact]
    public void Real_endless_team_data_changes_exactly_one_faction_row_per_map()
    {
        using var fixture = BackupZipGameFixture.Create();
        var endlessFiles = fixture.Backup.BackupFiles
            .Where(item => item.Key.StartsWith("MAPS/ENDL_", StringComparison.OrdinalIgnoreCase) &&
                           item.Key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase))
            .ToArray();

        Assert.Equal(5, endlessFiles.Length);
        foreach (var item in endlessFiles)
        {
            string original = SyntheticFixture.Text(item.Value);
            string patched = SyntheticFixture.Text(TeamDatPatcher.GetPatchedBytes(item.Value,
                new TeamDatOptions(false, RomanPlayer: true)));
            string[] originalLines = original.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);
            string[] patchedLines = patched.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None);

            Assert.Equal(originalLines.Length, patchedLines.Length);
            int changedLines = originalLines.Zip(patchedLines).Count(pair => pair.First != pair.Second);
            Assert.Equal(1, changedLines);
            Assert.Contains(patchedLines, line => line.StartsWith("0," + TeamDatPatcher.RomanFactionToken + ",", StringComparison.Ordinal));
        }
    }
}
