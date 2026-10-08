using System.Numerics;
using AgainstRomeMapEditor.Modules.Cinematics;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Cinematics;

public sealed class CameraTrackSplinePlannerTests
{
    [Fact]
    public void Evaluate_EmptyWaypoints_ReturnsSafeDefaultPose()
    {
        var planner = new CameraTrackSplinePlanner();
        var pose = planner.Evaluate(5.0f);

        Assert.Equal(0, planner.Count);
        Assert.Equal(0f, planner.TotalDuration);
        Assert.Equal(new Vector3(8192f, 0f, 8192f), pose.Position);
        Assert.Equal(30f, pose.PitchDegrees);
        Assert.Equal(45f, pose.YawDegrees);
    }

    [Fact]
    public void Evaluate_SingleWaypoint_ReturnsConstantPose()
    {
        var wp = new CameraWaypoint(new Vector3(1000f, 50f, 2000f), pitchDegrees: 40f, yawDegrees: 90f, zoom: 60f);
        var planner = new CameraTrackSplinePlanner([wp]);

        var poseStart = planner.Evaluate(0f);
        var poseLater = planner.Evaluate(10f);

        Assert.Equal(wp.Position, poseStart.Position);
        Assert.Equal(wp.PitchDegrees, poseStart.PitchDegrees);
        Assert.Equal(wp.Position, poseLater.Position);
    }

    [Fact]
    public void Evaluate_Waypoints_PassesAccuratelyThroughControlPoints()
    {
        var wp1 = new CameraWaypoint(new Vector3(1000f, 0f, 1000f), pitchDegrees: 30f, yawDegrees: 45f, zoom: 80f, duration: 0f);
        var wp2 = new CameraWaypoint(new Vector3(3000f, 100f, 2000f), pitchDegrees: 45f, yawDegrees: 90f, zoom: 60f, duration: 4f);
        var wp3 = new CameraWaypoint(new Vector3(5000f, 50f, 4000f), pitchDegrees: 35f, yawDegrees: 120f, zoom: 70f, duration: 3f);

        var planner = new CameraTrackSplinePlanner([wp1, wp2, wp3]);

        Assert.Equal(7.0f, planner.TotalDuration);

        // 評估 t = 0 (wp1)
        var pose0 = planner.Evaluate(0f);
        Assert.True(Vector3.Distance(wp1.Position, pose0.Position) < 0.1f);

        // 評估 t = 4 (wp2)
        var pose1 = planner.Evaluate(4.0f);
        Assert.True(Vector3.Distance(wp2.Position, pose1.Position) < 0.1f);
        Assert.InRange(pose1.PitchDegrees, 44.9f, 45.1f);

        // 評估 t = 7 (wp3)
        var pose2 = planner.Evaluate(7.0f);
        Assert.True(Vector3.Distance(wp3.Position, pose2.Position) < 0.1f);
    }

    [Fact]
    public void Evaluate_ShortestArcAngleInterpolation_WrapsAroundZeroDegreesSmoothly()
    {
        // 測試從 350° 轉向 10°（最短弧為順時針轉 20°，中點應在 0°/360°）
        var wp1 = new CameraWaypoint(new Vector3(1000f, 0f, 1000f), yawDegrees: 350f, duration: 0f);
        var wp2 = new CameraWaypoint(new Vector3(2000f, 0f, 2000f), yawDegrees: 10f, duration: 2f, easing: CameraEasingType.Linear);

        var planner = new CameraTrackSplinePlanner([wp1, wp2]);

        // 中間時刻 t = 1.0s，Yaw 應為 0° (或 360°)
        var midPose = planner.Evaluate(1.0f);
        Assert.InRange(midPose.YawDegrees, 0f, 0.01f);
    }

    [Fact]
    public void SamplePath_GeneratesRequestedNumberOfSamples()
    {
        var wp1 = new CameraWaypoint(new Vector3(1000f, 0f, 1000f), duration: 0f);
        var wp2 = new CameraWaypoint(new Vector3(2000f, 0f, 2000f), duration: 2f);
        var wp3 = new CameraWaypoint(new Vector3(3000f, 0f, 3000f), duration: 2f);

        var planner = new CameraTrackSplinePlanner([wp1, wp2, wp3]);
        var samples = planner.SamplePath(11);

        Assert.Equal(11, samples.Count);
        Assert.True(Vector3.Distance(wp1.Position, samples[0].Position) < 0.1f);
        Assert.True(Vector3.Distance(wp3.Position, samples[^1].Position) < 0.1f);
    }

    [Fact]
    public void ApplyEasing_CalculatesExpectedBoundaryAndMidpointValues()
    {
        Assert.Equal(0f, CameraTrackSplinePlanner.ApplyEasing(0f, CameraEasingType.SmoothStep));
        Assert.Equal(1f, CameraTrackSplinePlanner.ApplyEasing(1f, CameraEasingType.SmoothStep));
        Assert.Equal(0.5f, CameraTrackSplinePlanner.ApplyEasing(0.5f, CameraEasingType.SmoothStep));

        // EaseInQuad 0.5 -> 0.25
        Assert.Equal(0.25f, CameraTrackSplinePlanner.ApplyEasing(0.5f, CameraEasingType.EaseInQuad));

        // EaseOutQuad 0.5 -> 0.75
        Assert.Equal(0.75f, CameraTrackSplinePlanner.ApplyEasing(0.5f, CameraEasingType.EaseOutQuad));
    }
}
