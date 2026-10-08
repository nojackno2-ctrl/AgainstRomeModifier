using System.Drawing;
using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class AiMapEditScopeTests
{
    [Fact]
    public void Partial_height_and_collision_preserve_locked_tiles_and_shared_edges_and_undo()
    {
        var scope = AiMapEditScope.Parse("8,8,24,24", "16,16,4,4");
        var session = new TerrainHeightEditSession(129, Enumerable.Repeat((byte)80, 129 * 129).ToArray(), null, 128, new byte[128 * 128]);
        var plan = new AiMapPlan { BaseHeight = 150, EditScope = scope, Features = [new() { Type = "blocked", X = 18, Y = 18, Radius = 32 }] };
        var result = AiMapPlanApplier.Apply(plan, session, 64, 30, (_, _, _, _) => throw new InvalidOperationException());
        Assert.True(result.HeightSamplesChanged > 0); Assert.True(result.CollisionPixelsChanged > 0);
        Assert.Equal(150, session.Heights[24 * 129 + 24]);
        for (int y = 0; y < 129; y++) for (int x = 0; x < 129; x++)
            if (!scope.AllowsVertex(x / 2f, y / 2f, 64)) Assert.Equal(80, session.Heights[y * 129 + x]);
        for (int y = 0; y < 128; y++) for (int x = 0; x < 128; x++)
            if (!scope.AllowsTile(x / 2, y / 2)) Assert.Equal(0, session.Collision![y * 128 + x]);
        byte[] edited = session.Heights.ToArray(); session.CommitStroke(); session.Undo();
        Assert.All(session.Heights, value => Assert.Equal(80, value)); Assert.All(session.Collision!, value => Assert.Equal(0, value));
        session.Redo(); Assert.Equal(edited, session.Heights);
    }

    [Fact]
    public void Protected_passability_ignores_model_blocked_features_even_inside_edit_area()
    {
        var session = new TerrainHeightEditSession(65, new byte[65 * 65], null, 64, new byte[64 * 64]);
        var plan = new AiMapPlan { EditScope = AiMapEditScope.Parse("0,0,64,64", "", false), Features = [new() { Type = "blocked", X = 32, Y = 32, Radius = 20 }] };
        var result = AiMapPlanApplier.Apply(plan, session, 64, 30, (_, _, _, _) => true);
        Assert.Equal(0, result.CollisionPixelsChanged); Assert.All(session.Collision!, value => Assert.Equal(0, value)); Assert.False(session.IsDirty);
    }

    [Fact]
    public void Scope_copies_locks_and_rejects_overflow_or_malformed_rectangles()
    {
        var locks = new[] { new Rectangle(16, 16, 4, 4) };
        var scope = new AiMapEditScope(new(0, 0, 64, 64), locks); locks[0] = new(0, 0, 1, 1);
        Assert.False(scope.AllowsTile(16, 16)); Assert.True(scope.AllowsTile(0, 0));
        foreach (string value in new[] { "1,2,3", "0,0,0,1", "63,63,2,1", "2147483647,0,2,1", "1,1,2147483647,2" })
            Assert.Throws<ArgumentException>(() => AiMapEditScope.Parse(value, ""));
    }
}
