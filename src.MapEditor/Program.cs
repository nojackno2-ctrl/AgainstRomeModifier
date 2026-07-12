namespace AgainstRomeMapEditor;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        Application.Run(new MapEditorForm(EditorArguments.Parse(args)));
    }
}

internal sealed record EditorArguments(string? GamePath, int? SelectedSlot)
{
    public static EditorArguments Parse(string[] args)
    {
        string? gamePath = null; int? slot = null;
        for (int i = 0; i < args.Length; i++)
        {
            if (args[i].Equals("--game", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length) gamePath = args[++i];
            else if (args[i].Equals("--map", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length && int.TryParse(args[++i].Replace("ENDL_", "", StringComparison.OrdinalIgnoreCase), out int parsed) && parsed is >= 0 and <= 999) slot = parsed;
        }
        return new EditorArguments(gamePath, slot);
    }
}
