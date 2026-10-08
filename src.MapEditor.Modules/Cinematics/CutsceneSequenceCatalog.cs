using System.Text.Json;
using System.Text.Json.Serialization;

namespace AgainstRomeMapEditor.Modules.Cinematics;

/// <summary>
/// 歷史戰役過場動畫目錄模組 (Cutscene Sequence Catalog)。
/// 負責管理地圖中所有的相機運鏡與過場導演序列，支援驗證、髒標記追蹤、複製與 JSON 序列化。
/// </summary>
public sealed class CutsceneSequenceCatalog : IEditorModule<IReadOnlyList<CutsceneSequence>>
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };

    private readonly List<CutsceneSequence> _sequences;
    private IReadOnlyList<CutsceneSequence> _baseline = Array.Empty<CutsceneSequence>();

    public string ModuleId => "cutscenes";
    public int Count => _sequences.Count;
    public IReadOnlyList<CutsceneSequence> Baseline => DeepCloneList(_baseline);

    public bool IsDirty
    {
        get
        {
            if (_sequences.Count != _baseline.Count) return true;
            for (int i = 0; i < _sequences.Count; i++)
            {
                var a = _sequences[i];
                var b = _baseline[i];
                if (a.Id != b.Id || a.Name != b.Name || a.AutoLetterbox != b.AutoLetterbox
                    || a.DisablePlayerControl != b.DisablePlayerControl
                    || a.RestoreCameraOnComplete != b.RestoreCameraOnComplete
                    || a.OnCompleteTriggerEvent != b.OnCompleteTriggerEvent)
                    return true;
                if (a.CameraWaypoints.Count != b.CameraWaypoints.Count
                    || a.Subtitles.Count != b.Subtitles.Count
                    || a.UnitOrders.Count != b.UnitOrders.Count
                    || a.FxEvents.Count != b.FxEvents.Count)
                    return true;
            }
            return false;
        }
    }

    public CutsceneSequenceCatalog() : this(new List<CutsceneSequence>()) { }

    public CutsceneSequenceCatalog(IEnumerable<CutsceneSequence> sequences)
    {
        _sequences = sequences.Select(Clone).ToList();
        AcceptBaseline(_sequences);
    }

    public void Load(IReadOnlyList<CutsceneSequence> snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        _sequences.Clear();
        _sequences.AddRange(DeepCloneList(snapshot));
        AcceptChanges();
    }

    public IReadOnlyList<CutsceneSequence> Capture() => DeepCloneList(_sequences);

    public void AcceptChanges() => AcceptBaseline(_sequences);

    public void AcceptBaseline(IReadOnlyList<CutsceneSequence> snapshot) => _baseline = DeepCloneList(snapshot);

    public void Reset()
    {
        _sequences.Clear();
        _sequences.AddRange(DeepCloneList(_baseline));
    }

    public int Add(CutsceneSequence item)
    {
        ArgumentNullException.ThrowIfNull(item);
        if (_sequences.Count >= 64) throw new InvalidOperationException("單張地圖過場動畫上限為 64 個。");
        _sequences.Add(Clone(item));
        return _sequences.Count - 1;
    }

    public void Replace(int index, CutsceneSequence item)
    {
        ArgumentNullException.ThrowIfNull(item);
        if (index < 0 || index >= _sequences.Count) throw new ArgumentOutOfRangeException(nameof(index));
        _sequences[index] = Clone(item);
    }

    public void RemoveAt(int index)
    {
        if (index < 0 || index >= _sequences.Count) throw new ArgumentOutOfRangeException(nameof(index));
        _sequences.RemoveAt(index);
    }

    public int Duplicate(int index, string suffix = " (副本)")
    {
        if (index < 0 || index >= _sequences.Count) throw new ArgumentOutOfRangeException(nameof(index));
        if (_sequences.Count >= 64) throw new InvalidOperationException("單張地圖過場動畫上限為 64 個。");

        var source = _sequences[index];
        var copy = Clone(source) with
        {
            Id = Guid.NewGuid().ToString("N"),
            Name = source.Name + suffix
        };
        _sequences.Insert(index + 1, copy);
        return index + 1;
    }

    public CutsceneSequence? GetById(string id) => _sequences.FirstOrDefault(s => s.Id.Equals(id, StringComparison.OrdinalIgnoreCase));

    /// <summary>
    /// 全面驗證所有過場動畫資料的語意合法性。
    /// </summary>
    public void Validate()
    {
        var ids = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        foreach (var seq in _sequences)
        {
            if (seq is null) throw new InvalidDataException("過場序列物件不能為 null。");
            if (string.IsNullOrWhiteSpace(seq.Name) || seq.Name.Length > 100)
                throw new InvalidDataException($"過場序列「{seq.Name}」名稱無效，必須介於 1–100 個字元。");
            if (!ids.Add(seq.Id))
                throw new InvalidDataException($"發現重複的過場序列識別碼：{seq.Id}。");

            // 航點檢驗
            if (seq.CameraWaypoints.Count is > 0 and < 2)
            {
                throw new InvalidDataException($"過場「{seq.Name}」的相機航跡至少需要 2 個路徑點才能構成平滑飛行。");
            }

            foreach (var wp in seq.CameraWaypoints)
            {
                if (wp.Position.X is < 0 or > 16384 || wp.Position.Z is < 0 or > 16384)
                    throw new InvalidDataException($"過場「{seq.Name}」中航點座標 ({wp.Position.X}, {wp.Position.Z}) 超出地圖邊界 (0..16384)。");
                if (wp.PitchDegrees is < 5 or > 89)
                    throw new InvalidDataException($"過場「{seq.Name}」中航點俯仰角 ({wp.PitchDegrees}°) 超出有效視野範圍 (5°..89°)。");
                if (wp.Zoom is < 1 or > 500)
                    throw new InvalidDataException($"過場「{seq.Name}」中航點焦距距離 ({wp.Zoom}) 異常 (有效範圍 1..500)。");
            }

            // 字幕檢驗
            foreach (var sub in seq.Subtitles)
            {
                if (string.IsNullOrWhiteSpace(sub.Text) || sub.Text.Length > 1000)
                    throw new InvalidDataException($"過場「{seq.Name}」中的字幕對白文字為空或長度超過 1000 字元。");
                if (sub.DurationSeconds <= 0f)
                    throw new InvalidDataException($"過場「{seq.Name}」中的字幕持續時間必須大於 0 秒。");
            }

            // 部隊調度檢驗
            foreach (var order in seq.UnitOrders)
            {
                if (order.TargetX is < 0 or > 16384 || order.TargetZ is < 0 or > 16384)
                    throw new InvalidDataException($"過場「{seq.Name}」中的部隊移動目標座標超出邊界。");
            }
        }
    }

    /// <summary>
    /// 匯出為 JSON 字串。
    /// </summary>
    public string ToJson() => JsonSerializer.Serialize(_sequences, JsonOptions);

    /// <summary>
    /// 從 JSON 字串解析並載入。
    /// </summary>
    public static CutsceneSequenceCatalog FromJson(string json)
    {
        if (string.IsNullOrWhiteSpace(json)) return new CutsceneSequenceCatalog();
        var list = JsonSerializer.Deserialize<List<CutsceneSequence>>(json, JsonOptions);
        return new CutsceneSequenceCatalog(list ?? new List<CutsceneSequence>());
    }

    private static CutsceneSequence Clone(CutsceneSequence source) => source with
    {
        CameraWaypoints = source.CameraWaypoints.ToList(),
        Subtitles = source.Subtitles.ToList(),
        UnitOrders = source.UnitOrders.ToList(),
        FxEvents = source.FxEvents.ToList()
    };

    private static IReadOnlyList<CutsceneSequence> DeepCloneList(IReadOnlyList<CutsceneSequence> items)
        => items.Select(Clone).ToArray();
}
