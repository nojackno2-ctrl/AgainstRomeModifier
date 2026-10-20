using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using AgainstRomeMapEditor.Modules.Cinematics;

namespace AgainstRome.Campaign.CameraTrackOracle;

public sealed class OracleRequest
{
    public List<EasingQuery> EasingQueries { get; set; } = new();
    public List<CatmullRomQuery> CatmullRomQueries { get; set; } = new();
    public List<PlannerQuery> PlannerQueries { get; set; } = new();
    public List<SamplePathQuery> SamplePathQueries { get; set; } = new();
    public List<PreviewerQuery> PreviewerQueries { get; set; } = new();
    public List<StartCameraValidationQuery> StartCameraValidationQueries { get; set; } = new();
    public List<StartCameraJsonQuery> StartCameraJsonQueries { get; set; } = new();
}

public sealed class EasingQuery
{
    public int EasingType { get; set; }
    public float T { get; set; }
}

public sealed class CatmullRomQuery
{
    public float[] P0 { get; set; } = [];
    public float[] P1 { get; set; } = [];
    public float[] P2 { get; set; } = [];
    public float[] P3 { get; set; } = [];
    public float T { get; set; }
}

public sealed class WaypointDto
{
    public float[] Position { get; set; } = [8192f, 0f, 8192f];
    public float PitchDegrees { get; set; } = 30f;
    public float YawDegrees { get; set; } = 45f;
    public float Zoom { get; set; } = 82f;
    public float DurationFromPrevious { get; set; } = 3f;
    public int Easing { get; set; } = 1; // SmoothStep
    public string? Label { get; set; }
}

public sealed class PoseDto
{
    public float[] Position { get; set; } = [8192f, 0f, 8192f];
    public float PitchDegrees { get; set; }
    public float YawDegrees { get; set; }
    public float Zoom { get; set; }
    public float TimeSeconds { get; set; }
}

public sealed class PlannerQuery
{
    public string Name { get; set; } = "";
    public List<WaypointDto> Waypoints { get; set; } = new();
    public List<float> QueryTimes { get; set; } = new();
}

public sealed class SamplePathQuery
{
    public string Name { get; set; } = "";
    public List<WaypointDto> Waypoints { get; set; } = new();
    public int SampleCount { get; set; }
}

public sealed class PreviewerQuery
{
    public string Name { get; set; } = "";
    public List<WaypointDto> Waypoints { get; set; } = new();
    public List<SubtitleDto> Subtitles { get; set; } = new();
    public List<FxEventDto> FxEvents { get; set; } = new();
    public bool RestoreCameraOnComplete { get; set; } = true;
    public bool AutoRestoreCameraOnStop { get; set; } = true;
    public PoseDto? InitialCameraPose { get; set; }
    public float PlaybackSpeed { get; set; } = 1.0f;
    public List<PreviewerStepDto> Steps { get; set; } = new();
}

public sealed class SubtitleDto
{
    public float StartTime { get; set; }
    public float Duration { get; set; }
    public string Speaker { get; set; } = "";
    public string Text { get; set; } = "";
}

public sealed class FxEventDto
{
    public float TimeSeconds { get; set; }
    public int Kind { get; set; }
    public string Parameter { get; set; } = "";
}

public sealed class PreviewerStepDto
{
    public string Action { get; set; } = ""; // "update", "scrub", "pause", "resume", "stop"
    public float Value { get; set; }
    public bool Flag { get; set; }
}

public sealed class StartCameraValidationQuery
{
    public string X { get; set; } = "0";
    public string Y { get; set; } = "0";
    public string Z { get; set; } = "0";
    public string Zoom { get; set; } = "0";
}

public sealed class StartCameraJsonQuery
{
    public bool Enabled { get; set; }
    public double X { get; set; }
    public double Y { get; set; }
    public double Z { get; set; }
    public double Zoom { get; set; }
    public int Primitive { get; set; }
}

public sealed class OracleResponse
{
    public List<float> EasingResults { get; set; } = new();
    public List<float[]> CatmullRomResults { get; set; } = new();
    public List<PlannerResult> PlannerResults { get; set; } = new();
    public List<SamplePathResult> SamplePathResults { get; set; } = new();
    public List<PreviewerResult> PreviewerResults { get; set; } = new();
    public List<StartCameraValidationResult> StartCameraValidationResults { get; set; } = new();
    public List<StartCameraJsonResult> StartCameraJsonResults { get; set; } = new();
}

public sealed class PlannerResult
{
    public string Name { get; set; } = "";
    public float TotalDuration { get; set; }
    public int Count { get; set; }
    public List<PoseDto> Poses { get; set; } = new();
}

public sealed class SamplePathResult
{
    public string Name { get; set; } = "";
    public List<PoseDto> Samples { get; set; } = new();
}

