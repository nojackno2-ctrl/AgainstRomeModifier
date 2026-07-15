using System.Text;
using System.Text.RegularExpressions;
using System.Globalization;

namespace AgainstRomeModifier.Maps;

public abstract class MapTextDocument
{
    private readonly byte[] _originalBytes;
    protected readonly Encoding Encoding;
    protected string Text;
    protected MapTextDocument(string path, Encoding encoding)
    {
        Path = path;
        Encoding = encoding;
        _originalBytes = File.ReadAllBytes(path);
        Text = encoding.GetString(GameLZSS.DecompressPfil(_originalBytes));
    }
    public string Path { get; }
    public void Save(FileRollbackScope? rollback = null)
    {
        byte[] bytes = Encoding.GetBytes(Text);
        if (IsPfil(_originalBytes)) bytes = GameLZSS.CompressPfil(bytes, _originalBytes);
        Core.Services.SafeFileWriter.WriteAllBytes(Path, bytes, rollback);
    }
    private static bool IsPfil(byte[] bytes) => bytes.Length >= 64 && bytes[0] == 'P' && bytes[1] == 'F' && bytes[2] == 'I' && bytes[3] == 'L';
    public static bool CanEncodeGameText(string value)
    {
        try { GameEncoding.GetByteCount(value); return true; }
        catch (EncoderFallbackException) { return false; }
    }
    protected static Encoding GameEncoding { get; } = CreateGameEncoding();
    private static Encoding CreateGameEncoding() { Encoding.RegisterProvider(CodePagesEncodingProvider.Instance); return Encoding.GetEncoding(1251, EncoderFallback.ExceptionFallback, DecoderFallback.ExceptionFallback); }
}

public sealed class BodenIniDocument : MapTextDocument
{
    private BodenIniDocument(string path) : base(path, GameEncoding) { }
    public static BodenIniDocument Load(string path) => new(path);
    public string? GetValue(string section) => FindValue(section)?.value;
    public void SetValue(string section, string value)
    {
        var found = FindValue(section) ?? throw new KeyNotFoundException("找不到 boden.ini 區段: " + section);
        Text = Text[..found.start] + value + Text[found.end..];
    }
    private (int start, int end, string value)? FindValue(string section)
    {
        var match = Regex.Match(Text, $@"(?im)^\s*\[{Regex.Escape(section)}\][^\r\n]*(?:\r?\n)(?<value>[^\r\n;]+)");
        if (!match.Success) return null;
        Group value = match.Groups["value"];
        int start = value.Index, end = value.Index + value.Length;
        while (start < end && char.IsWhiteSpace(Text[start])) start++;
        while (end > start && char.IsWhiteSpace(Text[end - 1])) end--;
        return (start, end, Text[start..end]);
    }
}

public sealed class PutTextDocument : MapTextDocument
{
    private PutTextDocument(string path) : base(path, GameEncoding) { }
    public static PutTextDocument Load(string path) => new(path);
    public string? GetValue(string key)
    {
        var match = Find(key); return match.Success ? match.Groups["value"].Value : null;
    }
    public void SetValue(string key, string value)
    {
        if (value.Contains('"') || value.Contains('\r') || value.Contains('\n')) throw new ArgumentException("地圖文字不可含引號或換行。", nameof(value));
        var match = Find(key);
        if (!match.Success) throw new KeyNotFoundException("找不到 .put 變數: " + key);
        Group existing = match.Groups["value"];
        Text = Text[..existing.Index] + value + Text[(existing.Index + existing.Length)..];
    }

    public string? GetCompositeValue(string key)
    {
        Match assignment = FindAssignment(key);
        if (!assignment.Success) return null;
        MatchCollection fragments = Regex.Matches(assignment.Groups["expression"].Value, @"""(?<value>(?:\\.|[^""\\])*)""");
        return fragments.Count == 0 ? null : string.Concat(fragments.Select(fragment => Unescape(fragment.Groups["value"].Value)));
    }

    public void SetCompositeValue(string key, string value)
    {
        if (value.Contains('\0')) throw new ArgumentException("地圖文字不可包含 NUL 字元。", nameof(value));
        Match assignment = FindAssignment(key);
        if (!assignment.Success) throw new KeyNotFoundException("找不到 .put 變數: " + key);
        Group expression = assignment.Groups["expression"];
        string replacement = "\"" + Escape(value) + "\"";
        Text = Text[..expression.Index] + replacement + Text[(expression.Index + expression.Length)..];
    }

