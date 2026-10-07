using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed class ScenarioEventDialogTests
{
    [Fact]
    public void Conditions_are_edited_without_mutating_seed_and_saved_with_persistent_target()
    {
        InSta(() =>
        {
            Guid id = Guid.NewGuid();
            var seed = new ScenarioEvent("Conditions") { Actions = [new(ScenarioActionKind.Message, "Ready")],
                Conditions = Enumerable.Repeat(new ScenarioCondition(ScenarioConditionKind.ObjectExists, id), 32).ToList() };
            using var dialog = new ScenarioEventDialog(seed, [], true, targets: [new("HOUSE", 4000, 5000, 0) { Id = id }]);
            _ = dialog.Handle;
            Assert.False(Buttons(dialog).Single(button => button.Text == "Add condition").Enabled);
            Field<ListBox>(dialog, "_conditionList").SelectedIndex = 0;
            Click(Buttons(dialog).Single(button => button.Text == "Delete condition"));
            Assert.True(Buttons(dialog).Single(button => button.Text == "Add condition").Enabled);
            Assert.Equal(32, seed.Conditions.Count);
            Click(Buttons(dialog).Single(button => button.Text == "OK"));
            Assert.Equal(31, dialog.Result!.Conditions.Count);
            Assert.All(dialog.Result.Conditions, condition => Assert.Equal(id, condition.TargetId));
            Assert.NotSame(seed.Conditions, dialog.Result.Conditions);
        });
    }

    [Fact]
    public void Condition_selector_preserves_missing_target_until_user_selects_replacement()
    {
        InSta(() =>
        {
            Guid missing = Guid.NewGuid(), replacement = Guid.NewGuid();
            using var dialog = new ScenarioConditionDialog(new(ScenarioConditionKind.ObjectDeadOrRemoved, missing),
                [new("HOUSE", 4000, 5000, 0) { Id = replacement }], true);
            _ = dialog.Handle;
            ComboBox[] combos = ControlsOf<ComboBox>(dialog).ToArray();
            ComboBox target = combos.Single(combo => combo.Items.Cast<object>().Any(item => item.ToString()!.Contains("Missing target")));
            Assert.Contains("Missing target", target.SelectedItem!.ToString());
            target.SelectedIndex = 0;
            Click(Buttons(dialog).Single(button => button.Text == "OK"));
            Assert.Equal(replacement, dialog.Result!.TargetId);
            Assert.Equal(ScenarioConditionKind.ObjectDeadOrRemoved, dialog.Result.Kind);
        });
    }

    private static IEnumerable<T> ControlsOf<T>(Control parent) where T : Control => parent.Controls.Cast<Control>()
        .SelectMany(child => (child is T match ? new[] { match } : Array.Empty<T>()).Concat(ControlsOf<T>(child)));

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
