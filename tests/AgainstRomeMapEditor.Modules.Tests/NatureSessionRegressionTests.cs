using AgainstRomeMapEditor.Modules.Nature;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NatureSessionRegressionTests
{
    [Fact]
    public void Blank_terrain_removal_clears_stored_removable_and_pending_additions_while_preserving_unremovable_slots()
    {
        // 模擬待存新增物件（不同旋轉角度）
        var tree1 = new NatureAddition(null!, "LanGerLau00", 100, 0, 100, 0.785f);
        var tree2 = new NatureAddition(null!, "LanGerLau00", 200, 0, 200, 1.570f);
        var bush1 = new NatureAddition(null!, "LanGerBus00", 300, 0, 300, 3.141f);

        var session = new NatureEditSession();
        session.Plant(tree1);
        session.Plant(tree2);
        session.Plant(bush1);
        session.CommitStroke();

        Assert.Equal(3, session.Additions.Count);
        Assert.Empty(session.RemovedSlots);
        Assert.True(session.IsDirty);

        // 模擬地圖上的物件：可移除地景槽位 [10, 20, 30]，連結物件 [100]，建築 [200]
        int[] removableLandscapeSlots = [10, 20, 30];
        int[] linkedSlots = [100];
        int[] buildingSlots = [200];

        // Blank terrain 只應移除可移除地景槽位與 pending additions；不應包含 linked / building slots
        bool removed = session.Remove(removableLandscapeSlots, session.Additions);
        Assert.True(removed);
        session.CommitStroke();

        // 驗證 pending additions 已被清空
        Assert.Empty(session.Additions);
        // 驗證可移除地景槽位已標記刪除
        Assert.Equal(new[] { 10, 20, 30 }, session.Capture().RemovedSlots);
        // 驗證連結物件與建築槽位未被標記刪除
        Assert.DoesNotContain(linkedSlots[0], session.RemovedSlots);
        Assert.DoesNotContain(buildingSlots[0], session.RemovedSlots);
        Assert.True(session.IsDirty);
    }

    [Fact]
    public void Undo_after_blank_terrain_restores_exact_addition_order_rotation_and_removed_slots()
    {
        var item1 = new NatureAddition(null!, "TreeA", 10, 0, 10, 0.42f);
        var item2 = new NatureAddition(null!, "TreeB", 20, 0, 20, 1.84f);
        var item3 = new NatureAddition(null!, "RockC", 30, 0, 30, 5.12f);

        var session = new NatureEditSession();
        session.Plant(item1);
        session.Plant(item2);
        session.Plant(item3);
        session.CommitStroke();

        int[] removableSlots = [5, 15];
        session.Remove(removableSlots, session.Additions);
        session.CommitStroke();

        Assert.Empty(session.Additions);
        Assert.Equal(new[] { 5, 15 }, session.Capture().RemovedSlots);

        // 執行 Undo：還原空白地形清除操作
        Assert.True(session.Undo());
        Assert.Empty(session.Capture().RemovedSlots);
        Assert.Equal(3, session.Additions.Count);

        // 驗證還原後的順序、座標與旋轉角度完全一致
        Assert.Equal("TreeA", session.Additions[0].Name);
        Assert.Equal(0.42f, session.Additions[0].Rotation);
        Assert.Equal(10, session.Additions[0].X);

        Assert.Equal("TreeB", session.Additions[1].Name);
        Assert.Equal(1.84f, session.Additions[1].Rotation);
        Assert.Equal(20, session.Additions[1].X);

        Assert.Equal("RockC", session.Additions[2].Name);
        Assert.Equal(5.12f, session.Additions[2].Rotation);
        Assert.Equal(30, session.Additions[2].X);

        // 驗證 Redo 與再次 Undo
        Assert.True(session.Redo());
        Assert.Empty(session.Additions);
        Assert.Equal(new[] { 5, 15 }, session.Capture().RemovedSlots);

        Assert.True(session.Undo());
        Assert.Equal(new[] { item1, item2, item3 }, session.Additions);
        Assert.Empty(session.Capture().RemovedSlots);
    }

    [Fact]
    public void Indexed_additions_history_restores_exact_interleaved_indices_and_rotations()
    {
        // 驗證 NatureEditSession 的 indexed additions history 修復：
        // 刪除中間項後，Undo 必須插回精確的索引位置，避免旋轉或順序錯位
        var a = new NatureAddition(null!, "A", 1, 0, 1, 0.1f);
        var b = new NatureAddition(null!, "B", 2, 0, 2, 0.2f);
        var c = new NatureAddition(null!, "C", 3, 0, 3, 0.3f);
        var d = new NatureAddition(null!, "D", 4, 0, 4, 0.4f);
        var e = new NatureAddition(null!, "E", 5, 0, 5, 0.5f);

        var session = new NatureEditSession();
        session.Plant(a);
        session.Plant(b);
        session.Plant(c);
        session.Plant(d);
        session.Plant(e);
        session.CommitStroke();

        // 刪除中間項 B 與 D
        Assert.True(session.Remove([], [b, d]));
        session.CommitStroke();
        Assert.Equal(new[] { a, c, e }, session.Additions);

        // 新增項 F
        var f = new NatureAddition(null!, "F", 6, 0, 6, 0.6f);
        session.Plant(f);
        session.CommitStroke();
        Assert.Equal(new[] { a, c, e, f }, session.Additions);

        // 逐步 Undo
        Assert.True(session.Undo()); // Undo F
        Assert.Equal(new[] { a, c, e }, session.Additions);

        Assert.True(session.Undo()); // Undo 移除 B, D -> 精確插回索引 1 與 3
        Assert.Equal(new[] { a, b, c, d, e }, session.Additions);
        Assert.Equal(0.1f, session.Additions[0].Rotation);
        Assert.Equal(0.2f, session.Additions[1].Rotation);
        Assert.Equal(0.3f, session.Additions[2].Rotation);
        Assert.Equal(0.4f, session.Additions[3].Rotation);
        Assert.Equal(0.5f, session.Additions[4].Rotation);

        // 逐步 Redo
        Assert.True(session.Redo()); // Redo 移除 B, D
        Assert.Equal(new[] { a, c, e }, session.Additions);

        Assert.True(session.Redo()); // Redo 新增 F
        Assert.Equal(new[] { a, c, e, f }, session.Additions);
    }

    [Fact]
    public void Duplicate_value_additions_are_indexed_distinctly_and_restored_accurately()
    {
        // 相同數值的物件（同一範本、同一旋轉與座標）種植於不同時間
        var dup1 = new NatureAddition(null!, "Same", 10, 0, 10, 1.0f);
        var mid = new NatureAddition(null!, "Other", 20, 0, 20, 2.0f);
        var dup2 = new NatureAddition(null!, "Same", 10, 0, 10, 1.0f);

        var session = new NatureEditSession();
        session.Plant(dup1);
        session.Plant(mid);
        session.Plant(dup2);
        session.CommitStroke();

        // 移除第一個 dup1 與 mid
        session.Remove([], [dup1, mid]);
        session.CommitStroke();

        Assert.Single(session.Additions);
        Assert.Same(dup2, session.Additions[0]);

        // Undo 應正確把 dup1 放回索引 0，mid 放回索引 1
        Assert.True(session.Undo());
        Assert.Equal(3, session.Additions.Count);
        Assert.Same(dup1, session.Additions[0]);
        Assert.Same(mid, session.Additions[1]);
        Assert.Same(dup2, session.Additions[2]);
    }

    [Fact]
    public void Failed_save_preserves_dirty_state_pending_additions_and_undo_stack()
    {
        var session = new NatureEditSession();
        var item = new NatureAddition(null!, "LanGerLau00", 50, 0, 50, 0.99f);
        session.Plant(item);
        session.Remove([7], []);
        session.CommitStroke();

        Assert.True(session.IsDirty);
        Assert.True(session.CanUndo);

        // 模擬儲存失敗：宿主未呼叫 AcceptChanges()
        // 驗證 session 的髒狀態、新增物件與移除槽位完全保留
        Assert.True(session.IsDirty);
        Assert.Single(session.Additions);
        Assert.Equal(0.99f, session.Additions[0].Rotation);
        Assert.Equal(new[] { 7 }, session.Capture().RemovedSlots);
        Assert.True(session.CanUndo);

        // 儲存失敗後使用者依然能進行 Undo 操作
        Assert.True(session.Undo());
        Assert.Empty(session.Additions);
        Assert.Empty(session.Capture().RemovedSlots);
        Assert.False(session.IsDirty);

        // 再次 Redo
        Assert.True(session.Redo());
        Assert.True(session.IsDirty);
        Assert.Single(session.Additions);

        // 當儲存真正成功時呼叫 AcceptChanges()
        session.AcceptChanges();
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        Assert.False(session.CanRedo);
        Assert.Single(session.Additions);
    }
}
