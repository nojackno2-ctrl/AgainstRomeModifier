using System.Numerics;
using AgainstRomeMapEditor.Modules.Cinematics;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Cinematics;

public sealed class CinematicsGuardTests
{
    [Fact]
    public void CinematicBciCompiler_Flags_ConfirmExperimentalAndUnwiredStatus()
    {
        Assert.True(CinematicBciCompiler.IsExperimental);
        Assert.False(CinematicBciCompiler.IsWiredToLevelScript);
    }

    [Fact]
    public void CinematicBciCompiler_CompileCameraCalls_ThrowsOnNullImage()
    {
        Assert.Throws<ArgumentNullException>(() =>
            CinematicBciCompiler.CompileCameraCalls(null!, 0, 0, 0, 5));
    }

    [Theory]
    [InlineData(double.NaN, 0, 0)]
    [InlineData(0, double.PositiveInfinity, 0)]
    [InlineData(0, 0, double.NegativeInfinity)]
    [InlineData(4e38, 0, 0)]
    public void CinematicBciCompiler_CompileCameraCalls_ThrowsOnNonFiniteOrOverflowCoordinates(double x, double y, double z)
    {
        var image = BciImage.CreateIdleLevel();
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            CinematicBciCompiler.CompileCameraCalls(image, x, y, z, 5));
    }

    [Theory]
    [InlineData(-0.01)]
    [InlineData(9.01)]
    [InlineData(double.NaN)]
    [InlineData(double.PositiveInfinity)]
    public void CinematicBciCompiler_CompileCameraCalls_ThrowsOnInvalidZoom(double zoom)
    {
        var image = BciImage.CreateIdleLevel();
        Assert.Throws<ArgumentOutOfRangeException>(() =>
            CinematicBciCompiler.CompileCameraCalls(image, 100, 20, 100, zoom));
    }

    [Fact]
    public void CinematicBciCompiler_CompileCameraCalls_AcceptsExactBoundaryZoomValues()
    {
        var image = BciImage.CreateIdleLevel();
        byte[] codeMin = CinematicBciCompiler.CompileCameraCalls(image, 100, 20, 100, 0.0);
        Assert.NotEmpty(codeMin);

        byte[] codeMax = CinematicBciCompiler.CompileCameraCalls(image, 100, 20, 100, 9.0);
        Assert.NotEmpty(codeMax);
    }

    [Fact]
    public void CinematicBciCompiler_CompileMessageCall_ThrowsOnNullArguments()
    {
        var image = BciImage.CreateIdleLevel();
        Assert.Throws<ArgumentNullException>(() => CinematicBciCompiler.CompileMessageCall(null!, "hello"));
        Assert.Throws<ArgumentNullException>(() => CinematicBciCompiler.CompileMessageCall(image, null!));
    }

    [Fact]
    public void CinematicBciCompiler_CompileToScenarioEvents_GuardAndFormatting()
    {
        Assert.Throws<ArgumentNullException>(() => CinematicBciCompiler.CompileToScenarioEvents(null!));

        var emptySeq = new CutsceneSequence { Name = "Empty" };
        var emptyEvents = CinematicBciCompiler.CompileToScenarioEvents(emptySeq);
        Assert.Empty(emptyEvents);

        var seq = new CutsceneSequence
        {
            Name = "FormatTest",
            Subtitles =
            [
                new SubtitleKeyframe(1.0f, 2.0f, "首領", "前進！"),
                new SubtitleKeyframe(4.0f, 2.0f, "", "無名說話")
            ]
        };

        var events = CinematicBciCompiler.CompileToScenarioEvents(seq);
        Assert.Equal(2, events.Count);
        Assert.Equal("[首領] 前進！", events[0].Actions[0].Text);
        Assert.Equal("無名說話", events[1].Actions[0].Text);
    }

    [Fact]
    public void CinematicBciCompiler_GenerateBciScriptText_GuardsAndClamping()
    {
        Assert.Throws<ArgumentNullException>(() => CinematicBciCompiler.GenerateBciScriptText(null!));

        var seq = new CutsceneSequence
        {
            Name = "Track1",
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(100, 0, 100), duration: 0f),
                new CameraWaypoint(new Vector3(200, 0, 200), duration: 2f)
            ]
        };

        // 步數被 clamp 到 2..10000
        string textMin = CinematicBciCompiler.GenerateBciScriptText(seq, sampleSteps: -5);
        Assert.Contains("EXPERIMENTAL / UNWIRED", textMin);
        Assert.Contains("s_lgcSetEnginePos", textMin);

        string textMax = CinematicBciCompiler.GenerateBciScriptText(seq, sampleSteps: 20000);
        Assert.Contains("EXPERIMENTAL / UNWIRED", textMax);
    }

    [Fact]
    public void CutsceneSequenceCatalog_GuardsAgainstNullAndInvalidIndex()
    {
        var catalog = new CutsceneSequenceCatalog();

        Assert.Throws<ArgumentNullException>(() => catalog.Add(null!));
        Assert.Throws<ArgumentNullException>(() => catalog.Load(null!));
        Assert.Throws<ArgumentNullException>(() => catalog.Replace(0, null!));

        Assert.Throws<ArgumentOutOfRangeException>(() => catalog.Replace(-1, new CutsceneSequence()));
        Assert.Throws<ArgumentOutOfRangeException>(() => catalog.Replace(0, new CutsceneSequence()));
        Assert.Throws<ArgumentOutOfRangeException>(() => catalog.RemoveAt(-1));
        Assert.Throws<ArgumentOutOfRangeException>(() => catalog.RemoveAt(0));
        Assert.Throws<ArgumentOutOfRangeException>(() => catalog.Duplicate(-1));
        Assert.Throws<ArgumentOutOfRangeException>(() => catalog.Duplicate(0));
    }

    [Fact]
    public void CutsceneSequenceCatalog_MaxCapacity64_ThrowsInvalidOperationException()
    {
        var catalog = new CutsceneSequenceCatalog();
        for (int i = 0; i < 64; i++)
        {
            catalog.Add(new CutsceneSequence { Name = $"Seq_{i}" });
        }
        Assert.Equal(64, catalog.Count);

        var exAdd = Assert.Throws<InvalidOperationException>(() =>
            catalog.Add(new CutsceneSequence { Name = "OverflowSeq" }));
        Assert.Contains("64", exAdd.Message);

        var exDup = Assert.Throws<InvalidOperationException>(() =>
            catalog.Duplicate(0));
        Assert.Contains("64", exDup.Message);
    }

    [Fact]
    public void CutsceneSequenceCatalog_FromJson_NullOrWhitespace_ReturnsEmptyCatalog()
    {
        var fromNull = CutsceneSequenceCatalog.FromJson(null!);
        Assert.Equal(0, fromNull.Count);

        var fromEmpty = CutsceneSequenceCatalog.FromJson("   ");
        Assert.Equal(0, fromEmpty.Count);
    }

    [Fact]
    public void CutsceneSequenceCatalog_GetById_NonExistentReturnsNull()
    {
        var catalog = new CutsceneSequenceCatalog();
        Assert.Null(catalog.GetById("non_existent_id"));
    }

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    public void CutsceneSequenceCatalog_Validate_ThrowsOnInvalidName(string invalidName)
    {
        var catalog = new CutsceneSequenceCatalog();
        catalog.Add(new CutsceneSequence { Name = invalidName });
        Assert.Throws<InvalidDataException>(() => catalog.Validate());
    }

    [Fact]
    public void CutsceneSequenceCatalog_Validate_ThrowsOnDuplicateId()
    {
        var catalog = new CutsceneSequenceCatalog();
        string sharedId = Guid.NewGuid().ToString("N");
        catalog.Add(new CutsceneSequence { Id = sharedId, Name = "Seq1" });
        catalog.Add(new CutsceneSequence { Id = sharedId, Name = "Seq2" });
        Assert.Throws<InvalidDataException>(() => catalog.Validate());
    }

    [Fact]
    public void CutsceneSequenceCatalog_Validate_ThrowsOnSingleWaypoint()
    {
        var catalog = new CutsceneSequenceCatalog();
        catalog.Add(new CutsceneSequence
        {
            Name = "SinglePoint",
            CameraWaypoints = [new CameraWaypoint(new Vector3(100, 0, 100))]
        });
        var ex = Assert.Throws<InvalidDataException>(() => catalog.Validate());
        Assert.Contains("至少需要 2 個路徑點", ex.Message);
    }

    [Theory]
    [InlineData(100, 0, 4f, 80f)]   // Pitch < 5
    [InlineData(100, 0, 92f, 80f)]  // Pitch > 89
    [InlineData(100, 0, 45f, 0.5f)] // Zoom < 1
    [InlineData(100, 0, 45f, 600f)] // Zoom > 500
    [InlineData(-5, 0, 45f, 80f)]   // X < 0
    [InlineData(17000, 0, 45f, 80f)]// X > 16384
    public void CutsceneSequenceCatalog_Validate_ThrowsOnInvalidWaypointParameters(
        float x, float z, float pitch, float zoom)
    {
        var catalog = new CutsceneSequenceCatalog();
        catalog.Add(new CutsceneSequence
        {
            Name = "BadWaypoint",
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(100, 0, 100)),
                new CameraWaypoint(new Vector3(x, 0, z), pitchDegrees: pitch, zoom: zoom)
            ]
        });
        Assert.Throws<InvalidDataException>(() => catalog.Validate());
    }

    [Fact]
    public void CutsceneSequenceCatalog_Validate_ThrowsOnInvalidSubtitle()
    {
        var catalog = new CutsceneSequenceCatalog();
        catalog.Add(new CutsceneSequence
        {
            Name = "BadSub",
            Subtitles = [new SubtitleKeyframe { Text = "Text", DurationSeconds = 0f }] // 繞過構造函式預設 0.5f 限制
        });
        Assert.Throws<InvalidDataException>(() => catalog.Validate());
    }

    [Fact]
    public void CutsceneSequenceCatalog_Validate_ThrowsOnInvalidUnitOrderTarget()
    {
        var catalog = new CutsceneSequenceCatalog();
        catalog.Add(new CutsceneSequence
        {
            Name = "BadOrder",
            UnitOrders = [new UnitOrderKeyframe(1.0f, "GER_WARRIOR", CutsceneUnitOrderKind.MoveTo, -50f, 500f)]
        });
        Assert.Throws<InvalidDataException>(() => catalog.Validate());
    }

    [Fact]
    public void CameraFlightPreviewer_GuardsAgainstNullPlay()
    {
        var previewer = new CameraFlightPreviewer();
        Assert.Throws<ArgumentNullException>(() => previewer.Play(null!));
    }

    [Fact]
    public void CameraFlightPreviewer_StateTransitions_PauseResumeStop()
    {
        var previewer = new CameraFlightPreviewer();
        Assert.Equal(CameraPreviewState.Stopped, previewer.State);

        // 在 Stopped 狀態調用 Pause / Resume 不應改變狀態
        previewer.Pause();
        Assert.Equal(CameraPreviewState.Stopped, previewer.State);

        previewer.Resume();
        Assert.Equal(CameraPreviewState.Stopped, previewer.State);

        var seq = new CutsceneSequence
        {
            Name = "Flight",
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(100, 0, 100), duration: 0f),
                new CameraWaypoint(new Vector3(500, 0, 500), duration: 5f)
            ]
        };

        previewer.Play(seq);
        Assert.Equal(CameraPreviewState.Playing, previewer.State);

        previewer.Pause();
        Assert.Equal(CameraPreviewState.Paused, previewer.State);

        // 暫停狀態下 Update 不應推進時間
        previewer.Update(1.0f);
        Assert.Equal(0f, previewer.CurrentTime);

        previewer.Resume();
        Assert.Equal(CameraPreviewState.Playing, previewer.State);

        previewer.Stop();
        Assert.Equal(CameraPreviewState.Stopped, previewer.State);
        Assert.Equal(0f, previewer.CurrentTime);
    }

    [Fact]
    public void CameraFlightPreviewer_ScrubTo_NoActiveSequence_DoesNotCrash()
    {
        var previewer = new CameraFlightPreviewer();
        previewer.ScrubTo(10f); // 無 active sequence
        Assert.Equal(0f, previewer.CurrentTime);
    }

    [Fact]
    public void CutsceneSequence_CalculateTotalDuration_EmptyAndTrackMax()
    {
        var empty = new CutsceneSequence();
        Assert.Equal(0f, empty.CalculateTotalDuration());

        var seq = new CutsceneSequence
        {
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(0, 0, 0), duration: 0f),
                new CameraWaypoint(new Vector3(100, 0, 100), duration: 3f)
            ],
            Subtitles = [new SubtitleKeyframe(4f, 2f, "A", "B")], // EndTime = 6s
            UnitOrders = [new UnitOrderKeyframe(7f, "UNIT", CutsceneUnitOrderKind.MoveTo, 100, 100)], // 7s
            FxEvents = [new CutsceneFxKeyframe(8.5f, CutsceneFxEventKind.FadeIn)] // 8.5s
        };

        Assert.Equal(8.5f, seq.CalculateTotalDuration());
    }
}
