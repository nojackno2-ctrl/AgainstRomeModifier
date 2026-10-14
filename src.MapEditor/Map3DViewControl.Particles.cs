using OpenTK.Graphics.OpenGL4;
using Matrix4 = OpenTK.Mathematics.Matrix4;

namespace AgainstRomeMapEditor;

internal sealed partial class Map3DViewControl
{
    private IReadOnlyList<SavedParticlePreviewImage> _particleImages = Array.Empty<SavedParticlePreviewImage>();
    private int _particleProgram, _particleTexture, _particleVbo;
    private bool _particlesDrawnWithObjects;
    private ParticleSpriteBatch? _particleBatch;
    private float[] _particleReplacementVertices = Array.Empty<float>();
    private TerrainHeightField? _particleBatchHeights;
    private System.Numerics.Matrix4x4 _particleBatchView;
    private float _particleBatchRelief;
    private bool _particleTextureDirty = true;
    internal int ParticleBatchBuildCount { get; private set; }
    internal int ParticleTextureUploadCount { get; private set; }
    internal int LastParticleDrawCount { get; private set; }
    internal int LastParticleSubmissionCount { get; private set; }
    internal IReadOnlyList<string> ParticleDrawDiagnostics { get; private set; } = Array.Empty<string>();

    internal void UpdateSavedParticleImages(IReadOnlyList<SavedParticlePreviewImage> images)
    {
        _particleImages = images.ToArray();
        _particleBatch = null;
        _particleReplacementVertices = Array.Empty<float>();
        _particleBatchHeights = null;
        _particleTextureDirty = true;
        LastParticleDrawCount = 0;
        LastParticleSubmissionCount = 0;
        ParticleDrawDiagnostics = Array.Empty<string>();
        Invalidate();
    }