    private Match Find(string key) => Regex.Match(Text, $@"(?im)^\s*var:\s*{Regex.Escape(key)}\s*=\s*""(?<value>[^""]*)""");
    private Match FindAssignment(string key) => Regex.Match(Text,
        $@"(?ims)^(?<prefix>[ \t]*var:\s*{Regex.Escape(key)}\s*=\s*)(?<expression>.*?)(?<suffix>;[ \t]*(?:\r?\n|$))");

    private static string Escape(string value) => value
        .Replace("\\", "\\\\", StringComparison.Ordinal)
        .Replace("\r", "\\r", StringComparison.Ordinal)
        .Replace("\n", "\\n", StringComparison.Ordinal)
        .Replace("\t", "\\t", StringComparison.Ordinal)
        .Replace("\"", "\\\"", StringComparison.Ordinal);

    private static string Unescape(string value)
    {
        var result = new StringBuilder(value.Length);
        for (int index = 0; index < value.Length; index++)
        {
            if (value[index] != '\\' || index + 1 >= value.Length) { result.Append(value[index]); continue; }
            char escaped = value[++index];
            result.Append(escaped switch { 'n' => '\n', 'r' => '\r', 't' => '\t', '"' => '"', '\\' => '\\', _ => "\\" + escaped });
        }
        return result.ToString();
    }
}

public sealed class SdlDocument : MapTextDocument
{
    private static readonly Regex SectionPattern = new(@"(?ms)^\[(?<name>[^\]\r\n]+)\][^\r\n]*(?:\r?\n|\z)(?<body>.*?)(?=^\[|\z)", RegexOptions.Compiled);
    private static readonly Regex ObjectSectionName = new(@"^object(?<index>\d+)$", RegexOptions.IgnoreCase | RegexOptions.Compiled);

    private SdlDocument(string path) : base(path, GameEncoding) { }
    public static SdlDocument Load(string path) => new(path);
    public IReadOnlyDictionary<string, string> Settlement => ReadSection("settlement");
    public IReadOnlyList<SdlObjectSection> Objects => Sections()
        .Where(section => ObjectSectionName.IsMatch(section.Name))
        .Select(section => new SdlObjectSection(
            int.Parse(ObjectSectionName.Match(section.Name).Groups["index"].Value, CultureInfo.InvariantCulture),
            ReadFields(section.Body)))
        .ToArray();

    public void RewriteMapPath(string oldMapId, string newMapId)
        => Text = Regex.Replace(Text, $@"(?im)^(\s*name\s*=\s*MAPS/){Regex.Escape(oldMapId)}(?=/)", "$1" + newMapId);

    public void SetSettlementValue(string key, string value) => SetSectionValue("settlement", key, value);

    public void TranslateSettlement(float deltaX, float deltaY, float deltaZ)
    {
        SdlVector3 reference = SdlVector3.Parse(RequiredValue(Settlement, "refpos"));
        SetSettlementValue("refpos", new SdlVector3(reference.X + deltaX, reference.Y + deltaY, reference.Z + deltaZ).ToString());
    }

    public void SetObjectValue(int index, string key, string value) => SetSectionValue(ObjectName(index), key, value);
    public void SetObjectTeam(int index, int team) => SetObjectValue(index, "team", team.ToString(CultureInfo.InvariantCulture));
    public void SetObjectPosition(int index, SdlVector3 position) => SetObjectValue(index, "pos", position.ToString("0.00"));
    public void SetObjectAngle(int index, float angle) => SetObjectValue(index, "angle", angle.ToString("0.00", CultureInfo.InvariantCulture));
    public void SetObjectDefinition(int index, int definition, string name)
    {
        SetObjectValue(index, "def", definition.ToString(CultureInfo.InvariantCulture));
        SetObjectValue(index, "namedef", name);
    }

    public int AddObject(IReadOnlyDictionary<string, string> fields)
    {
        ArgumentNullException.ThrowIfNull(fields);
        int index = Objects.Count;
        string newline = Text.Contains("\r\n", StringComparison.Ordinal) ? "\r\n" : "\n";
        var block = new StringBuilder();
        if (Text.Length > 0 && !Text.EndsWith("\n", StringComparison.Ordinal)) block.Append(newline);
        block.Append('[').Append(ObjectName(index)).Append(']').Append(newline);
        foreach ((string key, string value) in fields)
        {
            ValidateKeyValue(key, value);
            block.Append(key).Append('=').Append(value).Append(newline);
        }
        Text += block.ToString();
        return index;
    }

    public void RemoveObject(int index)
    {
        TextSection section = FindSection(ObjectName(index));
        Text = Text.Remove(section.Start, section.Length);
        RenumberObjects();
    }

