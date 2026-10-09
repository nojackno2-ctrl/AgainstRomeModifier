using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Settlement;
using static AgainstRomeModifier.Tests.DialogControlTestSupport;

namespace AgainstRomeModifier.Tests;

public sealed class SettlementGeneratorDialogTests
{
    [Fact]
    public void Initial_values_and_edited_results_match_the_controls() => InSta(() =>
    {
        using var dialog = new SettlementGeneratorDialog();
        Assert.Equal(2, dialog.PlayerCount);
        Assert.Equal((SettlementTribe)0, dialog.SelectedTribe);
        Assert.Equal(42, dialog.Seed);
        Assert.True(dialog.IncludeNature);
        var tribe = Assert.Single(Descendants<ComboBox>(dialog));
        Assert.Equal(4, tribe.Items.Count);
        Assert.Equal(0, tribe.SelectedIndex);
        for (int index = 0; index < 4; index++)
        {
            tribe.SelectedIndex = index;
            Assert.Equal((SettlementTribe)index, dialog.SelectedTribe);
            dialog.SelectedTribe = (SettlementTribe)((index + 1) % 4);
            Assert.Equal((index + 1) % 4, tribe.SelectedIndex);
        }
        var nature = Assert.Single(Descendants<CheckBox>(dialog));
        nature.Checked = false;
        Assert.False(dialog.IncludeNature);
        dialog.IncludeNature = true;
        Assert.True(nature.Checked);
        dialog.Seed = 123456;
        Assert.Equal(123456, dialog.Seed);
    });

    [Theory]
    [InlineData(int.MinValue, 2)]
    [InlineData(1, 2)]
    [InlineData(2, 2)]
    [InlineData(5, 5)]
    [InlineData(8, 8)]
    [InlineData(9, 8)]
    [InlineData(int.MaxValue, 8)]
    public void Player_count_is_clamped_to_two_through_eight(int value, int expected) => InSta(() =>
    {
        using var dialog = new SettlementGeneratorDialog();
        var count = Assert.Single(Descendants<NumericUpDown>(dialog), n => n.Minimum == 2);
        Assert.Equal(2m, count.Minimum);
        Assert.Equal(8m, count.Maximum);
        dialog.PlayerCount = value;
        Assert.Equal(expected, dialog.PlayerCount);
        Assert.Equal((decimal)expected, count.Value);
        count.Value = 7;
        Assert.Equal(7, dialog.PlayerCount);
    });

    [Fact]
    public void Random_seed_button_updates_seed_within_its_generation_range() => InSta(() =>
    {
        using var dialog = new SettlementGeneratorDialog();
        ShowOffscreen(dialog);
        // Locate the public button in the seed row without language-dependent text.
        var randomize = Assert.Single(Descendants<Button>(dialog), b => b.DialogResult == DialogResult.None);
        for (int attempt = 0; attempt < 5; attempt++)
        {
            // Zero is outside Random.Next's result range, avoiding probabilistic inequality checks.
            dialog.Seed = 0;
            randomize.PerformClick();
            Assert.InRange(dialog.Seed, 1, 99999);
            Assert.Equal((decimal)dialog.Seed,
                Assert.Single(Descendants<NumericUpDown>(dialog), n => n.Maximum == 999999).Value);
        }
    });
}

