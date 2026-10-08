using System.Numerics;
using AgainstRomeMapEditor.Modules.Cinematics;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Cinematics;

public sealed class CinematicBciCompilerTests
{
    [Fact]
    public void CompileToScenarioEvents_ProducesChronologicallyOrderedEvents()
    {
        var seq = new CutsceneSequence
        {
            Name = "神廟突襲開場",
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(1000, 50, 1000), duration: 0f),
                new CameraWaypoint(new Vector3(2000, 50, 2000), duration: 5f)
            ],
            Subtitles =
            [
                new SubtitleKeyframe(1.0f, 3.0f, "祭司", "神明守護著我們的部落！"),
                new SubtitleKeyframe(5.0f, 2.0f, "勇士", "衝鋒！")
            ],
            OnCompleteTriggerEvent = "SpawnReinforcements"
        };

        var events = CinematicBciCompiler.CompileToScenarioEvents(seq);

        Assert.NotEmpty(events);
        Assert.Contains(events, e => e.Actions.Any(a => a.Text.Contains("神明守護著我們的部落！")));
        Assert.Contains(events, e => e.Actions.Any(a => a.Text.Contains("衝鋒！")));
        Assert.Contains(events, e => e.Name.Contains("完畢後續"));
    }

    [Fact]
    public void GenerateBciScriptText_EmitsValidNativeCallsAndStructure()
    {
        var seq = new CutsceneSequence
        {
            Id = "intro_01",
            Name = "大河渡口運鏡",
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(2000, 10, 3000), pitchDegrees: 30f, yawDegrees: 45f, zoom: 80f, duration: 0f),
                new CameraWaypoint(new Vector3(4000, 20, 5000), pitchDegrees: 40f, yawDegrees: 60f, zoom: 70f, duration: 4f)
            ],
            Subtitles =
            [
                new SubtitleKeyframe(2.0f, 2.0f, "探馬", "羅馬先鋒正在涉水渡河！")
            ],
            UnitOrders =
            [
                new UnitOrderKeyframe(2.0f, "CAV_SCOUT", CutsceneUnitOrderKind.MoveTo, 3500, 4500)
            ],
            DisablePlayerControl = true
        };

        string script = CinematicBciCompiler.GenerateBciScriptText(seq, sampleSteps: 4);

        Assert.Contains("cutscene_intro_01_main", script);
        Assert.Contains("s_disableGUI", script);
        Assert.Contains("s_lgcSetEnginePos", script);
        Assert.Contains("s_lgcSetEngineZoom", script);
        Assert.Contains("s_showTextBox", script);
        Assert.Contains("s_conMoveTo", script);
        Assert.Contains("s_conWaitTime", script);
    }
}
