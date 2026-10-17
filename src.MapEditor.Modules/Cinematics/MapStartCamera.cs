using System.Text.Json;
using System.Text.Json.Serialization;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeMapEditor.Modules.Cinematics;

/// <summary>
/// 開局相機使用的底層原生原語。
/// </summary>
public enum StartCameraPrimitive
{
    /// <summary>
    /// 使用原版戰役/多人地圖開局呼叫的 s_centerToPos(x, z) 搭配 s_lgcSetEngineZoom(zoom)。
    /// 引擎內部自動按地圖高度計算等角投影偏移 (engineX = x - height*2.0, engineZ = z - height*2.0, engineY = 0)。
    /// </summary>
    CenterToPos,

    /// <summary>
    /// 使用底層原生原語 s_lgcSetEnginePos(x, y, z) 搭配 s_lgcSetEngineZoom(zoom)。
    /// 直接設定引擎相機空間座標，y 為相機高度/海拔。
    /// </summary>
    DirectEnginePos
}

/// <summary>
/// 地圖開局相機設定 (Map Start Camera)，由企劃在編輯器中指定開局視角焦點與縮放。
/// 儲存於地圖目錄下的 arm_start_camera.json，並在儲存時注入至 ak_level.bci。
/// </summary>
public sealed record MapStartCamera
{
    public const string FileName = "arm_start_camera.json";
    public const double DefaultWorldX = 8192.0;
    public const double DefaultWorldZ = 8192.0;
    public const double DefaultElevation = 0.0;
    public const double DefaultZoom = 5.0;

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };

    /// <summary>是否啟用自訂開局相機。若為 false 則不注入腳本。</summary>
    public bool Enabled { get; init; } = true;

    /// <summary>世界座標 X (0..16383)。</summary>
    public double X { get; init; } = DefaultWorldX;

    /// <summary>海拔高度 Y / 相機高度 (預設 0.0)。</summary>
    public double Y { get; init; } = DefaultElevation;

    /// <summary>世界座標 Z (0..16383)。</summary>
    public double Z { get; init; } = DefaultWorldZ;

    /// <summary>原生引擎縮放 (0.0 .. 9.0)。</summary>
    public double Zoom { get; init; } = DefaultZoom;

    /// <summary>發射至腳本的原語類型。</summary>
    public StartCameraPrimitive Primitive { get; init; } = StartCameraPrimitive.CenterToPos;

    public void Validate()
    {
        if (!double.IsFinite(X) || X < 0 || X > 16383)
            throw new ArgumentOutOfRangeException(nameof(X), "X 座標必須在世界地圖範圍 0..16383 內。");
        if (!double.IsFinite(Z) || Z < 0 || Z > 16383)
            throw new ArgumentOutOfRangeException(nameof(Z), "Z 座標必須在世界地圖範圍 0..16383 內。");
        if (!double.IsFinite(Y) || Math.Abs(Y) > float.MaxValue)
            throw new ArgumentOutOfRangeException(nameof(Y), "高度必須為有限浮點數。");
        if (!double.IsFinite(Zoom) || Zoom < 0 || Zoom > 9)
            throw new ArgumentOutOfRangeException(nameof(Zoom), "原生縮放範圍必須為 0..9。");
    }

    public static MapStartCamera Load(string mapDirectory)
    {
        ArgumentNullException.ThrowIfNull(mapDirectory);
        string path = Path.Combine(mapDirectory, FileName);
        if (!File.Exists(path)) return new MapStartCamera { Enabled = false };
        string json = File.ReadAllText(path);
        var camera = JsonSerializer.Deserialize<MapStartCamera>(json, JsonOptions);
        return camera ?? new MapStartCamera { Enabled = false };
    }

    public void Save(string mapDirectory, FileRollbackScope? rollback = null)
    {
        ArgumentNullException.ThrowIfNull(mapDirectory);
        Validate();
        string path = Path.Combine(mapDirectory, FileName);
        byte[] bytes = JsonSerializer.SerializeToUtf8Bytes(this, JsonOptions);
        if (rollback != null)
            SafeFileWriter.WriteAllBytes(path, bytes, rollback);
        else
            File.WriteAllBytes(path, bytes);
    }
}
