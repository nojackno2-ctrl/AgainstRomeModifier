using System.Buffers.Binary;
using System.Text;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Cinematics;

/// <summary>
/// 已核對 EXE ABI 的底層呼叫片段；完整序列仍為實驗性、未接入存檔。
/// 不推測航點 Zoom、Pitch、Yaw 與原生引擎參數的對應。見 cinematic-camera.md。
/// </summary>
public static class CinematicBciCompiler
{
    public const bool IsExperimental = true;
    public const bool IsWiredToLevelScript = false;

    /// <summary>
    /// 產生位置與縮放 statement 的 CODE 片段，並登錄 image 常數；不附加 CODE、修改 main 或注入等待。
    /// 三個位置參數沿用原生引擎順序，zoom 為原生 0..9，不是 CameraWaypoint 的預覽距離。
    /// </summary>
    public static byte[] CompileCameraCalls(BciImage image, double engineX, double engineY, double engineZ, double engineZoom)
    {
        ArgumentNullException.ThrowIfNull(image);
        foreach (double value in new[] { engineX, engineY, engineZ })
            if (!double.IsFinite(value) || Math.Abs(value) > float.MaxValue)
                throw new ArgumentOutOfRangeException(nameof(engineX), "位置必須能以原生 float 表示。");
        if (!double.IsFinite(engineZoom) || engineZoom < 0 || engineZoom > 9)
            throw new ArgumentOutOfRangeException(nameof(engineZoom), "原生縮放範圍為 0..9；預覽距離尚無換算證據。");

        using var code = new MemoryStream();
        void Word(int value) { Span<byte> bytes = stackalloc byte[4]; BinaryPrimitives.WriteInt32LittleEndian(bytes, value); code.Write(bytes); }
        void Double(double value)
        {
            long bits = BitConverter.DoubleToInt64Bits(value);
            Word(67); Word(unchecked((int)bits)); Word(unchecked((int)(bits >> 32)));
        }
        // VM 0x5b1700 reads arguments from the stack top; each double occupies two words.
        Double(engineZ); Double(engineY); Double(engineX);
        Word(128); Word(NativeConstant(image, "s_lgcSetEnginePos")); Word(73); Word(-6);
        Double(engineZoom);
        Word(128); Word(NativeConstant(image, "s_lgcSetEngineZoom")); Word(73); Word(-2);
        // Void statements do not emit opcode 86 (which reads the native return register).
        return code.ToArray();
    }

    /// <summary>
    /// 編譯原版地圖開局使用的 s_centerToPos(x, z) 呼叫。
    /// 參數順序：BCI 棧頂為 X (arg0)，次層為 Z (arg1)。
    /// 單位為世界座標 (0..16383)，由引擎內部查詢地表高度並轉化為相機空間位置。
    /// </summary>
    public static byte[] CompileCenterToPosCall(BciImage image, int x, int z)
    {
        ArgumentNullException.ThrowIfNull(image);
        if (x < 0 || x > 16383) throw new ArgumentOutOfRangeException(nameof(x), "X 必須在 0..16383 範圍內。");
        if (z < 0 || z > 16383) throw new ArgumentOutOfRangeException(nameof(z), "Z 必須在 0..16383 範圍內。");

        int[] words = [
            66, z,
            66, x,
            128, NativeConstant(image, "s_centerToPos"),
            73, -2
        ];

        var code = new byte[words.Length * 4];
        for (int i = 0; i < words.Length; i++) BinaryPrimitives.WriteInt32LittleEndian(code.AsSpan(i * 4), words[i]);
        return code;
    }

    /// <summary>
    /// 依據 MapStartCamera 設定編譯開局相機 bytecode 片段。
    /// 支援 s_centerToPos + s_lgcSetEngineZoom 或 s_lgcSetEnginePos + s_lgcSetEngineZoom。
    /// </summary>
    public static byte[] CompileStartCameraCalls(BciImage image, MapStartCamera camera)
    {
        ArgumentNullException.ThrowIfNull(image);
        ArgumentNullException.ThrowIfNull(camera);
        camera.Validate();

        if (camera.Primitive == StartCameraPrimitive.CenterToPos)
        {
            using var ms = new MemoryStream();
            byte[] centerCode = CompileCenterToPosCall(image, (int)Math.Round(camera.X), (int)Math.Round(camera.Z));
            ms.Write(centerCode);

            void Word(int value) { Span<byte> bytes = stackalloc byte[4]; BinaryPrimitives.WriteInt32LittleEndian(bytes, value); ms.Write(bytes); }
            long bits = BitConverter.DoubleToInt64Bits(camera.Zoom);
            Word(67); Word(unchecked((int)bits)); Word(unchecked((int)(bits >> 32)));
            Word(128); Word(NativeConstant(image, "s_lgcSetEngineZoom")); Word(73); Word(-2);
            return ms.ToArray();
        }
        else
        {
            return CompileCameraCalls(image, camera.X, camera.Y, camera.Z, camera.Zoom);
        }
    }

