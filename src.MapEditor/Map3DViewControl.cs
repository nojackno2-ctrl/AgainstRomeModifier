using System.Drawing.Imaging;
using System.Numerics;
using AgainstRomeModifier.Maps;
using OpenTK.GLControl;
using OpenTK.Graphics.OpenGL4;
using Matrix4 = OpenTK.Mathematics.Matrix4;
using OpenTK.Windowing.Common;

namespace AgainstRomeMapEditor;

internal sealed class Map3DViewControl : GLControl
{
    private readonly EditorCamera _camera = new();
    private readonly HashSet<int> _paintedInDrag = new();
    private string[]? _textures;
    private string[]? _baselineTextures;
    private TerrainHeightField? _heights;
    private FloorTextureLibrary? _library;
    private FloorTextureAtlas? _atlas;
    private TerrainMeshData? _mesh;
    private IReadOnlyList<MapSceneObject> _objects = Array.Empty<MapSceneObject>();
    private int _dimension;
    private float _waterLevel, _heightMapStep;
    private Color _waterSourceColor = Color.SteelBlue;
    private bool _initialized, _painting, _panning, _rotating, _rightClick;
    private Point _lastPointer, _rightStart;
    private int _terrainProgram, _colorProgram, _vao, _vbo, _ebo, _atlasTexture, _waterVao, _waterVbo, _markerVao, _markerVbo;

    public Map3DViewControl() : base(new GLControlSettings { API = ContextAPI.OpenGL, APIVersion = new Version(3, 3), Profile = ContextProfile.Core, Flags = ContextFlags.ForwardCompatible })
    {
        Dock = DockStyle.Fill;
        BackColor = Color.FromArgb(24, 28, 36);
        Cursor = Cursors.Cross;
    }

    public string? BrushTexture { get; set; }
    public int BrushSize { get; set; } = 1;
    public bool EditingEnabled { get; set; }
    public bool ShowGrid { get; set; } = true;
    public bool ShowObjects { get; set; } = true;
    public float ReliefScale { get; private set; } = 1f;
    public bool IsReady => _initialized;
    public event EventHandler<TexturePaintEventArgs>? TexturePainted;
    public event EventHandler<TileHoverEventArgs>? TileHovered;
    public event EventHandler<TextureSampleEventArgs>? TextureSampled;

    public bool LoadTextures(int dimension, IReadOnlyList<string> textures, IReadOnlyList<string> baselineTextures, string mapDirectory, string floorTextureArchivePath, IReadOnlyList<MapSceneObject> sceneObjects, float waterLevel, float heightMapStep, Color waterColor)
    {
        if (!File.Exists(Path.Combine(mapDirectory, "boden.bmp"))) return false;
        _library?.Dispose(); _library = new FloorTextureLibrary(floorTextureArchivePath);
        if (!_library.IsAvailable) return false;
        using var bitmap = new Bitmap(Path.Combine(mapDirectory, "boden.bmp"));
        byte[] samples = ReadSamples(bitmap);
        // A 257x257 source covers the complete 64x64 tile map (four height samples per tile).
        _heights = new TerrainHeightField(bitmap.Width, bitmap.Height, samples, tileWidth: dimension, tileHeight: dimension);
        _dimension = dimension; _textures = textures.ToArray(); _baselineTextures = baselineTextures.ToArray(); _objects = sceneObjects; _waterLevel = waterLevel; _heightMapStep = heightMapStep; _waterSourceColor = waterColor;
        _atlas?.Dispose(); _atlas = FloorTextureAtlas.Create(_textures, _library);
        BuildMesh();
        if (_initialized) UploadResources();
        Invalidate();
        return true;
    }