public sealed class PreviewerResult
{
    public string Name { get; set; } = "";
    public List<PreviewerStepResult> StepResults { get; set; } = new();
}

public sealed class PreviewerStepResult
{
    public int State { get; set; }
    public float CurrentTime { get; set; }
    public PoseDto LastPose { get; set; } = new();
    public string? ActiveSubtitleText { get; set; }
    public int TriggeredFxCount { get; set; }
    public bool PlaybackFinished { get; set; }
}

public sealed class StartCameraValidationResult
{
    public bool IsValid { get; set; }
    public string? ErrorMessage { get; set; }
}

public sealed class StartCameraJsonResult
{
    public string SerializedJson { get; set; } = "";
    public bool DeserializedEnabled { get; set; }
    public double DeserializedX { get; set; }
    public double DeserializedY { get; set; }
    public double DeserializedZ { get; set; }
    public double DeserializedZoom { get; set; }
    public int DeserializedPrimitive { get; set; }
}

internal static class Program
{
    public static int Main(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("Usage: CameraTrackOracle <request.json> <response.json>");
            return 1;
        }

        string requestPath = args[0];
        string responsePath = args[1];

        var jsonOpts = new JsonSerializerOptions
        {
            PropertyNameCaseInsensitive = true,
            WriteIndented = true,
            DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
        };

        string requestJson = File.ReadAllText(requestPath);
        var request = JsonSerializer.Deserialize<OracleRequest>(requestJson, jsonOpts)
            ?? throw new InvalidOperationException("Failed to deserialize OracleRequest");

        var response = new OracleResponse();

        // 1. Easing Queries
        foreach (var q in request.EasingQueries)
        {
            var easing = (CameraEasingType)q.EasingType;
            float result = CameraTrackSplinePlanner.ApplyEasing(q.T, easing);
            response.EasingResults.Add(result);
        }

        // 2. CatmullRom Queries
        foreach (var q in request.CatmullRomQueries)
        {
            var p0 = new Vector3(q.P0[0], q.P0[1], q.P0[2]);
            var p1 = new Vector3(q.P1[0], q.P1[1], q.P1[2]);
            var p2 = new Vector3(q.P2[0], q.P2[1], q.P2[2]);
            var p3 = new Vector3(q.P3[0], q.P3[1], q.P3[2]);
            var v = CameraTrackSplinePlanner.EvaluateCentripetalCatmullRom(p0, p1, p2, p3, q.T);
            response.CatmullRomResults.Add([v.X, v.Y, v.Z]);
        }

        // 3. Planner Queries
        foreach (var q in request.PlannerQueries)
        {
            var waypoints = q.Waypoints.Select(ToWaypoint).ToList();
            var planner = new CameraTrackSplinePlanner(waypoints);

            var pRes = new PlannerResult
            {
                Name = q.Name,
                TotalDuration = planner.TotalDuration,
                Count = planner.Count,
                Poses = q.QueryTimes.Select(t => ToPoseDto(planner.Evaluate(t))).ToList()
            };
            response.PlannerResults.Add(pRes);
        }

        // 4. SamplePath Queries
        foreach (var q in request.SamplePathQueries)
        {
            var waypoints = q.Waypoints.Select(ToWaypoint).ToList();
            var planner = new CameraTrackSplinePlanner(waypoints);
            var samples = planner.SamplePath(q.SampleCount);

            var spRes = new SamplePathResult
            {
                Name = q.Name,
                Samples = samples.Select(ToPoseDto).ToList()
            };
            response.SamplePathResults.Add(spRes);
        }

