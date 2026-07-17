using System.Windows.Forms;

namespace AgainstRomeModifier.Tests;

public sealed class SaveManagerFormLayoutTests
{
    [Fact]
    public void Save_manager_layout_remains_visible_at_minimum_size()
    {
        Loc.OverrideLanguageForTesting(Language.TraditionalChinese);
        using var form = new SaveManagerForm(Path.GetTempPath());

        form.Size = form.MinimumSize;
        form.PerformLayout();

        Panel rootContent = Find<Panel>(form, "rootContent");
        TableLayoutPanel contentLayout = Find<TableLayoutPanel>(form, "contentLayout");
        Panel gameCard = Find<Panel>(form, "gameCard");
        Panel backupsCard = Find<Panel>(form, "backupsCard");
        TableLayoutPanel gameActions = Find<TableLayoutPanel>(form, "gameActions");
        TableLayoutPanel backupActions = Find<TableLayoutPanel>(form, "backupActions");
        DataGridView[] grids = Descendants(form).OfType<DataGridView>().ToArray();

        Assert.Equal(2, contentLayout.ColumnCount);
        Assert.Equal(68F, contentLayout.ColumnStyles[0].Width);
        Assert.Equal(32F, contentLayout.ColumnStyles[1].Width);
        Assert.Equal(form.ClientSize.Height, rootContent.Bottom);
        Assert.True(gameCard.ClientRectangle.Contains(gameActions.Bounds));
        Assert.True(backupsCard.ClientRectangle.Contains(backupActions.Bounds));
        Assert.All(gameActions.Controls.OfType<Button>(), button => Assert.True(button.Width >= 100));
        Assert.Equal(2, grids.Length);
        Assert.All(grids, grid => Assert.Equal(DataGridViewAutoSizeColumnsMode.Fill, grid.AutoSizeColumnsMode));
    }

    private static T Find<T>(Control root, string name) where T : Control
    {
        return Assert.IsType<T>(Descendants(root).Single(control => control.Name == name));
    }

    private static IEnumerable<Control> Descendants(Control root)
    {
        foreach (Control child in root.Controls)
        {
            yield return child;
            foreach (Control descendant in Descendants(child))
                yield return descendant;
        }
    }
}