    public void SetTexture(int x, int y, string texture)
    {
        if (_textures is null || _atlas is null || _mesh is null || x < 0 || x >= _dimension || y < 0 || y >= _dimension) return;
        _textures[y * _dimension + x] = texture;
        Vector2[] uv = _atlas.GetUv(texture);
        int vertexOffset = (y * _dimension + x) * 4;
        for (int index = 0; index < 4; index++) _mesh.Vertices[vertexOffset + index] = _mesh.Vertices[vertexOffset + index] with { TexCoord = uv[index] };
        if (_initialized)
        {
            MakeCurrent(); GL.BindBuffer(BufferTarget.ArrayBuffer, _vbo);
            float[] block = FlattenVertices(_mesh.Vertices.Skip(vertexOffset).Take(4));
            GL.BufferSubData(BufferTarget.ArrayBuffer, (IntPtr)(vertexOffset * 8 * sizeof(float)), block.Length * sizeof(float), block);
        }
        Invalidate();
    }

    protected override void OnLoad(EventArgs e)
    {
        base.OnLoad(e);
        try
        {
            MakeCurrent();
            GL.ClearColor(.07f, .09f, .12f, 1); GL.Enable(EnableCap.DepthTest); GL.Enable(EnableCap.CullFace); GL.Enable(EnableCap.Blend);
            GL.BlendFunc(BlendingFactor.SrcAlpha, BlendingFactor.OneMinusSrcAlpha);
            _terrainProgram = CreateProgram(TerrainVertexShader, TerrainFragmentShader);
            _colorProgram = CreateProgram(ColorVertexShader, ColorFragmentShader);
            _vao = GL.GenVertexArray(); _vbo = GL.GenBuffer(); _ebo = GL.GenBuffer(); _waterVao = GL.GenVertexArray(); _waterVbo = GL.GenBuffer(); _markerVao = GL.GenVertexArray(); _markerVbo = GL.GenBuffer();
            _initialized = true;
            if (_mesh is not null) UploadResources();
        }
        catch (Exception ex)
        {
            _initialized = false;
            InitializationFailed?.Invoke(this, ex);
        }
    }

    public event EventHandler<Exception>? InitializationFailed;

    public void SetReliefScale(float scale)
    {
        ReliefScale = Math.Clamp(scale, 0, 2);
        if (_heights is null) return;
        _heights.HeightScale = 6f * ReliefScale;
        BuildMesh();
        if (_initialized) UploadResources();
        Invalidate();
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        if (!_initialized || _mesh is null) return;
        MakeCurrent(); GL.Viewport(0, 0, ClientSize.Width, ClientSize.Height); GL.Clear(ClearBufferMask.ColorBufferBit | ClearBufferMask.DepthBufferBit);
        Matrix4 matrix = ToOpenTk(_camera.GetViewMatrix() * _camera.GetProjectionMatrix(ClientSize.Width / (float)Math.Max(1, ClientSize.Height)));
        DrawTerrain(matrix);
        if (ShowGrid) DrawGrid(matrix);
        DrawWater(matrix);
        if (ShowObjects) DrawMarkers(matrix);
        SwapBuffers();
    }

    protected override void OnMouseDown(MouseEventArgs e)
    {
        base.OnMouseDown(e); Focus(); _lastPointer = e.Location;
        if (e.Button == MouseButtons.Middle) { _panning = true; return; }
        if (e.Button == MouseButtons.Right) { _rightClick = true; _rightStart = e.Location; return; }
        if (e.Button == MouseButtons.Left) { _painting = true; _paintedInDrag.Clear(); TryPaint(e.Location); }
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        Point delta = new(e.X - _lastPointer.X, e.Y - _lastPointer.Y);
        if (_panning && e.Button == MouseButtons.Middle) { _camera.Pan(-delta.X * .08f, delta.Y * .08f); Invalidate(); }
        else if (_rightClick && e.Button == MouseButtons.Right && Math.Abs(e.X - _rightStart.X) + Math.Abs(e.Y - _rightStart.Y) >= 4) { _rotating = true; _camera.Rotate(delta.X * .35f, -delta.Y * .35f); Invalidate(); }
        else if (_painting && e.Button == MouseButtons.Left) TryPaint(e.Location);
        if (TryGetTile(e.Location, out int x, out int y)) TileHovered?.Invoke(this, new TileHoverEventArgs(x, y, _textures?[y * _dimension + x]));
        _lastPointer = e.Location;
    }

