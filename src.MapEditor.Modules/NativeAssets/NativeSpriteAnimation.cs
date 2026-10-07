namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>動畫 0 的唯讀循環序列；裁切錨點皆相對同一地面接觸點。</summary>
internal sealed class NativeSpriteAnimation
{
    public NativeSpriteAnimation(IEnumerable<NativeSprite> frames, int cycleDurationMs)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(cycleDurationMs);
        NativeSprite[] snapshot = frames.ToArray();
        if (snapshot.Length < 2) throw new ArgumentException("動畫至少需要兩格。", nameof(frames));
        Frames = Array.AsReadOnly(snapshot);
        CycleDurationMs = cycleDurationMs;
    }

    public IReadOnlyList<NativeSprite> Frames { get; }
    public int CycleDurationMs { get; }
    public double FrameDurationMs => CycleDurationMs / (double)Frames.Count;

    public int GetFrameIndex(double elapsedMs)
        => !double.IsFinite(elapsedMs) || elapsedMs <= 0 ? 0 :
            Math.Min(Frames.Count - 1, (int)((elapsedMs % CycleDurationMs) * Frames.Count / CycleDurationMs));
}
