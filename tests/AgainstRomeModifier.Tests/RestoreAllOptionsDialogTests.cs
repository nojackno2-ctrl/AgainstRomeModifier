using System.Windows.Forms;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Restore_dialog_defaults_to_preserving_maps_and_has_explicit_cancel_and_restore_results()
    {
        RunInSta(() =>
        {
            using var dialog = new RestoreAllOptionsDialog();
            _ = dialog.Handle;
            Assert.True(dialog.PreserveCustomMaps);
            Assert.Equal(DialogResult.Cancel, Assert.IsType<Button>(dialog.CancelButton).DialogResult);
            Assert.Equal(DialogResult.OK, Assert.IsType<Button>(dialog.AcceptButton).DialogResult);
            var checkbox = Descendants(dialog).OfType<CheckBox>().Single();
            Assert.True(checkbox.Checked);
            checkbox.Checked = false;
            Assert.False(dialog.PreserveCustomMaps);
        });
    }

    private static IEnumerable<Control> Descendants(Control control)
    {
        yield return control;
        foreach (Control child in control.Controls)
            foreach (Control item in Descendants(child)) yield return item;
    }
}
