using System.Numerics;
using AgainstRomeMapEditor.NativeAssets;

namespace AgainstRomeMapEditor;

internal enum SceneSpriteDrawKind { Object, ParticleReplacement }
internal sealed record SceneSpriteDraw(SceneSpriteDrawKind Kind, int SourceIndex,
    float[] Vertices, int FirstVertex, int VertexCount, float Depth);

/// <summary>Shared painter plan retaining each atlas/shader source. No GL calls,
/// copies of vertex buffers, native visibility or blend approximations.</summary>
internal static class SceneCombinedSpritePlan
{
    internal static IReadOnlyList<SceneSpriteDraw> Build(float[] objectVertices,
        ParticleSpriteBatch particles, Matrix4x4 view)
    {
        const int quadFloats = 6 * SceneObjectRenderer.FloatsPerSpriteVertex;
        if (objectVertices.Length % quadFloats != 0)
            throw new InvalidDataException("Object sprite buffer does not contain complete quads.");
        var pending = new List<SceneSpriteDraw>();
        for (int index = 0; index < objectVertices.Length / quadFloats; index++)
        {
            int offset = index * quadFloats;
            var anchor = new Vector3(objectVertices[offset], objectVertices[offset + 1], objectVertices[offset + 2]);
            float depth = -Vector3.Transform(anchor, view).Z;
            pending.Add(new(SceneSpriteDrawKind.Object, index, objectVertices, index * 6, 6, depth));
        }
        for (int index = 0; index < particles.Draws.Count; index++)
        {
            var draw = particles.Draws[index];
            if (draw.Source.Mode?.ResolvedMode != 0) continue;
            if (draw.Vertices.Length != quadFloats)
                throw new InvalidDataException("Particle sprite buffer does not contain one complete quad.");
            // Derive all depths with this view rather than trusting a batch from an older camera.
            var anchor = new Vector3(draw.Vertices[0], draw.Vertices[1], draw.Vertices[2]);
            float depth = -Vector3.Transform(anchor, view).Z;
            pending.Add(new(SceneSpriteDrawKind.ParticleReplacement, index, draw.Vertices, 0, 6, depth));
        }
        if (pending.Count == 0) return Array.Empty<SceneSpriteDraw>();
        var order = RustAlrFrameDecoder.OrderSceneDraws(pending.Select(draw => draw.Depth).ToArray());
        return Array.AsReadOnly(order.Select(index => pending[(int)index]).ToArray());
    }
}
