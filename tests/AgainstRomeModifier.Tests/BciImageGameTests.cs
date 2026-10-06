using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

/// <summary>以本機原版腳本（唯讀）驗證 BCI 映像往返；設定 ARM_GAME_PATH 才執行。</summary>
public sealed class BciImageGameTests
{
    [Fact]
    public void Every_original_script_round_trips_byte_identically()
    {
        if (Environment.GetEnvironmentVariable("ARM_GAME_PATH") is not { } game || !Directory.Exists(game)) return;
        int count = 0;
        foreach (string path in Directory.GetFiles(game, "*.bci", SearchOption.AllDirectories))
        {
            byte[] data = GameLZSS.DecompressPfil(File.ReadAllBytes(path));
            BciImage image = BciImage.Parse(data);
            Assert.Equal(data, image.Serialize());
            count++;
        }
        Assert.True(count > 50, $"只找到 {count} 個腳本");
        BciImage endless = BciImage.Parse(GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(game, "MAPS", "ENDL_000", "SCRIPT", "ak_level.bci"))));
        Assert.Equal(0x1ae64, endless.MainAddress);
        Assert.Contains(Enumerable.Range(0, endless.ConstOffsets.Count), index => endless.Constant(index) == "s_getTime");
    }
}
