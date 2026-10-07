namespace AgainstRomeMapEditor;

/// <summary>Pure native tile lookup boundary; bitmap loading and catalog construction belong to the host.</summary>
internal interface INativeTerrainMaterialResolver
{
    bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners);
    string? ResolveNativeTile(IReadOnlyList<string> cornerMaterialIds, int x, int y);
    /// <summary>可作為自動過渡中介的材質；預設為空（不做自動過渡）。</summary>
    IReadOnlyList<string> MaterialIds => Array.Empty<string>();
}