        // 5. Previewer Queries
        foreach (var q in request.PreviewerQueries)
        {
            var seq = new CutsceneSequence
            {
                Name = q.Name,
                CameraWaypoints = q.Waypoints.Select(ToWaypoint).ToList(),
                Subtitles = q.Subtitles.Select(s => new SubtitleKeyframe(s.StartTime, s.Duration, s.Speaker, s.Text)).ToList(),
                FxEvents = q.FxEvents.Select(f => new CutsceneFxKeyframe(f.TimeSeconds, (CutsceneFxEventKind)f.Kind, f.Parameter)).ToList(),
                RestoreCameraOnComplete = q.RestoreCameraOnComplete
            };

            var previewer = new CameraFlightPreviewer
            {
                AutoRestoreCameraOnStop = q.AutoRestoreCameraOnStop,
                PlaybackSpeed = q.PlaybackSpeed
            };

            CameraPose? initialPose = q.InitialCameraPose != null ? ToCameraPose(q.InitialCameraPose) : null;
            CameraPose lastUpdatedPose = default;
            SubtitleKeyframe? lastSubtitle = null;
            int triggeredFxCount = 0;
            bool playbackFinished = false;

            previewer.CameraPoseUpdated += (_, pose) => lastUpdatedPose = pose;
            previewer.ActiveSubtitleChanged += (_, sub) => lastSubtitle = sub;
            previewer.FxEventTriggered += (_, _) => triggeredFxCount++;
            previewer.PlaybackFinished += (_, _) => playbackFinished = true;

            previewer.Play(seq, initialPose);

            var prevRes = new PreviewerResult { Name = q.Name };

            foreach (var step in q.Steps)
            {
                switch (step.Action.ToLowerInvariant())
                {
                    case "update":
                        previewer.Update(step.Value);
                        break;
                    case "scrub":
                        previewer.ScrubTo(step.Value);
                        break;
                    case "pause":
                        previewer.Pause();
                        break;
                    case "resume":
                        previewer.Resume();
                        break;
                    case "stop":
                        previewer.Stop(step.Flag);
                        break;
                }

                prevRes.StepResults.Add(new PreviewerStepResult
                {
                    State = (int)previewer.State,
                    CurrentTime = previewer.CurrentTime,
                    LastPose = ToPoseDto(lastUpdatedPose),
                    ActiveSubtitleText = lastSubtitle?.Text,
                    TriggeredFxCount = triggeredFxCount,
                    PlaybackFinished = playbackFinished
                });
            }

            response.PreviewerResults.Add(prevRes);
        }

        // 6. StartCamera Validation Queries
        foreach (var q in request.StartCameraValidationQueries)
        {
            try
            {
                double x = double.Parse(q.X, System.Globalization.CultureInfo.InvariantCulture);
                double y = double.Parse(q.Y, System.Globalization.CultureInfo.InvariantCulture);
                double z = double.Parse(q.Z, System.Globalization.CultureInfo.InvariantCulture);
                double zoom = double.Parse(q.Zoom, System.Globalization.CultureInfo.InvariantCulture);

                var cam = new MapStartCamera
                {
                    X = x,
                    Y = y,
                    Z = z,
                    Zoom = zoom
                };
                cam.Validate();
                response.StartCameraValidationResults.Add(new StartCameraValidationResult
                {
                    IsValid = true,
                    ErrorMessage = null
                });
            }
            catch (Exception ex)
            {
                response.StartCameraValidationResults.Add(new StartCameraValidationResult
                {
                    IsValid = false,
                    ErrorMessage = ex.Message
                });
            }
        }

        // 7. StartCamera JSON Queries
        foreach (var q in request.StartCameraJsonQueries)
        {
            var cam = new MapStartCamera
            {
                Enabled = q.Enabled,
                X = q.X,
                Y = q.Y,
                Z = q.Z,
                Zoom = q.Zoom,
                Primitive = (StartCameraPrimitive)q.Primitive
            };

            string tempDir = Path.Combine(Path.GetTempPath(), "arm_start_cam_oracle_" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(tempDir);
            try
            {
                cam.Save(tempDir);
                string jsonText = File.ReadAllText(Path.Combine(tempDir, MapStartCamera.FileName));
                var reloaded = MapStartCamera.Load(tempDir);

                response.StartCameraJsonResults.Add(new StartCameraJsonResult
                {
                    SerializedJson = jsonText,
                    DeserializedEnabled = reloaded.Enabled,
                    DeserializedX = reloaded.X,
                    DeserializedY = reloaded.Y,
                    DeserializedZ = reloaded.Z,
                    DeserializedZoom = reloaded.Zoom,
                    DeserializedPrimitive = (int)reloaded.Primitive
                });
            }
            finally
            {
                try { Directory.Delete(tempDir, true); } catch { }
            }
        }

        string responseJson = JsonSerializer.Serialize(response, jsonOpts);
        File.WriteAllText(responsePath, responseJson);
        return 0;
    }

    private static CameraWaypoint ToWaypoint(WaypointDto dto)
    {
        return new CameraWaypoint(
            new Vector3(dto.Position[0], dto.Position[1], dto.Position[2]),
            dto.PitchDegrees,
            dto.YawDegrees,
            dto.Zoom,
            dto.DurationFromPrevious,
            (CameraEasingType)dto.Easing,
            dto.Label
        );
    }

    private static PoseDto ToPoseDto(CameraPose pose)
    {
        return new PoseDto
        {
            Position = [pose.Position.X, pose.Position.Y, pose.Position.Z],
            PitchDegrees = pose.PitchDegrees,
            YawDegrees = pose.YawDegrees,
            Zoom = pose.Zoom,
            TimeSeconds = pose.TimeSeconds
        };
    }

    private static CameraPose ToCameraPose(PoseDto dto)
    {
        return new CameraPose(
            new Vector3(dto.Position[0], dto.Position[1], dto.Position[2]),
            dto.PitchDegrees,
            dto.YawDegrees,
            dto.Zoom,
            dto.TimeSeconds
        );
    }
}
