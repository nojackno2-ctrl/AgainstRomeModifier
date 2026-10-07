using AgainstRomeMapEditor.Modules.Nature;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NatureEditSessionTests
{
    [Fact]
    public void Removing_middle_additions_and_equal_values_restores_exact_order_and_baseline()
    {
        // 模組不讀取範本 bytes，fixture 僅測試待存座標與歷史。
        var a = new NatureAddition(null!, "A", 1, 0, 1, 0);
        var b = new NatureAddition(null!, "B", 2, 0, 2, 1);
        var c = new NatureAddition(null!, "C", 3, 0, 3, 2);
        var session = new NatureEditSession(); session.Load(new([], [a, b, c]));
        session.Remove([], [b]); session.Undo();
        Assert.Equal(new[] { a, b, c }, session.Capture().Additions); Assert.False(session.IsDirty);
        session.Redo(); Assert.Equal(new[] { a, c }, session.Capture().Additions);
        session.Undo(); session.Plant(b); session.Undo();
        Assert.Equal(new[] { a, b, c }, session.Capture().Additions); Assert.False(session.IsDirty);
        session.Redo(); Assert.Equal(new[] { a, b, c, b }, session.Capture().Additions);
    }

    [Fact]
    public void Pending_stroke_is_undoable_and_new_edit_discards_redo()
    {
        var session = new NatureEditSession();
        Assert.True(session.Remove([1, 2, 2], []));
        Assert.True(session.Remove([2, 3], []));
        Assert.True(session.CanUndo); Assert.True(session.IsDirty);
        Assert.True(session.Undo()); Assert.Empty(session.Capture().RemovedSlots);
        Assert.False(session.IsDirty); Assert.True(session.CanRedo);
        Assert.True(session.Redo()); Assert.Equal(new[] { 1, 2, 3 }, session.Capture().RemovedSlots);
        session.Undo(); session.Remove([4], []);
        Assert.False(session.CanRedo); Assert.False(session.Redo());
        Assert.Equal(new[] { 4 }, session.Capture().RemovedSlots);
    }

    [Fact]
    public void Snapshots_reset_and_successful_save_keep_independent_baselines()
    {
        int[] input = [1]; var session = new NatureEditSession(); session.Load(new(input, []));
        input[0] = 9;
        Assert.Equal(new[] { 1 }, session.Capture().RemovedSlots);
        ((int[])session.Capture().RemovedSlots)[0] = 8;
        session.Remove([2], []); Assert.True(session.IsDirty);
        session.Reset(); Assert.Equal(new[] { 1 }, session.Capture().RemovedSlots);
        Assert.False(session.CanUndo); Assert.False(session.IsDirty);
        session.Remove([3], []); session.AcceptChanges();
        Assert.False(session.IsDirty); Assert.False(session.CanUndo);
        session.Remove([4], []); session.Reset();
        Assert.Equal(new[] { 1, 3 }, session.Capture().RemovedSlots);
        session.Clear(); Assert.Empty(session.Capture().RemovedSlots); Assert.False(session.IsDirty);
    }

    [Fact]
    public void Empty_removal_preserves_redo()
    {
        var session = new NatureEditSession(); session.Remove([1], []); session.Undo();
        Assert.False(session.Remove([], [])); Assert.True(session.CanRedo);
    }
}
