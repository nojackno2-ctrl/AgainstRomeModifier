using System.Windows.Forms;
using AgainstRomeMapEditor;
using static AgainstRomeModifier.Tests.DialogControlTestSupport;

namespace AgainstRomeModifier.Tests;

public sealed class PlacementBatchEditDialogTests
{
    [Theory]
    [InlineData(true, 0, 7)]
    [InlineData(false, -1, 15)]
    public void Team_range_and_results_depend_on_whether_selection_includes_units(bool includesUnits, int minimum, int maximum) => InSta(() =>
    {
        using var dialog = new PlacementBatchEditDialog(3, includesUnits);
        var grid = Assert.IsType<TableLayoutPanel>(Assert.Single(dialog.Controls.Cast<Control>()));
        var team = Assert.IsType<NumericUpDown>(grid.GetControlFromPosition(1, 1));
        Assert.Equal((decimal)minimum, team.Minimum);
        Assert.Equal((decimal)maximum, team.Maximum);
        dialog.Team = minimum;
        Assert.Equal(minimum, dialog.Team);
        dialog.Team = maximum;
        Assert.Equal(maximum, dialog.Team);
        Assert.Throws<ArgumentOutOfRangeException>(() => team.Value = minimum - 1);
        Assert.Throws<ArgumentOutOfRangeException>(() => team.Value = maximum + 1);
    });

    [Fact]
    public void Checkboxes_enable_only_their_fields_and_control_optional_results() => InSta(() =>
    {
        using var dialog = new PlacementBatchEditDialog(2, true);
        var grid = Assert.IsType<TableLayoutPanel>(Assert.Single(dialog.Controls.Cast<Control>()));
        var changeTeam = Assert.IsType<CheckBox>(grid.GetControlFromPosition(0, 1));
        var changeAngle = Assert.IsType<CheckBox>(grid.GetControlFromPosition(0, 2));
        var team = Assert.IsType<NumericUpDown>(grid.GetControlFromPosition(1, 1));
        var angle = Assert.IsType<NumericUpDown>(grid.GetControlFromPosition(1, 2));
        var apply = Assert.IsType<Button>(dialog.AcceptButton);
        Assert.Null(dialog.Team);
        Assert.Null(dialog.Angle);
        Assert.False(team.Enabled);
        Assert.False(angle.Enabled);
        Assert.False(apply.Enabled);

        changeTeam.Checked = true;
        team.Value = 6;
        Assert.True(team.Enabled);
        Assert.False(angle.Enabled);
        Assert.True(apply.Enabled);
        Assert.Equal(6, dialog.Team);
        Assert.Null(dialog.Angle);

        changeAngle.Checked = true;
        angle.Value = 315;
        Assert.True(angle.Enabled);
        Assert.Equal(315f, dialog.Angle);
        changeTeam.Checked = false;
        Assert.False(team.Enabled);
        Assert.Null(dialog.Team);
        Assert.True(apply.Enabled);
        changeAngle.Checked = false;
        Assert.False(angle.Enabled);
        Assert.Null(dialog.Angle);
        Assert.False(apply.Enabled);

        dialog.Team = 4;
        dialog.Angle = 359;
        Assert.True(changeTeam.Checked);
        Assert.True(changeAngle.Checked);
        Assert.True(team.Enabled);
        Assert.True(angle.Enabled);
        Assert.Equal(4m, team.Value);
        Assert.Equal(359m, angle.Value);
        dialog.Team = null;
        dialog.Angle = null;
        Assert.False(changeTeam.Checked);
        Assert.False(changeAngle.Checked);
        Assert.False(apply.Enabled);
    });
}

