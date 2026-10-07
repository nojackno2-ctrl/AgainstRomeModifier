using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

/// <summary>以本機原版 ENDL_000 腳本（唯讀）驗證注入結果；設定 ARM_GAME_PATH 才執行，ARM_DUMP_TARGET 時另存注入後映像供反組譯檢查。</summary>
public sealed class LevelScriptInjectorGameTests
{
    [Theory]
    [InlineData("ENDL_000")]
    [InlineData("ENDL_001")]
    [InlineData("ENDL_002")]
    [InlineData("ENDL_003")]
    [InlineData("ENDL_004")]
    public void Timer_events_can_hook_original_endless_main_without_writing_game_files(string map)
    {
        if (Environment.GetEnvironmentVariable("ARM_GAME_PATH") is not { } game || !Directory.Exists(game)) return;
        byte[] original = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(game, "MAPS", map, "SCRIPT", "ak_level.bci")));
        BciImage image = BciImage.Parse(original);
        int originalMain = image.MainAddress;
        ScenarioEventCompiler.Inject(image, [new ScenarioEvent("Runtime probe", 10)
            { Actions = [new ScenarioAction(ScenarioActionKind.Message, "ARM event OK")] }], originalMain);
        BciImage reparsed = BciImage.Parse(image.Serialize());
        Assert.True(reparsed.MainAddress >= BciImage.Parse(original).Code.Length);
        Assert.Equal(image.Code, reparsed.Code);
    }

    [Fact]
    public void Injected_endless_script_keeps_original_sections_and_jumps_back_to_main()
    {
        if (Environment.GetEnvironmentVariable("ARM_GAME_PATH") is not { } game || !Directory.Exists(game)) return;
        byte[] original = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(game, "MAPS", "ENDL_000", "SCRIPT", "ak_level.bci")));
        BciImage image = BciImage.Parse(original);
        int originalMain = image.MainAddress, originalCodeLength = image.Code.Length, originalConstants = image.ConstOffsets.Count;
        Assert.Contains(ScriptObjectAliases.Load(game), alias => alias.Alias == "GER_HAU00" && alias.NameDef == "BauGerHau00_Haupthaus");

        LevelScriptInjector.Inject(image, [new ScenarioSpawn("GER_HAU00", 8000, 8000, 0), new ScenarioSpawn("GER_INF00", 8300, 8000, 0, Count: 10)]);
        byte[] injected = image.Serialize();
        BciImage reparsed = BciImage.Parse(injected);
        Assert.Equal(originalCodeLength, reparsed.MainAddress);
        Assert.Equal(original.AsSpan(0x24, originalCodeLength).ToArray(), reparsed.Code.AsSpan(0, originalCodeLength).ToArray());
        Assert.Equal("DEFSCRIPT", reparsed.Constant(originalConstants));
        Assert.Equal("s_createObj", reparsed.Constant(originalConstants + 1));
        int jump = reparsed.Code.Length - 8;
        Assert.Equal(112, BitConverter.ToInt32(reparsed.Code, jump));
        Assert.Equal(originalMain, jump + 8 + BitConverter.ToInt32(reparsed.Code, jump + 4));
        if (Environment.GetEnvironmentVariable("ARM_DUMP_TARGET") is { } target) File.WriteAllBytes(Path.Combine(target, "injected_endl.bci"), injected);
    }
}
