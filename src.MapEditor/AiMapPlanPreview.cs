namespace AgainstRomeMapEditor;

internal sealed record AiMapPlanPreview(Bitmap Image, AiMapApplyResult Changes, bool HasCollision);

/// <summary>Runs the same applier against independent layer and corner snapshots without writing files.</summary>
internal static class AiMapPlanPreviewBuilder
{
    internal static AiMapPlanPreview Build(AiMapPlan plan, TerrainHeightEditSession source, TerrainBlendEditSession? materials,
        int dimension, float water)
    {
        var heights = new TerrainHeightEditSession(source.VertexSize, source.Heights.ToArray(), null,
            source.CollisionSize, source.Collision?.ToArray());
        var blend = materials?.Fork();
        var rejected = new List<(float X, float Y, float Radius)>();
        AiMapApplyResult changes = AiMapPlanApplier.Apply(plan, heights, dimension, water,
            (material, x, y, radius) =>
            {
                bool accepted = blend?.PaintCircle(x, y, radius, material, rollbackStrokeOnFailure: false).Succeeded == true;
                if (!accepted) rejected.Add((x, y, radius));
                return accepted;
            });
        var image = new Bitmap(heights.VertexSize, heights.VertexSize);
        try
        {
            for (int y = 0; y < image.Height; y++)
                for (int x = 0; x < image.Width; x++)
                {
                    byte height = heights.Heights[y * image.Width + x];
                    Color color = height < water ? Color.FromArgb(35, 95, 170) : Color.FromArgb(65 + height / 2, 155 - height / 4, 55 + height / 4);
                    if (blend is not null && materials is not null)
                    {
                        int tx = Math.Min(dimension - 1, x * dimension / image.Width);
                        int ty = Math.Min(dimension - 1, y * dimension / image.Height);
                        int tile = ty * dimension + tx;
                        if (x % 4 == 0 && y % 4 == 0 && !blend.CurrentTextures[tile].Equals(materials.CurrentTextures[tile], StringComparison.OrdinalIgnoreCase))
                            color = Color.FromArgb(215, 145, 45);
                    }
                    if (x % 4 == 0 && y % 4 == 0 && rejected.Any(area =>
                        MathF.Pow(x * dimension / (float)image.Width - area.X, 2) + MathF.Pow(y * dimension / (float)image.Height - area.Y, 2) <= area.Radius * area.Radius))
                        color = Color.FromArgb(180, 60, 200);
                    if (heights.HasCollision)
                    {
                        int cx = Math.Min(heights.CollisionSize - 1, x * heights.CollisionSize / image.Width);
                        int cy = Math.Min(heights.CollisionSize - 1, y * heights.CollisionSize / image.Height);
                        if (heights.Collision![cy * heights.CollisionSize + cx] > 0) color = Color.FromArgb(195, color.G / 2, color.B / 2);
                    }
                    image.SetPixel(x, y, color);
                }
            return new(image, changes, heights.HasCollision);
        }
        catch { image.Dispose(); throw; }
    }
}
