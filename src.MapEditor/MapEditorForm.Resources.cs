namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    /// <summary>Reload display assets without loading map documents or resetting any edit session.</summary>
    internal bool TryReloadDisplayResources(out Exception? error)
    {
        error = null;
        FloorMaterialCatalog? previousCatalog = _floorMaterials;
        TerrainBlendEditSession? previousBlend = _terrainBlendSession;
        var previousLayers = _terrainLayers;
        var previousBoden = _bodenLayer;
        var previousEmboss = _embossLayer;
        var previousCollision = _collisionLayer;
        FloorTextureLibrary? candidate = null;
        bool adopted = false;
        try
        {
            if (_selected is null || _texturesDocument is null || _floorTextures is null || IsDisposed)
                throw new InvalidOperationException("No map is open.");
            CommitStroke();
            candidate = new FloorTextureLibrary(Path.Combine(_gamePath, "floortex.dat"));
            if (!candidate.IsAvailable) throw new FileNotFoundException("floortex.dat has no readable terrain textures.");
            // Decode before ownership changes: a malformed tile must not break the currently usable library.
            foreach (string name in candidate.Names) _ = candidate.Get(name);
            var catalog = new FloorMaterialCatalog(candidate);
            foreach (string file in new[] { "boden.bmp", "emboss.bmp", "smooth.bmp", "minimap.bmp", "collision.bmp" })
            {
                string path = Path.Combine(_selected.DirectoryPath, file);
                if (File.Exists(path)) using (var bitmap = new Bitmap(path)) { _ = bitmap.Width; }
            }
            _currentMaterialSwatch.Image = null;
            _floorTextures.ReplaceWith(candidate);
            adopted = true;
            _floorMaterials = catalog;
            if (_terrainBlendSession is null) InitializeTerrainBlendSession();
            else _terrainBlendSession.RebindMaterialResolver(catalog);
            if (_terrainLayers is null) InitializeTerrainLayers(_selected.DirectoryPath);
            if (_view3d is { IsHandleCreated: true, IsReady: false } && !_view3d.RetryInitialization())
                Disable3DView(_view3d.LastFailureReason ?? "OpenGL 3.3 initialization failed.");
            LoadPalette(_paletteSearch.Text);
            LoadEditingScene(preserveView: true);
            ApplyHeightsToViews(useCurrentSamples: true);
            UpdateCollisionOverlay();
            UpdateEditorState();
            return true;
        }
        catch (Exception ex)
        {
            if (adopted && _floorTextures is not null && candidate is not null)
            {
                _currentMaterialSwatch.Image = null;
                _floorTextures.ReplaceWith(candidate);
                _floorMaterials = previousCatalog;
                _terrainBlendSession = previousBlend;
                _terrainLayers = previousLayers;
                _bodenLayer = previousBoden; _embossLayer = previousEmboss; _collisionLayer = previousCollision;
                if (previousCatalog is not null) _terrainBlendSession?.RebindMaterialResolver(previousCatalog);
            }
            error = ex;
            bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
            _last3DDiagnostic = Build3DDiagnostic(isEn ? "Display resource reload failed." : "顯示素材重新載入失敗。", ex);
            _3dDiagnosticsButton.Visible = true; _retryDisplayButton.Visible = true;
            _modeBanner.Text = isEn ? "Display reload failed; edits are retained. See 3D Diagnostics." : "顯示素材重載失敗，編輯已保留；請查看「3D 診斷」。";
            _modeBanner.BackColor = Color.FromArgb(86, 69, 40);
            UpdateEditorState();
            return false;
        }
        finally { candidate?.Dispose(); }
    }
}
