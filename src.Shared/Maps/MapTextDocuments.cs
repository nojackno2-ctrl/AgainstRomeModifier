using System.Text;
using System.Text.RegularExpressions;

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
    private Match Find(string key) => Regex.Match(Text, $@"(?im)^\s*var:\s*{Regex.Escape(key)}\s*=\s*""(?<value>[^""]*)""");
}

public sealed class SdlDocument : MapTextDocument
{
    private SdlDocument(string path) : base(path, GameEncoding) { }
    public static SdlDocument Load(string path) => new(path);
    public void RewriteMapPath(string oldMapId, string newMapId)
        => Text = Regex.Replace(Text, $@"(?im)^(\s*name\s*=\s*MAPS/){Regex.Escape(oldMapId)}(?=/)", "$1" + newMapId);
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
        if (string.IsNullOrWhiteSpace(texture) || texture.IndexOfAny(new[] { '\r', '\n' }) >= 0) throw new ArgumentException("材質名稱不可為空或包含換行。", nameof(texture));
        TextureEntry entry = EntryAt(x, y);
        Text = Text[..entry.Start] + texture.Trim() + Text[entry.End..];
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
