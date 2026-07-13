namespace AgainstRomeMapEditor;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        EditorArguments arguments = EditorArguments.Parse(args);
        string gamePath = arguments.GamePath ?? DetectGamePath();
        string? preferredMapId = arguments.SelectedMapId;
        while (true)
        {
            using var selection = new MapSelectionForm(gamePath, preferredMapId);
            if (selection.ShowDialog() != DialogResult.OK || selection.SelectedMap is null) return;
            gamePath = selection.GamePath; preferredMapId = selection.SelectedMap.Id;
            using var editor = new MapEditorForm(gamePath, selection.SelectedMap);
            Application.Run(editor);
            if (!editor.ReturnToMapMenu) return;
        }
    }

    private static string DetectGamePath() => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86), "Against Rome");
}

internal sealed record EditorArguments(string? GamePath, string? SelectedMapId)
{
    public static EditorArguments Parse(string[] args)
    {
        string? gamePath = null; string? mapId = null;
        for (int i = 0; i < args.Length; i++)
        {
            if (args[i].Equals("--game", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length) gamePath = args[++i];
            else if (args[i].Equals("--map", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length) mapId = args[++i];
        }
        return new EditorArguments(gamePath, mapId);
    }
}