    private IReadOnlyDictionary<string, string> ReadSection(string name) => ReadFields(FindSection(name).Body);

    private void SetSectionValue(string sectionName, string key, string value)
    {
        ValidateKeyValue(key, value);
        TextSection section = FindSection(sectionName);
        Match field = Regex.Match(section.Body, $@"(?im)^(?<prefix>[ \t]*{Regex.Escape(key)}[ \t]*=[ \t]*)(?<value>[^\r\n]*)");
        if (!field.Success) throw new KeyNotFoundException($"SDL 區段 [{sectionName}] 缺少欄位 {key}。");
        Group existing = field.Groups["value"];
        int start = section.BodyStart + existing.Index;
        Text = Text[..start] + value + Text[(start + existing.Length)..];
    }

    private void RenumberObjects()
    {
        int next = 0;
        Text = Regex.Replace(Text, @"(?im)^\[object\d+\]", _ => $"[object{next++:0000}]");
    }

    private TextSection FindSection(string name) => Sections().FirstOrDefault(section => SectionNameMatches(section.Name, name))
        ?? throw new KeyNotFoundException($"SDL 缺少 [{name}] 區段。");

    // 原版 SDL 的 [objectN] 補零寬度不一定是 4 位；讀取端只保留數值索引，因此以數值比對，
    // 避免寫回 [object0001] 卻對不上檔內 [object1]／[object00001] 而丟出 KeyNotFoundException。
    private static bool SectionNameMatches(string sectionName, string requested)
    {
        if (sectionName.Equals(requested, StringComparison.OrdinalIgnoreCase)) return true;
        Match actual = ObjectSectionName.Match(sectionName), wanted = ObjectSectionName.Match(requested);
        return actual.Success && wanted.Success
            && int.TryParse(actual.Groups["index"].Value, NumberStyles.None, CultureInfo.InvariantCulture, out int actualIndex)
            && int.TryParse(wanted.Groups["index"].Value, NumberStyles.None, CultureInfo.InvariantCulture, out int wantedIndex)
            && actualIndex == wantedIndex;
    }

    private IReadOnlyList<TextSection> Sections() => SectionPattern.Matches(Text).Select(match => new TextSection(
        match.Groups["name"].Value,
        match.Index,
        match.Length,
        match.Groups["body"].Index,
        match.Groups["body"].Value)).ToArray();

    private static IReadOnlyDictionary<string, string> ReadFields(string body)
    {
        var fields = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (Match match in Regex.Matches(body, @"(?im)^[ \t]*(?<key>[^=;\r\n]+?)[ \t]*=[ \t]*(?<value>[^\r\n]*)"))
            fields[match.Groups["key"].Value.Trim()] = match.Groups["value"].Value.Trim();
        return fields;
    }

    private static string RequiredValue(IReadOnlyDictionary<string, string> fields, string key)
        => fields.TryGetValue(key, out string? value) ? value : throw new KeyNotFoundException($"SDL 缺少欄位 {key}。");
    private static string ObjectName(int index) => index >= 0 ? $"object{index:0000}" : throw new ArgumentOutOfRangeException(nameof(index));
    private static void ValidateKeyValue(string key, string value)
    {
        if (string.IsNullOrWhiteSpace(key) || key.IndexOfAny(new[] { '=', '\r', '\n' }) >= 0) throw new ArgumentException("SDL 欄位名稱無效。", nameof(key));
        if (value.IndexOfAny(new[] { '\r', '\n' }) >= 0) throw new ArgumentException("SDL 欄位值不可包含換行。", nameof(value));
    }

    private sealed record TextSection(string Name, int Start, int Length, int BodyStart, string Body);
}

public sealed record SdlObjectSection(int Index, IReadOnlyDictionary<string, string> Fields)
{
    public string? GetValue(string key) => Fields.TryGetValue(key, out string? value) ? value : null;
}

public readonly record struct SdlVector3(float X, float Y, float Z)
{
    public static SdlVector3 Parse(string value)
    {
        string[] parts = value.Split(',');
        if (parts.Length != 3 ||
            !float.TryParse(parts[0].Trim(), NumberStyles.Float, CultureInfo.InvariantCulture, out float x) ||
            !float.TryParse(parts[1].Trim(), NumberStyles.Float, CultureInfo.InvariantCulture, out float y) ||
            !float.TryParse(parts[2].Trim(), NumberStyles.Float, CultureInfo.InvariantCulture, out float z))
            throw new FormatException($"無效的 SDL 三維座標：{value}");
        return new SdlVector3(x, y, z);
    }

    public override string ToString() => ToString("0.##");
    public string ToString(string format) => string.Join(",", X.ToString(format, CultureInfo.InvariantCulture), Y.ToString(format, CultureInfo.InvariantCulture), Z.ToString(format, CultureInfo.InvariantCulture));
}

