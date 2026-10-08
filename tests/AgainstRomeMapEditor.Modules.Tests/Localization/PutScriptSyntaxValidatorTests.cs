using AgainstRomeMapEditor.Modules.Localization;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Localization;

public sealed class PutScriptSyntaxValidatorTests
{
    [Fact]
    public void Validator_Detects_LiteralByteOverflow()
    {
        string oversizedLiteral = new string('x', 105);
        string script = $"var:briefing_titel_1 =\"{oversizedLiteral}\";\r\n";

        var report = PutScriptSyntaxValidator.ValidateScriptText(script, "WINDOWS-1252");
        Assert.False(report.IsValid);
        Assert.Contains(report.Issues, i => i.Severity == PutIssueSeverity.Error && i.Message.Contains("超過遊戲緩衝區上限"));
    }

    [Fact]
    public void Validator_Detects_MissingSemicolon()
    {
        string script = "var:briefing_titel_1 =\"Title Without Semicolon\"\r\n";

        var report = PutScriptSyntaxValidator.ValidateScriptText(script, "WINDOWS-1252");
        Assert.False(report.IsValid);
        Assert.Contains(report.Issues, i => i.Severity == PutIssueSeverity.Error && i.Message.Contains("缺少結尾分號"));
    }

    [Fact]
    public void Validator_Detects_UnclosedQuotation()
    {
        string script = "var:briefing_titel_1 =\"Unclosed String;\r\nvar:other = 1;\r\n";

        var report = PutScriptSyntaxValidator.ValidateScriptText(script, "WINDOWS-1252");
        Assert.False(report.IsValid);
        Assert.Contains(report.Issues, i => i.Severity == PutIssueSeverity.Error && i.Message.Contains("未正確閉合雙引號"));
    }

    [Fact]
    public void Validator_Windows1252_RejectsChineseCharacters()
    {
        string script = "var:briefing_titel_1 =\"羅馬軍團的進攻\";\r\n";

        var report = PutScriptSyntaxValidator.ValidateScriptText(script, "WINDOWS-1252");
        Assert.False(report.IsValid);
        Assert.Contains(report.Issues, i => i.Severity == PutIssueSeverity.Error && i.Message.Contains("無法由目標編碼"));
    }

    [Fact]
    public void Validator_Big5_AcceptsTraditionalChinese()
    {
        string script = "var:briefing_titel_1 =\"反抗羅馬\";\r\n";

        var report = PutScriptSyntaxValidator.ValidateScriptText(script, "BIG5");
        Assert.True(report.IsValid);
        Assert.Empty(report.Issues.Where(i => i.Severity == PutIssueSeverity.Error));
    }

    [Fact]
    public void Validator_Windows1252_AcceptsGermanUmlauts()
    {
        string script = "var:briefing_titel_1 =\"Kämpfe für die Götter, schöne Straße!\";\r\n";

        var report = PutScriptSyntaxValidator.ValidateScriptText(script, "WINDOWS-1252");
        Assert.True(report.IsValid);
    }

    [Fact]
    public void Validator_Detects_NewlineInSingleLineVariable()
    {
        var table = BriefingTextTable.CreateEmpty();
        table.Title1 = "First Line\nSecond Line"; // 單行標題違規換行

        var report = PutScriptSyntaxValidator.ValidateTable(table, "UTF-8");
        Assert.False(report.IsValid);
        Assert.Contains(report.Issues, i => i.Severity == PutIssueSeverity.Error && i.Message.Contains("不得包含換行字元"));
    }

    [Fact]
    public void Validator_Detects_DanglingBackslash()
    {
        string script = "var:briefing_titel_1 =\"Path C:\\\";\r\n";

        var report = PutScriptSyntaxValidator.ValidateScriptText(script, "WINDOWS-1252");
        Assert.False(report.IsValid);
    }
}
