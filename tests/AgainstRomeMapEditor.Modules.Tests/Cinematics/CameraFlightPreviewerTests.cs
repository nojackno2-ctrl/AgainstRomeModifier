using System.Numerics;
using AgainstRomeMapEditor.Modules.Cinematics;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Cinematics;

public sealed class CameraFlightPreviewerTests
{
    [Fact]
    public void Play_StartsPlaybackAndFiresInitialPose()
    {
        var previewer = new CameraFlightPreviewer();
        var seq = new CutsceneSequence
        {
            Name = "開場試飛",
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(1000, 0, 1000), duration: 0f),
                new CameraWaypoint(new Vector3(2000, 0, 2000), duration: 5f)
            ]
        };

        CameraPose? capturedPose = null;
        previewer.CameraPoseUpdated += (_, pose) => capturedPose = pose;

        var originalPose = new CameraPose(new Vector3(500, 0, 500), 30f, 45f, 82f, 0f);
        previewer.Play(seq, originalPose);

        Assert.Equal(CameraPreviewState.Playing, previewer.State);
        Assert.Equal(0f, previewer.CurrentTime);
        Assert.NotNull(capturedPose);
        Assert.Equal(new Vector3(1000, 0, 1000), capturedPose.Value.Position);
    }

    [Fact]
    public void Update_AdvancesTimeAndTriggersPoseUpdates()
    {
        var previewer = new CameraFlightPreviewer();
        var seq = new CutsceneSequence
        {
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(1000, 0, 1000), duration: 0f),
                new CameraWaypoint(new Vector3(2000, 0, 2000), duration: 4f)
            ]
        };

        previewer.Play(seq);
        previewer.Update(2.0f);

        Assert.Equal(2.0f, previewer.CurrentTime);
        Assert.Equal(CameraPreviewState.Playing, previewer.State);
    }

    [Fact]
    public void Update_ReachingEnd_CompletesPlaybackAndRestoresCamera()
    {
        var previewer = new CameraFlightPreviewer();
        var seq = new CutsceneSequence
        {
            RestoreCameraOnComplete = true,
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(1000, 0, 1000), duration: 0f),
                new CameraWaypoint(new Vector3(2000, 0, 2000), duration: 2f)
            ]
        };

        var originalPose = new CameraPose(new Vector3(9999, 0, 9999), 30f, 45f, 82f, 0f);
        CameraPose? lastPose = null;
        bool finishedFired = false;

        previewer.CameraPoseUpdated += (_, pose) => lastPose = pose;
        previewer.PlaybackFinished += (_, _) => finishedFired = true;

        previewer.Play(seq, originalPose);
        previewer.Update(2.5f); // 超過總時間

        Assert.True(finishedFired);
        Assert.Equal(CameraPreviewState.Stopped, previewer.State);
        Assert.NotNull(lastPose);
        Assert.Equal(originalPose.Position, lastPose.Value.Position); // 已成功還原
    }

    [Fact]
    public void SubtitleTrack_TriggersActiveSubtitleEvent()
    {
        var previewer = new CameraFlightPreviewer();
        var seq = new CutsceneSequence
        {
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(1000, 0, 1000), duration: 0f),
                new CameraWaypoint(new Vector3(2000, 0, 2000), duration: 5f)
            ],
            Subtitles =
            [
                new SubtitleKeyframe(1.0f, 2.0f, "族長", "準備伏擊！")
            ]
        };

        SubtitleKeyframe? currentSubtitle = null;
        previewer.ActiveSubtitleChanged += (_, sub) => currentSubtitle = sub;

        previewer.Play(seq);
        Assert.Null(currentSubtitle);

        // 推進到 1.5 秒（字幕區間內）
        previewer.Update(1.5f);
        Assert.NotNull(currentSubtitle);
        Assert.Equal("準備伏擊！", currentSubtitle.Text);

        // 推進到 3.5 秒（字幕結束）
        previewer.Update(2.0f);
        Assert.Null(currentSubtitle);
    }

    [Fact]
    public void ScrubTo_ChangesCurrentTimeAccurately()
    {
        var previewer = new CameraFlightPreviewer();
        var seq = new CutsceneSequence
        {
            CameraWaypoints =
            [
                new CameraWaypoint(new Vector3(1000, 0, 1000), duration: 0f),
                new CameraWaypoint(new Vector3(3000, 0, 3000), duration: 10f)
            ]
        };

        previewer.Play(seq);
        previewer.ScrubTo(5.0f);

        Assert.Equal(5.0f, previewer.CurrentTime);
    }
}
