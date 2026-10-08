using AgainstRomeModifier;
using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private EditorConsoleControl? _console;
    private TabPage? _consoleTab;
    private CommandExecutionContext? _consoleContext;

    private void AddConsoleTab()
    {
        _console = new EditorConsoleControl();
        _consoleContext = new CommandExecutionContext();
        _console.Bind(_consoleContext, new MacroScriptRunner(CommandRegistry.CreateDefault()));
        _console.BeforeExecute = RefreshConsoleContext;
        _console.CommandExecuted += (_, _) => ApplyConsoleResults();
        _consoleTab = new TabPage("控制台") { BackColor = WinFormsTheme.Surface };
        _consoleTab.Controls.Add(_console);
        _inspectorTabs.TabPages.Add(_consoleTab);
    }

    /// <summary>把目前地圖的各編輯 Session 與目錄交給控制台（載入新地圖後 Session 是新實例）。</summary>
    internal void RefreshConsoleContext()
    {
        if (_consoleContext is not { } context) return;
        context.HeightSession = _terrainLayers;
        context.BlendSession = _terrainBlendSession;
        context.PlacementSession = _placementSession;
        context.NatureSession = _natureSession;
        int tileDim = _texturesDocument?.Dimension ?? _terrainBlendSession?.TileDimension ?? 64;
        context.MapTileDimension = tileDim;
        context.WorldDimension = tileDim * 256f;
        context.AvailableObjectTypes = _objectCatalog;
        context.AvailableNatureTemplates = NatureLayoutTemplates();
        context.WaterLevel = (float)_waterLevel.Value;
        context.HeightStep = _heightMapStep;
        context.Events = EventSession.Capture();
        context.KnownAliases = _objectCatalog.Select(AliasOf).ToArray();
        context.KnownTextures = _texturesDocument?.Textures.Distinct(StringComparer.OrdinalIgnoreCase).ToArray() ?? [];
    }

    /// <summary>巨集改動各 Session 後，同步 2D／3D 視圖、清單與按鈕狀態。</summary>
    internal void ApplyConsoleResults()
    {
        if (_terrainLayers is not null) ApplyHeightsToViews();
        if (_texturesDocument is not null && _terrainBlendSession is not null)
        {
            _texturesDocument.SetTextures(_terrainBlendSession.CurrentTextures);
            _canvas.Invalidate(); _view3d?.Invalidate();
        }
        RefreshPlacedList(); RefreshSceneMarkers(); UpdateEditorState();
    }
}
