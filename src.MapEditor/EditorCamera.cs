using System.Numerics;

namespace AgainstRomeMapEditor;

internal sealed class EditorCamera
{
    public Vector3 Target { get; set; } = new(32, 0, 32);
    // 預設對齊原遊戲等角視角：地圖 +X 往右下、+Z（地圖 y）往左下，原生 sprite 以此方向預先繪製。
    public float YawDegrees { get; private set; } = 45;
    public float PitchDegrees { get; private set; } = 30; // 遊戲為 2:1 等角（實測每世界單位 0.5,0.25 px），即俯角 30°
    public float Distance { get; private set; } = 82;
    public float MinDistance { get; init; } = 2;
    public float MaxDistance { get; init; } = 180;
    /// <summary>
    /// 正交投影（預設）與原遊戲的等角畫面一致：近大遠小會讓預先繪製的 sprite 與地形錯位。
    /// 此時 Distance 只決定可視高度（與相同距離的透視畫面中心一樣大），鏡頭本身放在遠處避免裁切地形。
    /// </summary>
    public bool Orthographic { get; set; } = true;
    private const float FieldOfViewDegrees = 52, OrthographicEyeDistance = 400;

    public void Rotate(float yawDelta, float pitchDelta)
    {
        YawDegrees += yawDelta;
        PitchDegrees = Math.Clamp(PitchDegrees + pitchDelta, 20, 80);
    }

    /// <summary>
    /// 原遊戲畫面每 tile（256 世界單位）在鏡頭水平方向的像素數：實測每世界單位沿地圖 X 為 (0.5, 0.25) px，
    /// 即水平軸 0.5·√2 px／單位。
    /// </summary>
    public const float GamePixelsPerTile = 256 * .70710678f;

    /// <summary>正交模式下把縮放設成與遊戲 1:1 的像素比例。</summary>
    public void ZoomToGameScale(int viewportHeight)
        => Distance = Math.Clamp(viewportHeight / GamePixelsPerTile / (2 * MathF.Tan(DegreesToRadians(FieldOfViewDegrees / 2))), MinDistance, MaxDistance);

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
            return Target + direction * (Orthographic ? OrthographicEyeDistance : Distance);
        }
    }

    public Matrix4x4 GetViewMatrix() => Matrix4x4.CreateLookAt(Position, Target, Vector3.UnitY);
    public Matrix4x4 GetProjectionMatrix(float aspectRatio)
    {
        float aspect = Math.Max(.1f, aspectRatio);
        if (!Orthographic) return Matrix4x4.CreatePerspectiveFieldOfView(DegreesToRadians(FieldOfViewDegrees), aspect, .1f, 500);
        float height = 2 * Distance * MathF.Tan(DegreesToRadians(FieldOfViewDegrees / 2));
        return Matrix4x4.CreateOrthographic(height * aspect, height, 1, OrthographicEyeDistance * 2);
    }
    public TerrainRay CreateRay(float x, float y, float width, float height)
    {
        Vector3 near = Unproject(x, y, 0, width, height);
        Vector3 far = Unproject(x, y, 1, width, height);
        return new TerrainRay(near, Vector3.Normalize(far - near));
    }

    private Vector3 Unproject(float x, float y, float z, float width, float height)
    {
        Vector4 clip = new(x / width * 2 - 1, 1 - y / height * 2, z * 2 - 1, 1);
        if (!Matrix4x4.Invert(GetViewMatrix() * GetProjectionMatrix(width / height), out Matrix4x4 inverse))
            return Target; // view×projection 理論上恆可逆；退化時回傳對焦點，避免 NaN 拾取。
        Vector4 world = Vector4.Transform(clip, inverse);
        return new Vector3(world.X, world.Y, world.Z) / world.W;
    }

    private static float DegreesToRadians(float value) => value * MathF.PI / 180f;
}
