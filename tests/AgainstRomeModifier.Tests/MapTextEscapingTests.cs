using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class MapTextEscapingTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "ArmTextEscapes_" + Guid.NewGuid().ToString("N"));

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Single_literal_escapes_round_trip_and_preserve_other_lines(bool pfil)
    {
        string source = "; keep comment\r\nvar:briefing_titel_1 =\"Original\";\r\nvar:other =\"leave\\nme\";\r\n";
        string path = Create(source, pfil);
        var doc = PutTextDocument.Load(path);
        string value = "План \"Alpha\" C:\\new\\route\\ \t end";
        doc.SetValue("briefing_titel_1", value);
        doc.Save();
        var loaded = PutTextDocument.Load(path);
        Assert.Equal(value, loaded.GetValue("briefing_titel_1"));
        Assert.Equal(value, loaded.GetCompositeValue("briefing_titel_1"));
        string text = SyntheticFixture.GameEncoding.GetString(GameLZSS.DecompressPfil(File.ReadAllBytes(path)));
        Assert.StartsWith("; keep comment\r\n", text);
        Assert.EndsWith("var:other =\"leave\\nme\";\r\n", text);
        Assert.Contains("C:\\\\new\\\\route\\\\", text);
        Assert.Contains("\\\"Alpha\\\"", text);
        byte[] saved = File.ReadAllBytes(path);
        loaded.SetValue("briefing_titel_1", value);
        loaded.Save();
        Assert.Equal(saved, File.ReadAllBytes(path));
    }

    [Fact]
    public void Existing_escaped_single_literal_reads_as_display_text_and_noop_preserves_bytes()
    {
        string source = "var:briefing_titel_1 =\"A \\\"title\\\" C:\\\\new\\\\ \\t end\";\r\n";
        string path = Create(source, true);
        byte[] initial = File.ReadAllBytes(path);
        var doc = PutTextDocument.Load(path);
        Assert.Equal("A \"title\" C:\\new\\ \t end", doc.GetValue("briefing_titel_1"));
        doc.SetValue("briefing_titel_1", doc.GetValue("briefing_titel_1")!);
        doc.Save();
        Assert.Equal(initial, File.ReadAllBytes(path));
    }

    [Theory]
    [InlineData("bad\0text")]
    [InlineData("bad\rtext")]
    [InlineData("bad\ntext")]
    [InlineData("中文")]
    public void Invalid_single_literal_does_not_change_document_or_disk(string value)
    {
        string path = Create("var:briefing_titel_1 =\"Original\";\r\n", false);
        byte[] initial = File.ReadAllBytes(path);
        var doc = PutTextDocument.Load(path);
        Assert.Throws<ArgumentException>(() => doc.SetValue("briefing_titel_1", value));
        Assert.Equal("Original", doc.GetValue("briefing_titel_1"));
        doc.Save();
        Assert.Equal(initial, File.ReadAllBytes(path));
    }

    [Fact]
    public void Single_literal_limit_counts_escaped_bytes_before_mutation()
    {
        string path = Create("var:briefing_titel_1 =\"Original\";\r\n", false);
        var doc = PutTextDocument.Load(path);
        Assert.Throws<ArgumentException>(() => doc.SetValue("briefing_titel_1", new string('\\', 51)));
        Assert.Equal("Original", doc.GetValue("briefing_titel_1"));
        doc.SetValue("briefing_titel_1", new string('\\', 50));
        doc.Save();
        Assert.Equal(new string('\\', 50), PutTextDocument.Load(path).GetCompositeValue("briefing_titel_1"));
    }

    [Fact]
    public void Unencodable_briefing_is_rejected_before_document_mutation_with_encoding_guidance()
    {
        string path = Create("var:briefing_text =\"Original\";\r\n", false);
        var doc = PutTextDocument.Load(path);
        var error = Assert.Throws<ArgumentException>(() => doc.SetCompositeValue("briefing_text", "中文"));
        Assert.Contains("CP1251", error.Message);
        Assert.Equal("Original", doc.GetCompositeValue("briefing_text"));
    }

    private string Create(string text, bool pfil)
    {
        Directory.CreateDirectory(_root);
        string path = Path.Combine(_root, "briefing.put");
        File.WriteAllBytes(path, pfil ? SyntheticFixture.Pfil(text) : SyntheticFixture.GameEncoding.GetBytes(text));
        return path;
    }

    public void Dispose() { if (Directory.Exists(_root)) Directory.Delete(_root, true); }
}
