using System.Numerics;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

/// <summary>Reusable sprite geometry preserving the original float depth ordering, including translation ties.</summary>
internal sealed class SceneSpriteGeometryBuffer
{
    private readonly List<(float Distance, Vector3 Anchor, NativeSprite Sprite, NativeSpriteUv Uv, int ObjectIndex)> _visible = new();
    private float[] _vertices = [];
    private int[] _objectOffsets = [];
    private NativeSprite?[] _previousSprites = [];
    private IReadOnlyList<MapSceneObject>? _objects;
    private IReadOnlyList<NativeSprite?>? _sprites;
    private NativeSpriteAtlas? _atlas;
    private TerrainHeightField? _heights;
    private float _heightScale;
    private Vector3 _depthAxis;
    private float _depthTranslation;
    private bool _valid;

    internal float[] Build(IReadOnlyList<MapSceneObject> objects, IReadOnlyList<NativeSprite?> sprites,
        NativeSpriteAtlas atlas, TerrainHeightField heights, Matrix4x4 view)
    {
        Vector3 depthAxis = new(view.M13, view.M23, view.M33);
        bool same = _valid && ReferenceEquals(objects, _objects) && ReferenceEquals(sprites, _sprites) && ReferenceEquals(atlas, _atlas)
            && ReferenceEquals(heights, _heights) && heights.HeightScale == _heightScale
            && depthAxis == _depthAxis && view.M43 == _depthTranslation
            && sprites.Count == _previousSprites.Length;
        if (same)
        {
            for (int i = 0; i < sprites.Count; i++)
            {
                if (ReferenceEquals(sprites[i], _previousSprites[i])) continue;
                if (i >= objects.Count) { _previousSprites[i] = sprites[i]; continue; }
                bool wasDrawn = _objectOffsets[i] >= 0;
                NativeSpriteUv uv = default;
                bool drawn = sprites[i] is { } sprite && atlas.TryGetUv(sprite, out uv);
                if (wasDrawn != drawn) { same = false; break; }
                if (drawn) WriteQuad(SceneObjectRenderer.GroundPoint(objects[i], heights), sprites[i]!, uv, _objectOffsets[i]);
                _previousSprites[i] = sprites[i];
            }
            if (same) return _vertices;
        }
        _objects = objects; _sprites = sprites; _atlas = atlas; _heights = heights; _heightScale = heights.HeightScale; _depthAxis = depthAxis;
        _depthTranslation = view.M43;
        if (_previousSprites.Length != sprites.Count) _previousSprites = new NativeSprite?[sprites.Count];
        for (int i = 0; i < sprites.Count; i++) _previousSprites[i] = sprites[i];
        if (_objectOffsets.Length != objects.Count) _objectOffsets = new int[objects.Count];
        Array.Fill(_objectOffsets, -1);
        _visible.Clear();
        for (int index = 0; index < objects.Count && index < sprites.Count; index++)
        {
            if (sprites[index] is not { } sprite || !atlas.TryGetUv(sprite, out NativeSpriteUv uv)) continue;
            Vector3 anchor = SceneObjectRenderer.GroundPoint(objects[index], heights);
            _visible.Add((-Vector3.Transform(anchor, view).Z, anchor, sprite, uv, index));
        }
        _visible.Sort(static (a, b) => b.Distance.CompareTo(a.Distance));
        int length = _visible.Count * 6 * SceneObjectRenderer.FloatsPerSpriteVertex;
        if (_vertices.Length != length) _vertices = new float[length];
        int offset = 0;
        foreach (var item in _visible)
        {
            _objectOffsets[item.ObjectIndex] = offset;
            WriteQuad(item.Anchor, item.Sprite, item.Uv, offset);
            offset += 6 * SceneObjectRenderer.FloatsPerSpriteVertex;
        }
        _valid = true;
        return _vertices;
    }
    private void WriteQuad(Vector3 anchor, NativeSprite sprite, NativeSpriteUv uv, int offset)
    {
        float left = -sprite.AnchorX * SceneObjectRenderer.TilesPerSpritePixel, right = (sprite.Width - sprite.AnchorX) * SceneObjectRenderer.TilesPerSpritePixel;
        float top = sprite.AnchorY * SceneObjectRenderer.TilesPerSpritePixel, bottom = (sprite.AnchorY - sprite.Height) * SceneObjectRenderer.TilesPerSpritePixel;
        void Vertex(float ox, float oy, float u, float v)
        {
            _vertices[offset++] = anchor.X; _vertices[offset++] = anchor.Y; _vertices[offset++] = anchor.Z;
            _vertices[offset++] = ox; _vertices[offset++] = oy; _vertices[offset++] = u; _vertices[offset++] = v;
        }
        Vertex(left, bottom, uv.U0, uv.V1); Vertex(right, bottom, uv.U1, uv.V1); Vertex(right, top, uv.U1, uv.V0);
        Vertex(left, bottom, uv.U0, uv.V1); Vertex(right, top, uv.U1, uv.V0); Vertex(left, top, uv.U0, uv.V0);
    }
}

/// <summary>Ground shadow geometry is independent of camera, animation and lighting.</summary>
internal sealed class SceneShadowGeometryCache
{
    private IReadOnlyList<MapSceneObject>? _objects;
    private IReadOnlyList<NativeObjectShadow?>? _shadows;
    private NativeSpriteAtlas? _atlas;
    private TerrainHeightField? _heights;
    private float _heightScale;
    private float[] _vertices = [];
    internal float[] Build(IReadOnlyList<MapSceneObject> objects, IReadOnlyList<NativeObjectShadow?> shadows,
        Func<NativeObjectShadow, NativeSprite?> mask, NativeSpriteAtlas atlas, TerrainHeightField heights)
    {
        if (!ReferenceEquals(objects, _objects) || !ReferenceEquals(shadows, _shadows) || !ReferenceEquals(atlas, _atlas)
            || !ReferenceEquals(heights, _heights) || heights.HeightScale != _heightScale)
        {
            _vertices = SceneObjectRenderer.BuildShadowVertices(objects, shadows, mask, atlas, heights);
            _objects = objects; _shadows = shadows; _atlas = atlas; _heights = heights; _heightScale = heights.HeightScale;
        }
        return _vertices;
    }
}
