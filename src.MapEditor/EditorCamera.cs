using System.Numerics;

namespace AgainstRomeMapEditor;

internal sealed class EditorCamera
{
    public Vector3 Target { get; set; } = new(32, 0, 32);
    public float YawDegrees { get; private set; } = -45;
    public float PitchDegrees { get; private set; } = 55;
    public float Distance { get; private set; } = 82;
    public float MinDistance { get; init; } = 8;
    public float MaxDistance { get; init; } = 180;

    public void Rotate(float yawDelta, float pitchDelta)
    {
        YawDegrees += yawDelta;
        PitchDegrees = Math.Clamp(PitchDegrees + pitchDelta, 20, 80);
    }

    public void Zoom(float factor) => Distance = Math.Clamp(Distance * factor, MinDistance, MaxDistance);
    public void Pan(float right, float forward)
    {
        float yaw = DegreesToRadians(YawDegrees);
        Vector3 horizontalForward = Vector3.Normalize(new Vector3(MathF.Sin(yaw), 0, MathF.Cos(yaw)));
        Vector3 horizontalRight = new(horizontalForward.Z, 0, -horizontalForward.X);
        Target += horizontalRight * right + horizontalForward * forward;
    }

    public Vector3 Position
    {
        get
        {
            float yaw = DegreesToRadians(YawDegrees), pitch = DegreesToRadians(PitchDegrees);
            Vector3 direction = new(MathF.Cos(pitch) * MathF.Sin(yaw), MathF.Sin(pitch), MathF.Cos(pitch) * MathF.Cos(yaw));
            return Target + direction * Distance;
        }
    }

    public Matrix4x4 GetViewMatrix() => Matrix4x4.CreateLookAt(Position, Target, Vector3.UnitY);
    public Matrix4x4 GetProjectionMatrix(float aspectRatio) => Matrix4x4.CreatePerspectiveFieldOfView(DegreesToRadians(52), Math.Max(.1f, aspectRatio), .1f, 500);
    public TerrainRay CreateRay(float x, float y, float width, float height)
    {
        Vector3 near = Unproject(x, y, 0, width, height);
        Vector3 far = Unproject(x, y, 1, width, height);
        return new TerrainRay(near, Vector3.Normalize(far - near));
    }

    private Vector3 Unproject(float x, float y, float z, float width, float height)
    {
        Vector4 clip = new(x / width * 2 - 1, 1 - y / height * 2, z * 2 - 1, 1);
        Matrix4x4.Invert(GetViewMatrix() * GetProjectionMatrix(width / height), out Matrix4x4 inverse);
        Vector4 world = Vector4.Transform(clip, inverse);
        return new Vector3(world.X, world.Y, world.Z) / world.W;
    }

    private static float DegreesToRadians(float value) => value * MathF.PI / 180f;
}