public sealed class BodenTexturesDocument : MapTextDocument
{
    private BodenTexturesDocument(string path) : base(path, GameEncoding) { }
    public static BodenTexturesDocument Load(string path) => new(path);
    public int Dimension
    {
        get
        {
            Match match = Regex.Match(Text, @"(?im)^\s*\[Dimension\]\s*\r?\n\s*(\d+)");
            return match.Success && int.TryParse(match.Groups[1].Value, out int dimension) && dimension > 0
                ? dimension : throw new InvalidDataException("boden.txt 缺少有效的 [Dimension]。");
        }
    }

    public IReadOnlyList<string> Textures => Entries().Select(x => x.Value).ToArray();
    public string GetTexture(int x, int y) => EntryAt(x, y).Value;
    public void SetTexture(int x, int y, string texture)
    {
        ValidateTextureName(texture);
        TextureEntry entry = EntryAt(x, y);
        Text = Text[..entry.Start] + texture.Trim() + Text[entry.End..];
    }

    /// <summary>一次寫入整張 64×64 材質表；比逐格 SetTexture 快一個數量級（單次解析、單次重組）。</summary>
    public void SetTextures(IReadOnlyList<string> textures)
    {
        int dimension = Dimension;
        if (dimension != 64) throw new InvalidOperationException("目前只支援已驗證的 64×64 boden.txt 材質格。");
        TextureEntry[] entries = Entries();
        if (entries.Length != dimension * dimension) throw new InvalidDataException($"boden.txt 應有 {dimension * dimension} 個材質格，實際為 {entries.Length}。");
        if (textures.Count != entries.Length) throw new ArgumentException($"材質數量應為 {entries.Length}，實際為 {textures.Count}。", nameof(textures));
        foreach (string texture in textures) ValidateTextureName(texture);
        var builder = new StringBuilder(Text.Length + 64);
        int position = 0;
        for (int index = 0; index < entries.Length; index++)
        {
            builder.Append(Text, position, entries[index].Start - position);
            builder.Append(textures[index].Trim());
            position = entries[index].End;
        }
        builder.Append(Text, position, Text.Length - position);
        Text = builder.ToString();
    }

    private static void ValidateTextureName(string texture)
    {
        if (string.IsNullOrWhiteSpace(texture) || texture.IndexOfAny(new[] { '\r', '\n' }) >= 0) throw new ArgumentException("材質名稱不可為空或包含換行。", nameof(texture));
    }

    private TextureEntry EntryAt(int x, int y)
    {
        int dimension = Dimension;
        if (x is < 0 or >= 64 || y is < 0 or >= 64 || dimension != 64) throw new InvalidOperationException("目前只支援已驗證的 64×64 boden.txt 材質格。");
        TextureEntry[] entries = Entries();
        int index = y * dimension + x;
        if (entries.Length != dimension * dimension) throw new InvalidDataException($"boden.txt 應有 {dimension * dimension} 個材質格，實際為 {entries.Length}。");
        return entries[index];
    }

    private TextureEntry[] Entries()
    {
        Match section = Regex.Match(Text, @"(?im)^\s*\[Texturen\]\s*\r?\n");
        if (!section.Success) throw new InvalidDataException("boden.txt 缺少 [Texturen]。");
        var entries = new List<TextureEntry>(); int position = section.Index + section.Length;
        while (position < Text.Length)
        {
            int lineEnd = Text.IndexOf('\n', position); if (lineEnd < 0) lineEnd = Text.Length;
            int valueStart = position; int valueEnd = lineEnd;
            if (valueEnd > valueStart && Text[valueEnd - 1] == '\r') valueEnd--;
            while (valueStart < valueEnd && char.IsWhiteSpace(Text[valueStart])) valueStart++;
            while (valueEnd > valueStart && char.IsWhiteSpace(Text[valueEnd - 1])) valueEnd--;
            if (valueStart < valueEnd) entries.Add(new TextureEntry(valueStart, valueEnd, Text[valueStart..valueEnd]));
            position = lineEnd == Text.Length ? lineEnd : lineEnd + 1;
        }
        return entries.ToArray();
    }

    private sealed record TextureEntry(int Start, int End, string Value);
}
