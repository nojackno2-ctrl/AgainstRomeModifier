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
    private TerrainHeightField? _heights;
    private FloorTextureLibrary? _library;
    private FloorTextureAtlas? _atlas;
    private TerrainMeshData? _mesh;
    private IReadOnlyList<MapSceneObject> _objects = Array.Empty<MapSceneObject>();
    private int _dimension;
    private int _hoverX = -1, _hoverY = -1;
    private float _waterLevel, _heightMapStep;
    private Color _waterSourceColor = Color.SteelBlue;
    private bool _initialized, _painting, _movingSceneObject, _panning, _rotating, _rightClick;
    private Point _lastPointer, _rightStart;
    private int _terrainProgram, _colorProgram, _vao, _vbo, _ebo, _atlasTexture, _waterVao, _waterVbo, _markerVao, _markerVbo, _cursorVao, _cursorVbo;
    private int _cursorVertexCount;

    public Map3DViewControl() : base(new GLControlSettings { API = ContextAPI.OpenGL, APIVersion = new Version(3, 3), Profile = ContextProfile.Core, Flags = ContextFlags.ForwardCompatible })
    {
        Dock = DockStyle.Fill;
        BackColor = Color.FromArgb(24, 28, 36);
        Cursor = Cursors.Cross;
    }

    public string? BrushTexture { get; set; }
    public int BrushSize { get; set; } = 1;
    public bool EditingEnabled { get; set; }
    public bool SceneMoveEnabled { get; set; }
    public bool ShowGrid { get; set; } = true;
    public bool ShowObjects { get; set; } = true;
    public float ReliefScale { get; private set; } = 1f;
    public bool IsReady => _initialized;
    public string? LastFailureReason { get; private set; }
    public string? ContextDescription { get; private set; }
    public event EventHandler<TexturePaintEventArgs>? TexturePainted;
    public event EventHandler<TileHoverEventArgs>? TileHovered;
    public event EventHandler<TextureSampleEventArgs>? TextureSampled;
    public event EventHandler? StrokeEnded;
    public event EventHandler<SceneObjectMoveEventArgs>? SceneObjectMoved;

    // 3D 檢視不繪製「與已儲存基準的差異」高亮（那是 2D MapCanvasControl 的職責），
    // 因此這裡不需要 baselineTextures。
    public bool LoadTextures(int dimension, IReadOnlyList<string> textures, string mapDirectory, FloorTextureLibrary floorTextures, IReadOnlyList<MapSceneObject> sceneObjects, float waterLevel, float heightMapStep, Color waterColor)
    {
        LastFailureReason = null;
        string heightSource = Path.Combine(mapDirectory, "boden.bmp");
        _library = floorTextures; // 生命週期由 MapEditorForm 擁有，此處僅借用。
        LastFailureReason = ValidateResources(mapDirectory, _library.IsAvailable);
        if (LastFailureReason is not null) return false;
        using var bitmap = new Bitmap(heightSource);
        byte[] samples = ReadSamples(bitmap);
        // A 257x257 source covers the complete 64x64 tile map (four height samples per tile).
        _heights = new TerrainHeightField(bitmap.Width, bitmap.Height, samples, tileWidth: dimension, tileHeight: dimension);
        _dimension = dimension; _textures = textures.ToArray(); _objects = sceneObjects; _waterLevel = waterLevel; _heightMapStep = heightMapStep; _waterSourceColor = waterColor;
        _atlas?.Dispose(); _atlas = FloorTextureAtlas.Create(_textures, _library);
        BuildMesh();
        if (_initialized) UploadResources();
        Invalidate();
        return true;
    }

    internal static string? ValidateResources(string mapDirectory, bool floorTextureLibraryAvailable)
    {
        string heightSource = Path.Combine(mapDirectory, "boden.bmp");
        if (!File.Exists(heightSource)) return $"地圖缺少 3D 地勢來源：{heightSource}";
        if (!floorTextureLibraryAvailable) return "無法讀取 floortex.dat，3D 地表材質庫不可用。";
        return null;
    }

    public void SetTexture(int x, int y, string texture)
    {
        if (_textures is null || _atlas is null || _mesh is null || x < 0 || x >= _dimension || y < 0 || y >= _dimension) return;
        _textures[y * _dimension + x] = texture;
        if (!_atlas.Contains(texture)) { RebuildAtlasAndMesh(); return; } // 完整材質庫可能加入原圖沒有的地表，需擴充圖集。
        Vector2[] uv = _atlas.GetUv(texture);
        int vertexOffset = (y * _dimension + x) * 4;
        for (int index = 0; index < 4; index++) _mesh.Vertices[vertexOffset + index] = _mesh.Vertices[vertexOffset + index] with { TexCoord = uv[index] };
        if (_initialized)
        {
            MakeCurrent(); GL.BindBuffer(BufferTarget.ArrayBuffer, _vbo);
            var block = new float[4 * 8];
            for (int index = 0; index < 4; index++)
            {
                TerrainVertex vertex = _mesh.Vertices[vertexOffset + index]; int b = index * 8;
                block[b] = vertex.Position.X; block[b + 1] = vertex.Position.Y; block[b + 2] = vertex.Position.Z;
                block[b + 3] = vertex.Normal.X; block[b + 4] = vertex.Normal.Y; block[b + 5] = vertex.Normal.Z;
                block[b + 6] = vertex.TexCoord.X; block[b + 7] = vertex.TexCoord.Y;
            }
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
            ContextDescription = $"OpenGL {GL.GetString(StringName.Version) ?? "unknown"} | {GL.GetString(StringName.Vendor) ?? "unknown"} | {GL.GetString(StringName.Renderer) ?? "unknown"}";
            GL.ClearColor(.07f, .09f, .12f, 1); GL.Enable(EnableCap.DepthTest); GL.Enable(EnableCap.CullFace); GL.Enable(EnableCap.Blend);
            GL.BlendFunc(BlendingFactor.SrcAlpha, BlendingFactor.OneMinusSrcAlpha);
            _terrainProgram = CreateProgram(TerrainVertexShader, TerrainFragmentShader);
            _colorProgram = CreateProgram(ColorVertexShader, ColorFragmentShader);
            _vao = GL.GenVertexArray(); _vbo = GL.GenBuffer(); _ebo = GL.GenBuffer(); _waterVao = GL.GenVertexArray(); _waterVbo = GL.GenBuffer(); _markerVao = GL.GenVertexArray(); _markerVbo = GL.GenBuffer(); _cursorVao = GL.GenVertexArray(); _cursorVbo = GL.GenBuffer();
            _initialized = true;
            LastFailureReason = null;
            if (_mesh is not null) UploadResources();
        }
        catch (Exception ex)
        {
            _initialized = false;
            LastFailureReason = $"OpenGL 3.3 初始化失敗：{ex.GetType().Name}: {ex.Message}";
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

    /// <summary>改水面高度／顏色時的即時預覽：只更新水面四邊形與顏色，不重建地形。</summary>
    public void UpdateWater(float waterLevel, Color waterColor)
    {
        _waterLevel = waterLevel; _waterSourceColor = waterColor;
        if (!_initialized || _heights is null) return;
        MakeCurrent();
        float waterY = _heightMapStep <= 0 ? 0 : _waterLevel / _heightMapStep / 255f * _heights.HeightScale;
        UploadColoredGeometry(_waterVao, _waterVbo, new[] { new Vector3(0, waterY, 0), new Vector3(0, waterY, _dimension), new Vector3(_dimension, waterY, _dimension), new Vector3(_dimension, waterY, 0) });
        _waterColor = new Vector4(_waterSourceColor.R / 255f, _waterSourceColor.G / 255f, _waterSourceColor.B / 255f, .55f);
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
        DrawBrushCursor(matrix);
        SwapBuffers();
    }

    private void DrawBrushCursor(Matrix4 matrix)
    {
        if (!EditingEnabled || _hoverX < 0 || _hoverY < 0 || _heights is null || string.IsNullOrWhiteSpace(BrushTexture)) return;
        int radius = Math.Max(0, BrushSize / 2);
        int x0 = Math.Max(0, _hoverX - radius), y0 = Math.Max(0, _hoverY - radius);
        int x1 = Math.Min(_dimension - 1, _hoverX + radius) + 1, y1 = Math.Min(_dimension - 1, _hoverY + radius) + 1;
        var perimeter = new List<System.Numerics.Vector3>();
        for (int x = x0; x <= x1; x++) perimeter.Add(CursorPoint(x, y0));
        for (int y = y0 + 1; y <= y1; y++) perimeter.Add(CursorPoint(x1, y));
        for (int x = x1 - 1; x >= x0; x--) perimeter.Add(CursorPoint(x, y1));
        for (int y = y1 - 1; y > y0; y--) perimeter.Add(CursorPoint(x0, y));
        _cursorVertexCount = perimeter.Count;
        UploadColoredGeometry(_cursorVao, _cursorVbo, perimeter.ToArray());
        GL.Disable(EnableCap.DepthTest); GL.LineWidth(2);
        DrawColorGeometry(_cursorVao, PrimitiveType.LineLoop, _cursorVertexCount, matrix, new System.Numerics.Vector4(1f, .94f, .47f, 1f), 1);
        GL.Enable(EnableCap.DepthTest);
    }

    private System.Numerics.Vector3 CursorPoint(int tileX, int tileY)
        => new(tileX, _heights!.SampleHeight(tileX, tileY) + .15f, tileY);

    protected override void OnMouseDown(MouseEventArgs e)
    {
        base.OnMouseDown(e); Focus(); _lastPointer = e.Location;
        if (e.Button == MouseButtons.Middle) { _panning = true; return; }
        if (e.Button == MouseButtons.Right) { _rightClick = true; _rightStart = e.Location; return; }
        if (e.Button == MouseButtons.Left)
        {
            if (SceneMoveEnabled) _movingSceneObject = TryMoveSceneObject(e.Location, completed: false);
            else { _painting = true; _paintedInDrag.Clear(); TryPaint(e.Location); }
        }
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        Point delta = new(e.X - _lastPointer.X, e.Y - _lastPointer.Y);
        if (_panning && e.Button == MouseButtons.Middle) { _camera.Pan(-delta.X * .08f, delta.Y * .08f); Invalidate(); }
        else if (_rightClick && e.Button == MouseButtons.Right && Math.Abs(e.X - _rightStart.X) + Math.Abs(e.Y - _rightStart.Y) >= 4) { _rotating = true; _camera.Rotate(delta.X * .35f, -delta.Y * .35f); Invalidate(); }
        else if (_movingSceneObject && e.Button == MouseButtons.Left) TryMoveSceneObject(e.Location, completed: false);
        else if (_painting && e.Button == MouseButtons.Left) TryPaint(e.Location);
        if (TryGetTile(e.Location, out int x, out int y))
        {
            if (x != _hoverX || y != _hoverY) { _hoverX = x; _hoverY = y; if (EditingEnabled) Invalidate(); }
            TileHovered?.Invoke(this, new TileHoverEventArgs(x, y, _textures?[y * _dimension + x]));
        }
        else if (_hoverX != -1) { _hoverX = _hoverY = -1; if (EditingEnabled) Invalidate(); }
        _lastPointer = e.Location;
    }

    protected override void OnMouseLeave(EventArgs e)
    {
        base.OnMouseLeave(e);
        if (_hoverX == -1) return;
        _hoverX = _hoverY = -1; if (EditingEnabled) Invalidate();
    }

    protected override void OnMouseUp(MouseEventArgs e)
    {
        base.OnMouseUp(e);
        if (e.Button == MouseButtons.Right && _rightClick && !_rotating && TryGetTile(e.Location, out int x, out int y) && _textures is not null)
            TextureSampled?.Invoke(this, new TextureSampleEventArgs(x, y, _textures[y * _dimension + x]));
        bool wasPainting = _painting;
        if (_movingSceneObject && e.Button == MouseButtons.Left) TryMoveSceneObject(e.Location, completed: true);
        _painting = _movingSceneObject = _panning = _rotating = _rightClick = false; _paintedInDrag.Clear();
        if (wasPainting) StrokeEnded?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>將攝影機對焦到指定 tile，供場景物件清單跳轉使用。</summary>
    public void FocusTile(float tileX, float tileY)
    {
        float y = _heights?.SampleHeight(tileX, tileY) ?? 0;
        _camera.Target = new Vector3(tileX, y, tileY);
        Invalidate();
    }

    public void UpdateSceneObjects(IReadOnlyList<MapSceneObject> sceneObjects)
    {
        _objects = sceneObjects;
        if (_initialized && _heights is not null)
        {
            MakeCurrent();
            UploadColoredGeometry(_markerVao, _markerVbo, SceneObjectRenderer.BuildMarkerPoints(_objects, _heights));
        }
        Invalidate();
    }

    protected override void OnMouseWheel(MouseEventArgs e) { base.OnMouseWheel(e); _camera.Zoom(e.Delta > 0 ? .84f : 1.19f); Invalidate(); }

    private void TryPaint(Point point)
    {
        if (!EditingEnabled || string.IsNullOrWhiteSpace(BrushTexture) || !TryGetTile(point, out int x, out int y) || _textures is null) return;
        int offset = y * _dimension + x;
        if (!_paintedInDrag.Add(offset)) return;
        TexturePainted?.Invoke(this, new TexturePaintEventArgs(x, y, _textures[offset], BrushTexture));
    }

    private bool TryMoveSceneObject(Point point, bool completed)
    {
        if (!EditingEnabled || !SceneMoveEnabled || !TryGetTile(point, out int x, out int y)) return false;
        float worldUnitsPerTile = SdlSceneCatalog.WorldUnitsPerMapPixel * (SdlSceneCatalog.MapPixelSize / (float)_dimension);
        SceneObjectMoved?.Invoke(this, new SceneObjectMoveEventArgs((x + .5f) * worldUnitsPerTile, (y + .5f) * worldUnitsPerTile, completed));
        return true;
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

    // 繪入原圖沒有的材質時擴充圖集並重建 UV；若圖集容量已滿則保留舊圖集（該材質在 3D 以近似色顯示，2D 仍正確）。
    private void RebuildAtlasAndMesh()
    {
        if (_textures is null || _library is null || _heights is null) return;
        try
        {
            FloorTextureAtlas rebuilt = FloorTextureAtlas.Create(_textures, _library);
            _atlas?.Dispose(); _atlas = rebuilt;
            BuildMesh();
            if (_initialized) UploadResources();
            Invalidate();
        }
        catch (InvalidOperationException) { Invalidate(); }
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
        // 限制 mip 層級：每格 gutter 為 FloorTextureAtlas.Pad 像素，層級 3 的取樣足跡約 8 像素，剛好不越過 gutter 造成串色。
        GL.TexParameter(TextureTarget.Texture2D, TextureParameterName.TextureMaxLevel, 3);
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
        int[] pixels = BitmapPixels.Read(bitmap);
        var samples = new byte[pixels.Length];
        for (int i = 0; i < pixels.Length; i++) samples[i] = (byte)((pixels[i] >> 16) & 0xff);
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
        int vs = CompileShader(ShaderType.VertexShader, vertex);
        int fs = 0;
        try
        {
            fs = CompileShader(ShaderType.FragmentShader, fragment);
            int program = GL.CreateProgram(); GL.AttachShader(program, vs); GL.AttachShader(program, fs); GL.LinkProgram(program);
            GL.GetProgram(program, GetProgramParameterName.LinkStatus, out int linked);
            string log = GL.GetProgramInfoLog(program);
            if (linked == 0) { GL.DeleteProgram(program); throw new InvalidOperationException("OpenGL shader program 連結失敗：" + log); }
            return program;
        }
        finally
        {
            GL.DeleteShader(vs);
            if (fs != 0) GL.DeleteShader(fs);
        }
    }
    private static int CompileShader(ShaderType type, string source)
    {
        int shader = GL.CreateShader(type); GL.ShaderSource(shader, source); GL.CompileShader(shader);
        GL.GetShader(shader, ShaderParameter.CompileStatus, out int compiled);
        string log = GL.GetShaderInfoLog(shader);
        if (compiled != 0) return shader;
        GL.DeleteShader(shader);
        throw new InvalidOperationException($"OpenGL {type} 編譯失敗：{log}");
    }
    protected override void Dispose(bool disposing)
    {
        if (disposing) { _atlas?.Dispose(); /* _library 由 MapEditorForm 擁有，不在此釋放 */ if (_initialized) { MakeCurrent(); GL.DeleteBuffer(_vbo); GL.DeleteBuffer(_ebo); GL.DeleteBuffer(_waterVbo); GL.DeleteBuffer(_markerVbo); GL.DeleteBuffer(_cursorVbo); GL.DeleteVertexArray(_vao); GL.DeleteVertexArray(_waterVao); GL.DeleteVertexArray(_markerVao); GL.DeleteVertexArray(_cursorVao); GL.DeleteTexture(_atlasTexture); GL.DeleteProgram(_terrainProgram); GL.DeleteProgram(_colorProgram); } }
        base.Dispose(disposing);
    }

    private const string TerrainVertexShader = "#version 330 core\nlayout(location=0) in vec3 p; layout(location=1) in vec3 n; layout(location=2) in vec2 uv; uniform mat4 uMvp; out vec3 N; out vec2 UV; void main(){ N=n; UV=uv; gl_Position=uMvp*vec4(p,1.0);}";
    private const string TerrainFragmentShader = "#version 330 core\nin vec3 N; in vec2 UV; uniform sampler2D uAtlas; uniform vec3 uLight; out vec4 c; void main(){float l=max(.28,dot(normalize(N),normalize(uLight))); c=vec4(texture(uAtlas,UV).rgb*l,1.0);}";
    private const string ColorVertexShader = "#version 330 core\nlayout(location=0) in vec3 p; uniform mat4 uMvp; void main(){gl_Position=uMvp*vec4(p,1.0);}";
    private const string ColorFragmentShader = "#version 330 core\nuniform vec4 uColor; out vec4 c; void main(){c=uColor;}";
}
