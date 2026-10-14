using System.Numerics;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed record ParticleSpriteDraw(SavedParticlePreviewImage Source, float Depth, float[] Vertices);
internal sealed record ParticleSpriteBatch(NativeSpriteAtlas? Atlas, IReadOnlyList<ParticleSpriteDraw> Draws,
    IReadOnlyList<string> Diagnostics);

/// <summary>Detached renderer input. Leaves mode-specific GL state and native
/// visibility/palette preparation to the draw consumer.</summary>
internal static class SceneParticleRenderer
{
    internal static ParticleSpriteBatch Build(IReadOnlyList<SavedParticlePreviewImage> images,
        TerrainHeightField heights, Matrix4x4 view)
    {
        var diagnostics = new List<string>();
        var sprites = new Dictionary<NativeParticleAssetFrame, NativeSprite>(ReferenceEqualityComparer.Instance);
        var accepted = new List<(SavedParticlePreviewImage Source, Vector3 Anchor, float Depth, NativeSprite Sprite)>();
        foreach (var source in images)
        {
            string? reason = source.Diagnostic ?? source.GeometryDiagnostic ?? source.ModeDiagnostic;
            if (reason is not null || source.Image is null || source.Rect is null || source.Mode?.LeafAddress is null)
            { diagnostics.Add(reason ?? "Particle renderer input is incomplete."); continue; }
            var child = source.Candidate.Child;
            if (!heights.TryNativeHeight(child.Y, out float y))
            { diagnostics.Add("Particle native height conversion is unavailable."); continue; }
            const float worldUnitsPerTile = SdlSceneCatalog.WorldUnitsPerMapPixel * 4f;
            var anchor = new Vector3(child.X / worldUnitsPerTile, y, child.Z / worldUnitsPerTile);
            float depth = -Vector3.Transform(anchor, view).Z;
            if (!float.IsFinite(depth))
            { diagnostics.Add("Particle view depth is invalid."); continue; }
            if (depth <= 0) continue;
            if (!sprites.TryGetValue(source.Image, out var sprite))
            {
                sprite = new(source.Image.Image.Width, source.Image.Image.Height,
                    source.Image.Image.ArgbPixels.ToArray(), 0, 0, source.Image.AssetName);
                sprites.Add(source.Image, sprite);
            }
            accepted.Add((source, anchor, depth, sprite));
        }
        if (accepted.Count == 0) return new(null, Array.Empty<ParticleSpriteDraw>(), diagnostics.AsReadOnly());
        var atlas = NativeSpriteAtlas.Pack(sprites.Values);
        var draws = new List<ParticleSpriteDraw>();
        var order = RustAlrFrameDecoder.OrderSceneDraws(accepted.Select(item => item.Depth).ToArray());
        foreach (uint index in order)
        {
            var item = accepted[(int)index];
            if (!atlas.TryGetUv(item.Sprite, out var uv))
            { diagnostics.Add("Particle frame did not fit the sprite atlas."); continue; }
            var rect = item.Source.Rect!;
            float scale = SceneObjectRenderer.TilesPerSpritePixel;
            var vertices = new float[6 * SceneObjectRenderer.FloatsPerSpriteVertex];
            int offset = 0;
            void Vertex(float x, float y, float u, float v)
            {
                vertices[offset++] = item.Anchor.X; vertices[offset++] = item.Anchor.Y; vertices[offset++] = item.Anchor.Z;
                vertices[offset++] = x * scale; vertices[offset++] = -y * scale;
                vertices[offset++] = u; vertices[offset++] = v;
                vertices[offset++] = item.Source.Candidate.Child.Y;
            }
            Vertex(rect.Left, rect.Bottom, uv.U0, uv.V1); Vertex(rect.Right, rect.Bottom, uv.U1, uv.V1);
            Vertex(rect.Right, rect.Top, uv.U1, uv.V0); Vertex(rect.Left, rect.Bottom, uv.U0, uv.V1);
            Vertex(rect.Right, rect.Top, uv.U1, uv.V0); Vertex(rect.Left, rect.Top, uv.U0, uv.V0);
            draws.Add(new(item.Source, item.Depth, vertices));
        }
        return new(atlas, draws.AsReadOnly(), diagnostics.AsReadOnly());
    }
}
