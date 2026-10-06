using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 以本機原版地圖（唯讀）驗證 <see cref="LevelObjectStore"/>；設定 ARM_GAME_PATH 才執行，只在記憶體中操作，不寫回遊戲目錄。
/// </summary>
public sealed class LevelObjectStoreGameTests
{
    private static string? GamePath => Environment.GetEnvironmentVariable("ARM_GAME_PATH");

    [Fact]
    public void Original_endless_map_parses_and_add_remove_round_trips_in_memory()
    {
        if (GamePath is not { } game || !Directory.Exists(Path.Combine(game, "MAPS", "ENDL_000"))) return;
        LevelObjectStore store = LevelObjectStore.Load(Path.Combine(game, "MAPS", "ENDL_000"));
        IReadOnlyDictionary<int, string> names = ObjDefNames.Load(game);
        IReadOnlyList<LevelWorldObject> before = store.Objects();
        Assert.Equal(6618, before.Count);
        Assert.All(before, item => Assert.InRange(item.X, 0, 16384));
        Assert.Equal("LanGerGra00_Gras_M", names[146]);

        LevelObjectTemplate tree = store.Templates().First(template => names.TryGetValue(template.TypeId, out string? name) && name.Contains("Tanne", StringComparison.OrdinalIgnoreCase));
        int slot = store.Add(tree, 8000, 200, 8100, 1.5f);
        Assert.True(slot >= 0);
        LevelWorldObject added = Assert.Single(store.Objects(), item => item.Slot == slot);
        Assert.Equal(tree.TypeId, added.TypeId);
        Assert.Equal((8000f, 200f, 8100f, 1.5f), (added.X, added.Y, added.Z, added.Rotation));
        Assert.True(added.Uid > before.Max(item => item.Uid));
        Assert.Equal(before.Count + 1, store.Objects().Count);

        Assert.True(store.Remove(slot));
        Assert.Equal(before.Count, store.Objects().Count);
        LevelWorldObject linked = before.First(item => item.Linked);
        Assert.False(store.Remove(linked.Slot)); // 連結物件不可刪
    }
}
