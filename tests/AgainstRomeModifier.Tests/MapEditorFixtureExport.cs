namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    /// <summary>
    /// 手動煙霧測試用：設定 ARM_FIXTURE_EXPORT 時，把合成遊戲目錄（floortex.dat＋MAPS/ENDL_005）複製到該路徑，
    /// 供以 <c>dotnet AgainstRomeModifier.dll --game &lt;path&gt; --map ENDL_005</c> 在真正的程式行程中開啟。未設定時不做任何事。
    /// </summary>
    [Fact]
    public void Export_synthetic_game_root_for_manual_smoke_runs()
    {
        string? target = Environment.GetEnvironmentVariable("ARM_FIXTURE_EXPORT");
        if (string.IsNullOrWhiteSpace(target)) return;
        CreateFixture();
        foreach (string file in Directory.EnumerateFiles(_root, "*", SearchOption.AllDirectories))
        {
            string destination = Path.Combine(target, Path.GetRelativePath(_root, file));
            Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
            File.Copy(file, destination, overwrite: true);
        }
    }
}