    /// <summary>與原版 ENDL 的訊息 statement 相同；字串使用遊戲編碼，沒有字幕時長/語音/強制彈出語意。</summary>
    public static byte[] CompileMessageCall(BciImage image, string text)
    {
        ArgumentNullException.ThrowIfNull(image);
        ArgumentNullException.ThrowIfNull(text);
        int textIndex = image.AddGameConstant(text);
        int[] words = [76, textIndex, 66, 0, 128, NativeConstant(image, "s_showTextBox"), 73, -2];
        var code = new byte[words.Length * 4];
        for (int i = 0; i < words.Length; i++) BinaryPrimitives.WriteInt32LittleEndian(code.AsSpan(i * 4), words[i]);
        return code;
    }

    internal static int NativeConstant(BciImage image, string name)
    {
        for (int i = 0; i < image.ConstOffsets.Count; i++)
            if (image.Constant(i) == name) return i;
        return image.AddConstant(name);
    }

    /// <summary>
    /// 僅把字幕轉為一次性 Message 事件。DelaySeconds 是從開局算起，非前一字幕的相對延遲。
    /// 不執行相機、輸入鎖定、黑邊、語音、部隊或後續觸發；沒有字幕就沒有事件。
    /// </summary>
    public static IReadOnlyList<ScenarioEvent> CompileToScenarioEvents(CutsceneSequence sequence)
    {
        ArgumentNullException.ThrowIfNull(sequence);
        return sequence.Subtitles.OrderBy(s => s.StartTimeSeconds).Select((sub, index) =>
            new ScenarioEvent($"{sequence.Name}_字幕_{index + 1}", checked((int)Math.Round(sub.StartTimeSeconds)), false, true)
            {
                Actions = [new(ScenarioActionKind.Message, Text: string.IsNullOrWhiteSpace(sub.Speaker) ? sub.Text : $"[{sub.Speaker}] {sub.Text}")]
            }).ToArray();
    }

    /// <summary>人類可讀的預覽規劃，不是 IPR 原始碼或可執行 BCI；所有輸出皆為註解。</summary>
    public static string GenerateBciScriptText(CutsceneSequence sequence, int sampleSteps = 8)
    {
        ArgumentNullException.ThrowIfNull(sequence);
        var sb = new StringBuilder();
        sb.AppendLine("// EXPERIMENTAL / UNWIRED: planning preview only; not executable IPR or BCI.");
        sb.AppendLine($"// Sequence: {sequence.Name} ({sequence.Id})");
        sb.AppendLine("// Verified ABI: s_lgcSetEnginePos v(ddd); s_lgcSetEngineZoom v(d), native zoom 0..9.");
        sb.AppendLine("// Preview Zoom/Pitch/Yaw mapping, timing, letterbox, player control, restore, FX and orders are unverified.");
        var planner = new CameraTrackSplinePlanner(sequence.CameraWaypoints);
        float duration = Math.Max(sequence.CalculateTotalDuration(), planner.TotalDuration);
        int steps = Math.Clamp(sampleSteps, 2, 10000);
        for (int i = 0; i <= steps; i++)
        {
            CameraPose pose = planner.Evaluate(i * duration / steps);
            sb.AppendLine(FormattableString.Invariant($"// t={pose.TimeSeconds:F2}s preview position=({pose.Position.X:F1}, {pose.Position.Y:F1}, {pose.Position.Z:F1}) pitch={pose.PitchDegrees:F1} yaw={pose.YawDegrees:F1} preview distance={pose.Zoom:F1}"));
        }
        return sb.ToString();
    }
}
