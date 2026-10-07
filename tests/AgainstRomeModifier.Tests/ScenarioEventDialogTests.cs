using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed class ScenarioEventDialogTests
{
    [Fact]
    public void Reordering_actions_preserves_selection_and_seed_until_confirmed()
    {
        InSta(() =>
        {
            var seed = new ScenarioEvent("Ordered", 12, Repeat: true)
                { Actions = [new(ScenarioActionKind.Message, "A"), new(ScenarioActionKind.Message, "B"), new(ScenarioActionKind.Message, "C")] };
            using var dialog = new ScenarioEventDialog(seed, [], true);
            _ = dialog.Handle;
            ListBox list = Field<ListBox>(dialog, "_list");
            Button up = Field<Button>(dialog, "_moveUp"), down = Field<Button>(dialog, "_moveDown");
            Assert.False(up.Enabled); Assert.False(down.Enabled);
            list.SelectedIndex = 1; Click(up); Assert.Equal(0, list.SelectedIndex);
            Assert.False(up.Enabled); Assert.True(down.Enabled);
            Click(down); Click(down); Assert.Equal(2, list.SelectedIndex);
            Assert.False(down.Enabled);
            Assert.Equal(new[] { "A", "B", "C" }, seed.Actions.Select(action => action.Text));
            Click(Buttons(dialog).Single(button => button.Text == "OK"));
            Assert.Equal(DialogResult.OK, dialog.DialogResult);
            Assert.Equal(new[] { "A", "C", "B" }, dialog.Result!.Actions.Select(action => action.Text));
            Assert.Equal(12, dialog.Result.DelaySeconds); Assert.True(dialog.Result.Repeat);
        });
    }

    [Fact]
    public void Full_action_list_disables_add_but_still_allows_reordering()
    {
        InSta(() =>
        {
            using var dialog = new ScenarioEventDialog(new("Full")
                { Actions = Enumerable.Range(0, 32).Select(i => new ScenarioAction(ScenarioActionKind.Message, $"Action {i}")).ToList() }, [], false);
            _ = dialog.Handle;
            Assert.False(Field<Button>(dialog, "_addAction").Enabled);
            Field<ListBox>(dialog, "_list").SelectedIndex = 31;
            Assert.True(Field<Button>(dialog, "_moveUp").Enabled);
            Assert.False(Field<Button>(dialog, "_moveDown").Enabled);
        });
    }

    private static T Field<T>(object target, string name) =>
        (T)target.GetType().GetField(name, BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(target)!;
    private static IEnumerable<Button> Buttons(Control parent) => parent.Controls.Cast<Control>()
        .SelectMany(child => child is Button button ? new[] { button } : Buttons(child));
    private static void Click(Button button) => typeof(Button).GetMethod("OnClick", BindingFlags.NonPublic | BindingFlags.Instance)!
        .Invoke(button, [EventArgs.Empty]);
    private static void InSta(Action action)
    {
        Exception? failure = null;
        var thread = new Thread(() => { try { action(); } catch (Exception ex) { failure = ex; } });
        thread.SetApartmentState(ApartmentState.STA); thread.Start();
        Assert.True(thread.Join(TimeSpan.FromSeconds(60)));
        if (failure is not null) ExceptionDispatchInfo.Capture(failure).Throw();
    }
}
