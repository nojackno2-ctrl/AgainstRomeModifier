using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Direct_map_entry_resolves_the_same_scope_as_the_map_selection()
    {
        string custom = CreateFixture("ENDL_005");
        CreateFixture("ENDL_000");
        CreateFixture("KAMP_01");

        GameMapInfo map = Program.ResolveDirectMap(Program.ParseStartupOptions(["--game", _root, "--map", "endl_005"]));
        Assert.Equal(custom, map.DirectoryPath);
        Assert.True(map.IsCustom);
        Assert.False(Program.ResolveDirectMap(new Program.StartupOptions(_root, "ENDL_000")).IsCustom); // 原版可直接開啟但唯讀

        Assert.Contains("--game", Assert.Throws<ArgumentException>(() => Program.ResolveDirectMap(new Program.StartupOptions(null, "ENDL_005"))).Message);
        Assert.Contains("KAMP_01", Assert.Throws<ArgumentException>(() => Program.ResolveDirectMap(new Program.StartupOptions(_root, "KAMP_01"))).Message);
        Assert.Throws<DirectoryNotFoundException>(() => Program.ResolveDirectMap(new Program.StartupOptions(_root, "ENDL_123")));
        Assert.Throws<ArgumentException>(() => Program.ResolveDirectMap(new Program.StartupOptions(_root, "..\\ENDL_005")));
        Assert.Throws<DirectoryNotFoundException>(() => Program.ResolveDirectMap(new Program.StartupOptions(Path.Combine(_root, "missing"), "ENDL_005")));
    }
}