    private bool DrawParticleSprites(Matrix4 view, Matrix4 projection, float[]? objectVertices = null)
    {
        if (objectVertices is null && _particlesDrawnWithObjects) return true;
        LastParticleDrawCount = 0;
        LastParticleSubmissionCount = 0;
        if (_heights is null || _particleImages.Count == 0) return false;
        var cameraView = _camera.GetViewMatrix();
        if (_particleBatch is null || !ReferenceEquals(_particleBatchHeights, _heights)
            || _particleBatchView != cameraView || _particleBatchRelief != _heights.HeightScale)
        {
            _particleBatch = SceneParticleRenderer.Build(_particleImages, _heights, cameraView);
            _particleReplacementVertices = _particleBatch.Draws.Where(draw => draw.Source.Mode?.ResolvedMode == 0)
                .SelectMany(draw => draw.Vertices).ToArray();
            _particleBatchHeights = _heights; _particleBatchView = cameraView;
            _particleBatchRelief = _heights.HeightScale;
            _particleTextureDirty = true; ParticleBatchBuildCount++;
        }
        var batch = _particleBatch;
        var diagnostics = batch.Diagnostics.ToList();
        var replacement = batch.Draws.Where(draw => draw.Source.Mode?.ResolvedMode == 0).ToArray();
        foreach (var draw in batch.Draws.Where(draw => draw.Source.Mode?.ResolvedMode != 0))
            diagnostics.Add($"Particle GL mode {draw.Source.Mode?.ResolvedMode} is not implemented.");
        ParticleDrawDiagnostics = diagnostics.AsReadOnly();
        if (replacement.Length == 0 || batch.Atlas is null) return false;
        _particleProgram = _particleProgram == 0 ? CreateProgram(ParticleVertexShader, ParticleFragmentShader) : _particleProgram;
        if (_particleTexture == 0) _particleTexture = GL.GenTexture();
        if (_particleVbo == 0) _particleVbo = GL.GenBuffer();
        GL.ActiveTexture(TextureUnit.Texture0);
        GL.BindTexture(TextureTarget.Texture2D, _particleTexture);
        if (_particleTextureDirty)
        {
        GL.PixelStore(PixelStoreParameter.UnpackAlignment, 4);
        GL.TexImage2D(TextureTarget.Texture2D, 0, PixelInternalFormat.Rgba, batch.Atlas.Width, batch.Atlas.Height,
            0, PixelFormat.Bgra, PixelType.UnsignedByte, batch.Atlas.ArgbPixels);
        GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int)TextureMinFilter.Nearest);
        GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMagFilter, (int)TextureMagFilter.Nearest);
        GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureWrapS, (int)TextureWrapMode.ClampToEdge);
        GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureWrapT, (int)TextureWrapMode.ClampToEdge);
        _particleTextureDirty = false; ParticleTextureUploadCount++;
        }
        GL.UseProgram(_particleProgram);
        GL.UniformMatrix4(GL.GetUniformLocation(_particleProgram, "uView"), false, ref view);
        GL.UniformMatrix4(GL.GetUniformLocation(_particleProgram, "uProjection"), false, ref projection);
        GL.Uniform1(GL.GetUniformLocation(_particleProgram, "uSprites"), 0);
        bool depth = GL.IsEnabled(EnableCap.DepthTest), cull = GL.IsEnabled(EnableCap.CullFace), blend = GL.IsEnabled(EnableCap.Blend);
        try
        {
            GL.Disable(EnableCap.DepthTest); GL.Disable(EnableCap.CullFace); GL.Disable(EnableCap.Blend);
            // Concatenation preserves the batch's far-to-near primitive order.
            float[] vertices = _particleReplacementVertices;
            GL.BindVertexArray(_spriteVao); GL.BindBuffer(BufferTarget.ArrayBuffer, _particleVbo);
            GL.BufferData(BufferTarget.ArrayBuffer, vertices.Length * sizeof(float), vertices, BufferUsageHint.StreamDraw);
            if (objectVertices is { Length: > 0 })
            {
                GL.BindBuffer(BufferTarget.ArrayBuffer, _spriteVbo);
                GL.BufferData(BufferTarget.ArrayBuffer, objectVertices.Length * sizeof(float), objectVertices, BufferUsageHint.StreamDraw);
                var offsets = new Dictionary<int, int>(); int ordinal = 0;
                for (int index = 0; index < batch.Draws.Count; index++)
                    if (batch.Draws[index].Source.Mode?.ResolvedMode == 0) offsets.Add(index, ordinal++ * 6);
                var plan = SceneCombinedSpritePlan.Build(objectVertices, batch, _camera.GetViewMatrix());
                for (int drawIndex = 0; drawIndex < plan.Count;)
                {
                    var draw = plan[drawIndex++];
                    bool particle = draw.Kind == SceneSpriteDrawKind.ParticleReplacement;
                    int first = particle ? offsets[draw.SourceIndex] : draw.FirstVertex;
                    int count = draw.VertexCount;
                    while (drawIndex < plan.Count && plan[drawIndex].Kind == draw.Kind)
                    {
                        var next = plan[drawIndex];
                        int nextFirst = particle ? offsets[next.SourceIndex] : next.FirstVertex;
                        if (nextFirst != first + count) break;
                        count += next.VertexCount; drawIndex++;
                    }
                    GL.UseProgram(particle ? _particleProgram : _spriteProgram);
                    GL.BindTexture(TextureTarget.Texture2D, particle ? _particleTexture : _spriteTexture);
                    if (particle) GL.Disable(EnableCap.Blend); else GL.Enable(EnableCap.Blend);
                    if (!particle) GL.Uniform1(GL.GetUniformLocation(_spriteProgram, "uAlpha"), 1f);
                    BindSpriteDrawBuffer(particle ? _particleVbo : _spriteVbo);
                    GL.DrawArrays(PrimitiveType.Triangles, first, count);
                    if (particle) { LastParticleDrawCount += count / 6; LastParticleSubmissionCount++; }
                }
            }
            else
            {
                BindSpriteDrawBuffer(_particleVbo);
                GL.DrawArrays(PrimitiveType.Triangles, 0, vertices.Length / SceneObjectRenderer.FloatsPerSpriteVertex);
                LastParticleDrawCount = replacement.Length; LastParticleSubmissionCount = 1;
            }
        }
        finally
        {
            if (depth) GL.Enable(EnableCap.DepthTest); else GL.Disable(EnableCap.DepthTest);
            if (cull) GL.Enable(EnableCap.CullFace); else GL.Disable(EnableCap.CullFace);
            if (blend) GL.Enable(EnableCap.Blend); else GL.Disable(EnableCap.Blend);
        }
        return true;
    }

    private void BindSpriteDrawBuffer(int buffer)
    {
        GL.BindVertexArray(_spriteVao); GL.BindBuffer(BufferTarget.ArrayBuffer, buffer);
        int stride = SceneObjectRenderer.FloatsPerSpriteVertex * sizeof(float);
        GL.EnableVertexAttribArray(0); GL.VertexAttribPointer(0, 3, VertexAttribPointerType.Float, false, stride, 0);
        GL.EnableVertexAttribArray(1); GL.VertexAttribPointer(1, 2, VertexAttribPointerType.Float, false, stride, 3 * sizeof(float));
        GL.EnableVertexAttribArray(2); GL.VertexAttribPointer(2, 2, VertexAttribPointerType.Float, false, stride, 5 * sizeof(float));
        GL.EnableVertexAttribArray(3); GL.VertexAttribPointer(3, 1, VertexAttribPointerType.Float, false, stride, 7 * sizeof(float));
    }

    private void ReleaseParticleGlResources()
    {
        GL.DeleteTexture(_particleTexture); GL.DeleteProgram(_particleProgram);
        GL.DeleteBuffer(_particleVbo); _particleTexture = _particleProgram = _particleVbo = 0;
        _particleTextureDirty = true;
    }

    private const string ParticleVertexShader = "#version 330 core\n" + """
        layout(location=0) in vec3 anchor; layout(location=1) in vec2 offset;
        layout(location=2) in vec2 uv; uniform mat4 uView; uniform mat4 uProjection;
        out vec2 UV; void main(){ vec4 p=uView*vec4(anchor,1); p.xy+=offset;
        gl_Position=uProjection*p; UV=uv; }
        """;
    private const string ParticleFragmentShader = "#version 330 core\n" + """
        in vec2 UV; uniform sampler2D uSprites; out vec4 c;
        void main(){ vec4 t=texture(uSprites,UV); if(t.a==0) discard; c=t; }
        """;
}
