using System.Text;
using AgainstRomeMapEditor.Modules.Cinematics;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

/// <summary>開局相機必須經由存檔管線從原版備份重建注入：可重複儲存、與事件並存、失敗可回滾。</summary>
public sealed class StartCameraSavePipelineTests : IDisposable
{
    private static readonly string[] Aliases = ["GER_INF01"];
    private readonly string _map = Path.Combine(Path.GetTempPath(), "ArmStartCamera_" + Guid.NewGuid().ToString("N"));
    private readonly MapStartCamera _camera = new() { Enabled = true, X = 12512, Z = 14816, Zoom = 5 };

    public StartCameraSavePipelineTests()
    {
        Directory.CreateDirectory(Path.Combine(_map, "SCRIPT"));
        File.WriteAllBytes(ScriptPath, ScenarioEventsTests.Fixture().Serialize());
    }

    private string ScriptPath => Path.Combine(_map, "SCRIPT", LevelScriptInjector.ScriptFile);
    private Action<BciImage> Prologue => image => MapStartCameraInjector.Inject(image, _camera);

    private void Save(ScenarioDocument scenario, Action<BciImage>? prologue)
    {
        using var rollback = new FileRollbackScope();
        LevelScriptInjector.Apply(_map, scenario, Aliases, rollback, prologue);
        rollback.Commit();
    }

    private static int Occurrences(string script, string constant)
    {
        byte[] blob = BciImage.Parse(File.ReadAllBytes(script)).ConstBlob;
        string text = Encoding.Latin1.GetString(blob);
        return text.Split('\0').Count(item => item == constant);
    }

    [Fact]
    public void Repeated_saves_inject_camera_exactly_once()
    {
        Save(new ScenarioDocument(), Prologue);
        byte[] first = File.ReadAllBytes(ScriptPath);
        Save(new ScenarioDocument(), Prologue);
        Assert.Equal(first, File.ReadAllBytes(ScriptPath));
        Assert.Equal(1, Occurrences(ScriptPath, "s_centerToPos"));
    }

    [Fact]
    public void Camera_and_events_coexist_and_camera_runs_first()
    {
        var scenario = new ScenarioDocument
        {
            Events = [new("Timer", 2, false) { Actions = [new(ScenarioActionKind.Message, "Hi")] }]
        };
        Save(scenario, null);
        int eventsOnlyMain = BciImage.Parse(File.ReadAllBytes(ScriptPath)).MainAddress;
        Save(scenario, Prologue);
        var image = BciImage.Parse(File.ReadAllBytes(ScriptPath));
        Assert.Equal(1, Occurrences(ScriptPath, "s_centerToPos"));
        Assert.NotEqual(eventsOnlyMain, image.MainAddress);
        // Saving again without a camera restores the events-only script.
        Save(scenario, null);
        Assert.Equal(0, Occurrences(ScriptPath, "s_centerToPos"));
        Assert.Equal(eventsOnlyMain, BciImage.Parse(File.ReadAllBytes(ScriptPath)).MainAddress);
    }

    [Fact]
    public void Disabling_camera_without_events_restores_original_script()
    {
        byte[] original = File.ReadAllBytes(ScriptPath);
        Save(new ScenarioDocument(), Prologue);
        Save(new ScenarioDocument(), null);
        Assert.Equal(original, File.ReadAllBytes(ScriptPath));
    }

    [Fact]
    public void Failed_transaction_rolls_back_camera_injection()
    {
        byte[] original = File.ReadAllBytes(ScriptPath);
        using (var rollback = new FileRollbackScope())
            LevelScriptInjector.Apply(_map, new ScenarioDocument(), Aliases, rollback, Prologue);
        Assert.Equal(original, File.ReadAllBytes(ScriptPath));
        Assert.False(File.Exists(Path.Combine(_map, "SCRIPT", LevelScriptInjector.OriginalBackupFile)));
    }

    public void Dispose()
    {
        try { Directory.Delete(_map, true); } catch (IOException) { }
    }
}
