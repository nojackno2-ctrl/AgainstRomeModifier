using AgainstRomeMapEditor.Modules.Localization;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Localization;

public sealed class BriefingTextTableCatalogTests
{
    [Fact]
    public void Catalog_ContainsAllStandardBriefingKeys()
    {
        var descriptors = BriefingTextTableCatalog.GetAllStandardDescriptors();
        Assert.True(descriptors.Count >= 18); // 2 headers + 1 body + 4 video/audio + 4 debriefing + 8 teams = 19

        Assert.True(BriefingTextTableCatalog.TryGetDescriptor("briefing_titel_1", out var titleDesc));
        Assert.NotNull(titleDesc);
        Assert.Equal(BriefingVariableCategory.MapHeader, titleDesc.Category);
        Assert.True(titleDesc.IsRequired);
        Assert.Equal(BriefingVariableType.SingleLineString, titleDesc.ValueType);

        Assert.True(BriefingTextTableCatalog.TryGetDescriptor("briefing_text", out var bodyDesc));
        Assert.NotNull(bodyDesc);
        Assert.Equal(BriefingVariableCategory.BriefingBody, bodyDesc.Category);
        Assert.Equal(BriefingVariableType.CompositeString, bodyDesc.ValueType);

        Assert.True(BriefingTextTableCatalog.TryGetDescriptor("debriefing_text_win", out var winDesc));
        Assert.NotNull(winDesc);
        Assert.Equal(BriefingVariableCategory.Debriefing, winDesc.Category);

        Assert.True(BriefingTextTableCatalog.TryGetDescriptor("briefing_video_nummmer", out var vidDesc));
        Assert.NotNull(vidDesc);
        Assert.Equal(BriefingVariableType.Integer, vidDesc.ValueType);
    }

    [Theory]
    [InlineData(0, "briefing_text_teamname0")]
    [InlineData(1, "briefing_text_teamname1")]
    [InlineData(7, "briefing_text_teamname7")]
    public void TeamKey_Helpers_RoundTrip(int index, string expectedKey)
    {
        string key = BriefingTextTableCatalog.GetTeamKey(index);
        Assert.Equal(expectedKey, key);

        Assert.True(BriefingTextTableCatalog.TryGetTeamIndex(key, out int parsedIndex));
        Assert.Equal(index, parsedIndex);
    }

    [Theory]
    [InlineData("ENDL_000", 0, "endl_000_text_00")]
    [InlineData("ENDL_005", 2, "endl_005_text_02")]
    public void ObjectiveKey_Helpers_RoundTrip(string slot, int index, string expectedKey)
    {
        string key = BriefingTextTableCatalog.GetObjectiveKey(slot, index);
        Assert.Equal(expectedKey, key);

        Assert.True(BriefingTextTableCatalog.TryGetObjectiveIndex(key, out string parsedSlot, out int parsedIndex));
        Assert.Equal("000", parsedSlot.PadLeft(3, '0'));
        Assert.Equal(index, parsedIndex);
    }

    [Fact]
    public void PutScriptDocument_Parse_PreservesCommentsAndFormatting()
    {
        string script =
            "// Briefing-Texte fuer Szenario ENDL_000 US\r\n" +
            "; Sekundaerer Kommentar\r\n" +
            "\r\n" +
            "var:briefing_titel_1 =\"Battle for Rome\";\r\n" +
            "var:briefing_video_nummmer = 3;\r\n" +
            "var:briefing_text =\r\n" +
            "    \"Line 1\\n\"\r\n" +
            "    \"Line 2\";\r\n";

        var doc = PutScriptDocument.Parse(script);
        Assert.Equal(6, doc.Nodes.Count);

        Assert.Equal("Battle for Rome", doc.GetStringValue("briefing_titel_1"));
        Assert.Equal(3, doc.GetIntegerValue("briefing_video_nummmer"));
        Assert.Equal("Line 1\nLine 2", doc.GetStringValue("briefing_text"));

        string serialized = doc.ToScriptText();
        Assert.Contains("// Briefing-Texte", serialized);
        Assert.Contains("; Sekundaerer Kommentar", serialized);
        Assert.Contains("var:briefing_video_nummmer = 3;", serialized);
    }

    [Fact]
    public void BriefingTextTable_GetAndSetProperties_BehaveCorrectly()
    {
        var table = BriefingTextTable.CreateEmpty();
        table.Title1 = "Custom Map";
        table.Title2 = "Episode I";
        table.BriefingText = "Survive the assault.\nBuild towers.";
        table.SampleName = "CUSTOM_01.wav";
        table.VideoNumber = 5;
        table.SetTeamName(0, "Germans");
        table.SetTeamName(1, "Romans");
        table.DebriefingWin = "Victory is yours!";

        Assert.Equal("Custom Map", table.Title1);
        Assert.Equal("Episode I", table.Title2);
        Assert.Equal("Survive the assault.\nBuild towers.", table.BriefingText);
        Assert.Equal("CUSTOM_01.wav", table.SampleName);
        Assert.Equal(5, table.VideoNumber);
        Assert.Equal("Germans", table.GetTeamName(0));
        Assert.Equal("Romans", table.GetTeamName(1));
        Assert.Equal("Victory is yours!", table.DebriefingWin);

        string script = table.ToScriptText();
        Assert.Contains("var:briefing_titel_1 =\"Custom Map\";", script);
        Assert.Contains("var:briefing_text_teamname0 =\"Germans\";", script);
        Assert.Contains("var:briefing_video_nummmer = 5;", script);
    }

    [Fact]
    public void PutScriptFormatting_SplitEscapedIntoSafeChunks_DoesNotExceedMaxBytes()
    {
        string longText = new string('A', 250); // 250 位元組
        var chunks = PutScriptFormatting.SplitEscapedIntoSafeChunks(longText, System.Text.Encoding.ASCII, 100);

        Assert.True(chunks.Count >= 3);
        foreach (string chunk in chunks)
        {
            Assert.True(System.Text.Encoding.ASCII.GetByteCount(chunk) <= 100);
        }
        Assert.Equal(longText, string.Concat(chunks));
    }
}