    protected override void OnMouseUp(MouseEventArgs e)
    {
        base.OnMouseUp(e);
        if (e.Button == MouseButtons.Right && _rightClick && !_rotating && TryGetTile(e.Location, out int x, out int y) && _textures is not null)
            TextureSampled?.Invoke(this, new TextureSampleEventArgs(x, y, _textures[y * _dimension + x]));
        _painting = _panning = _rotating = _rightClick = false; _paintedInDrag.Clear();
    }

    protected override void OnMouseWheel(MouseEventArgs e) { base.OnMouseWheel(e); _camera.Zoom(e.Delta > 0 ? .84f : 1.19f); Invalidate(); }

    private void TryPaint(Point point)
    {
        if (!EditingEnabled || string.IsNullOrWhiteSpace(BrushTexture) || !TryGetTile(point, out int x, out int y) || _textures is null) return;
        int radius = BrushSize / 2;
        for (int py = Math.Max(0, y - radius); py <= Math.Min(_dimension - 1, y + radius); py++)
        for (int px = Math.Max(0, x - radius); px <= Math.Min(_dimension - 1, x + radius); px++)
        {
            int offset = py * _dimension + px;
            if (!_paintedInDrag.Add(offset) || StringComparer.OrdinalIgnoreCase.Equals(_textures[offset], BrushTexture)) continue;
            string before = _textures[offset]; SetTexture(px, py, BrushTexture); TexturePainted?.Invoke(this, new TexturePaintEventArgs(px, py, before, BrushTexture));
        }
    }

    private bool TryGetTile(Point point, out int x, out int y)
    {
        x = y = -1;
        if (_heights is null || ClientSize.Width <= 0 || ClientSize.Height <= 0) return false;
        return TerrainRayPicker.TryPick(_heights, _camera.CreateRay(point.X, point.Y, ClientSize.Width, ClientSize.Height), out x, out y);
    }

    private void BuildMesh()
    {
        if (_heights is null || _atlas is null || _textures is null) return;
        _mesh = TerrainMeshBuilder.Build(_heights, _dimension, (x, y) => _atlas.GetUv(_textures[y * _dimension + x]));
    }

