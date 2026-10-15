using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class AiEditorOperatorTests
{
    private static TerrainHeightEditSession CreateSession(int dimension = 64, byte initialHeight = 100)
    {
        int vertexSize = dimension * 4 + 1;
        int collisionSize = dimension * 4;
        return new TerrainHeightEditSession(
            vertexSize,
            Enumerable.Repeat(initialHeight, vertexSize * vertexSize).ToArray(),
            null,
            collisionSize,
            new byte[collisionSize * collisionSize]);
    }

    [Fact]
    public void Operator_produces_only_brush_strokes_with_valid_parameters()
    {
        var session = CreateSession();
        var plan = new AiMapPlan
        {
            BaseHeight = 80,
            BaseMaterial = "Grass",
            Features =
            [
                new() { Type = "hill", X = 20, Y = 20, Radius = 6, Amount = 30 },
                new() { Type = "lake", X = 40, Y = 40, Radius = 5 },
                new() { Type = "river", X = 10, Y = 10, X2 = 50, Y2 = 10, Radius = 2 },
                new() { Type = "blocked", X = 5, Y = 5, X2 = 15, Y2 = 5, Radius = 1 },
                new() { Type = "material", Material = "Rock", X = 30, Y = 30, Radius = 4 }
            ]
        };

        var result = AiEditorOperator.Apply(plan, session, null, 64, 50f, commitPerStroke: false, (_, _, _, _) => true);

        Assert.True(result.OperationCount > 0);
        Assert.NotEmpty(result.OperationLog);

        int[] allowedBrushSizes = [1, 3, 5, 9, 15];
        foreach (string logLine in result.OperationLog)
        {
            // Each log line starts with [操作 X] ... -> /brush <tool> ...
            Assert.Contains("/brush", logLine);
            string command = logLine[(logLine.IndexOf("/brush", StringComparison.Ordinal))..];
            string[] parts = command.Split(' ', StringSplitOptions.RemoveEmptyEntries);
            Assert.True(parts.Length >= 2);
            string tool = parts[1];
            Assert.Contains(tool, new[] { "height", "collision", "material" });

            int sizeIndex = Array.IndexOf(parts, "--size");
            if (sizeIndex >= 0 && sizeIndex + 1 < parts.Length)
            {
                int size = int.Parse(parts[sizeIndex + 1]);
                Assert.Contains(size, allowedBrushSizes);
            }
        }
    }

    [Fact]
    public void Hill_raises_center_by_approximately_amount()
    {
        var session = CreateSession(64, 100);
        int cx = 32, cy = 32;
        int centerVertex = (cy * 4 + 2) * 257 + cx * 4 + 2;
        int initialH = session.Heights[centerVertex];

        var plan = new AiMapPlan
        {
            Features = [new() { Type = "hill", X = cx, Y = cy, Radius = 8, Amount = 40 }]
        };

        var result = AiEditorOperator.Apply(plan, session, null, 64, 50f);

        int finalH = session.Heights[centerVertex];
        int gained = finalH - initialH;
        Assert.InRange(gained, 36, 44);
        Assert.True(result.HeightSamplesChanged > 0);
    }

    [Fact]
    public void Lake_creates_samples_below_water_level()
    {
        var session = CreateSession(64, 100);
        float waterLevel = 60f;
        int cx = 25, cy = 25;
        int centerVertex = (cy * 4 + 2) * 257 + cx * 4 + 2;

        var plan = new AiMapPlan
        {
            Features = [new() { Type = "lake", X = cx, Y = cy, Radius = 6 }]
        };

        var result = AiEditorOperator.Apply(plan, session, null, 64, waterLevel);

        int centerHeight = session.Heights[centerVertex];
        Assert.True(centerHeight < (int)waterLevel, $"Center height {centerHeight} should be below water level {waterLevel}");
        Assert.True(result.HeightSamplesChanged > 0);
    }

    [Fact]
    public void Log_is_non_empty_and_records_brush_commands()
    {
        var session = CreateSession();
        var plan = new AiMapPlan
        {
            Features =
            [
                new() { Type = "ridge", X = 10, Y = 10, X2 = 20, Y2 = 20, Radius = 4, Amount = 25 },
                new() { Type = "passable", X = 10, Y = 10, X2 = 20, Y2 = 10, Radius = 1 }
            ]
        };

        var result = AiEditorOperator.Apply(plan, session, null, 64, 50f);

        Assert.NotEmpty(result.OperationLog);
        Assert.Equal(result.OperationCount, result.OperationLog.Count);
        Assert.All(result.OperationLog, line =>
        {
            Assert.Contains("/brush", line);
            Assert.False(string.IsNullOrWhiteSpace(line));
        });
    }

    [Fact]
    public void Scope_is_strictly_respected_and_outside_tiles_are_untouched()
    {
        var session = CreateSession(64, 100);
        var scope = AiMapEditScope.Parse("20,20,20,20", "28,28,4,4"); // Only (20,20)-(40,40) allowed, minus hole at (28,28)-(32,32)

        var plan = new AiMapPlan
        {
            EditScope = scope,
            BaseHeight = 150,
            Features =
            [
                new() { Type = "hill", X = 30, Y = 30, Radius = 15, Amount = 50 },
                new() { Type = "lake", X = 10, Y = 10, Radius = 8 } // outside scope!
            ]
        };

        var result = AiEditorOperator.Apply(plan, session, null, 64, 50f);

        // Outside edit scope (e.g. at 5, 5 or 10, 10), vertices should remain 100
        int outsideVertex = (10 * 4 + 2) * 257 + 10 * 4 + 2;
        Assert.Equal(100, session.Heights[outsideVertex]);

        // Locked hole at 30, 30: vertex (30*4+2, 30*4+2) is tile (30, 30) which is locked!
        int lockedVertex = (30 * 4 + 2) * 257 + 30 * 4 + 2;
        Assert.Equal(100, session.Heights[lockedVertex]);

        // Inside allowed area (e.g. tile 22, 22), height should be modified
        int insideVertex = (22 * 4 + 2) * 257 + 22 * 4 + 2;
        Assert.NotEqual(100, session.Heights[insideVertex]);
    }
}