    private void UploadResources()
    {
        if (_mesh is null || _atlas is null || _heights is null) return;
        MakeCurrent();
        GL.BindVertexArray(_vao); GL.BindBuffer(BufferTarget.ArrayBuffer, _vbo);
        float[] vertices = FlattenVertices(_mesh.Vertices); GL.BufferData(BufferTarget.ArrayBuffer, vertices.Length * sizeof(float), vertices, BufferUsageHint.DynamicDraw);
        GL.BindBuffer(BufferTarget.ElementArrayBuffer, _ebo); GL.BufferData(BufferTarget.ElementArrayBuffer, _mesh.Indices.Length * sizeof(uint), _mesh.Indices, BufferUsageHint.StaticDraw);
        ConfigureVertexAttributes();
        if (_atlasTexture != 0) GL.DeleteTexture(_atlasTexture); _atlasTexture = GL.GenTexture(); GL.BindTexture(TextureTarget.Texture2D, _atlasTexture);
        UploadBitmap(_atlas.Image); GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMinFilter, (int)TextureMinFilter.LinearMipmapLinear); GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMagFilter, (int)TextureMagFilter.Linear); GL.GenerateMipmap(GenerateMipmapTarget.Texture2D);
        float waterY = _heightMapStep <= 0 ? 0 : _waterLevel / _heightMapStep / 255f * _heights.HeightScale;
        // Counter-clockwise from above so the water remains visible with back-face culling enabled.
        UploadColoredGeometry(_waterVao, _waterVbo, new[] { new Vector3(0, waterY, 0), new Vector3(0, waterY, _dimension), new Vector3(_dimension, waterY, _dimension), new Vector3(_dimension, waterY, 0) });
        UploadColoredGeometry(_markerVao, _markerVbo, SceneObjectRenderer.BuildMarkerPoints(_objects, _heights));
        _waterColor = new Vector4(_waterSourceColor.R / 255f, _waterSourceColor.G / 255f, _waterSourceColor.B / 255f, .55f);
    }

    private System.Numerics.Vector4 _waterColor = new(.25f, .5f, .8f, .55f);
    private void DrawTerrain(Matrix4 matrix)
    {
        GL.UseProgram(_terrainProgram); GL.UniformMatrix4(GL.GetUniformLocation(_terrainProgram, "uMvp"), false, ref matrix); var light = new OpenTK.Mathematics.Vector3(.4f, .85f, .3f); GL.Uniform3(GL.GetUniformLocation(_terrainProgram, "uLight"), ref light);
        GL.ActiveTexture(TextureUnit.Texture0); GL.BindTexture(TextureTarget.Texture2D, _atlasTexture); GL.Uniform1(GL.GetUniformLocation(_terrainProgram, "uAtlas"), 0);
        GL.BindVertexArray(_vao); GL.DrawElements(PrimitiveType.Triangles, _mesh!.Indices.Length, DrawElementsType.UnsignedInt, 0);
    }

    private void DrawGrid(Matrix4 matrix)
    {
        GL.UseProgram(_terrainProgram); GL.UniformMatrix4(GL.GetUniformLocation(_terrainProgram, "uMvp"), false, ref matrix); var light = new OpenTK.Mathematics.Vector3(.4f, .85f, .3f); GL.Uniform3(GL.GetUniformLocation(_terrainProgram, "uLight"), ref light);
        GL.BindTexture(TextureTarget.Texture2D, _atlasTexture); GL.PolygonMode(TriangleFace.FrontAndBack, PolygonMode.Line); GL.LineWidth(1); GL.BindVertexArray(_vao); GL.DrawElements(PrimitiveType.Triangles, _mesh!.Indices.Length, DrawElementsType.UnsignedInt, 0); GL.PolygonMode(TriangleFace.FrontAndBack, PolygonMode.Fill);
    }

    private void DrawWater(Matrix4 matrix) { DrawColorGeometry(_waterVao, PrimitiveType.TriangleFan, 4, matrix, _waterColor, 1); }
    private void DrawMarkers(Matrix4 matrix)
    {
        for (int index = 0; index < _objects.Count; index++) DrawColorGeometry(_markerVao, PrimitiveType.Points, 1, matrix, TeamColor(_objects[index].Team), 10, index);
    }
    private void DrawColorGeometry(int vao, PrimitiveType primitive, int count, Matrix4 matrix, System.Numerics.Vector4 color, float size, int first = 0)
    {
        if (count == 0) return;
        GL.UseProgram(_colorProgram); GL.UniformMatrix4(GL.GetUniformLocation(_colorProgram, "uMvp"), false, ref matrix); GL.Uniform4(GL.GetUniformLocation(_colorProgram, "uColor"), new OpenTK.Mathematics.Vector4(color.X, color.Y, color.Z, color.W)); GL.PointSize(size); GL.BindVertexArray(vao); GL.DrawArrays(primitive, first, count);
    }

    private static void ConfigureVertexAttributes()
    {
        GL.EnableVertexAttribArray(0); GL.VertexAttribPointer(0, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 0);
        GL.EnableVertexAttribArray(1); GL.VertexAttribPointer(1, 3, VertexAttribPointerType.Float, false, 8 * sizeof(float), 3 * sizeof(float));
        GL.EnableVertexAttribArray(2); GL.VertexAttribPointer(2, 2, VertexAttribPointerType.Float, false, 8 * sizeof(float), 6 * sizeof(float));
    }
    private static void UploadColoredGeometry(int vao, int vbo, System.Numerics.Vector3[] points)
    {
        GL.BindVertexArray(vao); GL.BindBuffer(BufferTarget.ArrayBuffer, vbo); float[] values = points.SelectMany(p => new[] { p.X, p.Y, p.Z }).ToArray(); GL.BufferData(BufferTarget.ArrayBuffer, values.Length * sizeof(float), values, BufferUsageHint.DynamicDraw);
        GL.EnableVertexAttribArray(0); GL.VertexAttribPointer(0, 3, VertexAttribPointerType.Float, false, 3 * sizeof(float), 0);
    }
    private static float[] FlattenVertices(IEnumerable<TerrainVertex> vertices) => vertices.SelectMany(v => new[] { v.Position.X, v.Position.Y, v.Position.Z, v.Normal.X, v.Normal.Y, v.Normal.Z, v.TexCoord.X, v.TexCoord.Y }).ToArray();
    private static byte[] ReadSamples(Bitmap bitmap)
    {
        var samples = new byte[bitmap.Width * bitmap.Height];
        for (int y = 0; y < bitmap.Height; y++) for (int x = 0; x < bitmap.Width; x++) samples[y * bitmap.Width + x] = bitmap.GetPixel(x, y).R;
        return samples;
    }
    private static void UploadBitmap(Bitmap image)
    {
        var rectangle = new Rectangle(0, 0, image.Width, image.Height); BitmapData data = image.LockBits(rectangle, ImageLockMode.ReadOnly, System.Drawing.Imaging.PixelFormat.Format32bppArgb);
        try { GL.TexImage2D(TextureTarget.Texture2D, 0, PixelInternalFormat.Rgba, image.Width, image.Height, 0, OpenTK.Graphics.OpenGL4.PixelFormat.Bgra, PixelType.UnsignedByte, data.Scan0); }
        finally { image.UnlockBits(data); }
    }
    private static Matrix4 ToOpenTk(Matrix4x4 value) => new(value.M11, value.M12, value.M13, value.M14, value.M21, value.M22, value.M23, value.M24, value.M31, value.M32, value.M33, value.M34, value.M41, value.M42, value.M43, value.M44);
    private static System.Numerics.Vector4 TeamColor(int team) => team switch
    {
        1 => new(.25f, .45f, .95f, 1), 2 => new(.9f, .15f, .2f, 1), 3 => new(.9f, .7f, .1f, 1), 4 => new(.2f, .7f, .4f, 1),
        5 => new(.7f, .35f, .85f, 1), 6 => new(.95f, .4f, .1f, 1), 7 => new(.1f, .85f, .9f, 1), _ => new(.8f, .8f, .8f, 1)
    };
    private static int CreateProgram(string vertex, string fragment)
    {
        int vs = GL.CreateShader(ShaderType.VertexShader); GL.ShaderSource(vs, vertex); GL.CompileShader(vs);
        int fs = GL.CreateShader(ShaderType.FragmentShader); GL.ShaderSource(fs, fragment); GL.CompileShader(fs);
        int program = GL.CreateProgram(); GL.AttachShader(program, vs); GL.AttachShader(program, fs); GL.LinkProgram(program); GL.DeleteShader(vs); GL.DeleteShader(fs); return program;
    }
    protected override void Dispose(bool disposing)
    {
        if (disposing) { _atlas?.Dispose(); _library?.Dispose(); if (_initialized) { MakeCurrent(); GL.DeleteBuffer(_vbo); GL.DeleteBuffer(_ebo); GL.DeleteVertexArray(_vao); GL.DeleteTexture(_atlasTexture); GL.DeleteProgram(_terrainProgram); GL.DeleteProgram(_colorProgram); } }
        base.Dispose(disposing);
    }

    private const string TerrainVertexShader = "#version 330 core\nlayout(location=0) in vec3 p; layout(location=1) in vec3 n; layout(location=2) in vec2 uv; uniform mat4 uMvp; out vec3 N; out vec2 UV; void main(){ N=n; UV=uv; gl_Position=uMvp*vec4(p,1.0);}";
    private const string TerrainFragmentShader = "#version 330 core\nin vec3 N; in vec2 UV; uniform sampler2D uAtlas; uniform vec3 uLight; out vec4 c; void main(){float l=max(.28,dot(normalize(N),normalize(uLight))); c=vec4(texture(uAtlas,UV).rgb*l,1.0);}";
    private const string ColorVertexShader = "#version 330 core\nlayout(location=0) in vec3 p; uniform mat4 uMvp; void main(){gl_Position=uMvp*vec4(p,1.0);}";
    private const string ColorFragmentShader = "#version 330 core\nuniform vec4 uColor; out vec4 c; void main(){c=uColor;}";
}
