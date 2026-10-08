using AgainstRomeMapEditor.Modules.Localization;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Localization;

public sealed class MultiLanguageTextStudioTests
{
    [Fact]
    public void Studio_ParallelEditing_TracksDirtyAndCoverage()
    {
        var studio = new MultiLanguageTextStudio();
        studio.MasterLanguage = SupportedLanguage.TraditionalChinese;

        studio.UpdateText("briefing_titel_1", SupportedLanguage.TraditionalChinese, "日耳曼防線");
        studio.UpdateText("briefing_titel_1", SupportedLanguage.English, "Germanic Defense Line");
        studio.UpdateText("briefing_titel_1", SupportedLanguage.German, "Germanische Verteidigungslinie");

        var entry = studio.GetOrCreateEntry("briefing_titel_1");
        Assert.Equal("日耳曼防線", entry.GetText(SupportedLanguage.TraditionalChinese));
        Assert.Equal("Germanic Defense Line", entry.GetText(SupportedLanguage.English));
        Assert.Equal("Germanische Verteidigungslinie", entry.GetText(SupportedLanguage.German));

        Assert.True(entry.GetSlot(SupportedLanguage.TraditionalChinese).IsDirty);
        Assert.True(entry.GetSlot(SupportedLanguage.English).IsDirty);
    }

    [Fact]
    public void Studio_MasterLanguageUpdate_FlagsOtherLanguagesAsStaleOrMissing()
    {
        var studio = new MultiLanguageTextStudio();
        studio.MasterLanguage = SupportedLanguage.TraditionalChinese;

        // 初始狀態
        studio.UpdateText("briefing_titel_1", SupportedLanguage.TraditionalChinese, "舊標題");
        studio.UpdateText("briefing_titel_1", SupportedLanguage.English, "Old Title");
        studio.RefreshParityStatus();

        var entry = studio.GetOrCreateEntry("briefing_titel_1");
        Assert.Equal(SlotSyncStatus.Synchronized, entry.GetSlot(SupportedLanguage.English).Status);
        Assert.Equal(SlotSyncStatus.Missing, entry.GetSlot(SupportedLanguage.German).Status);

        // 修改基準語言
        studio.UpdateText("briefing_titel_1", SupportedLanguage.TraditionalChinese, "新標題");

        // 英文已有文字但尚未校對更新 -> Stale
        Assert.Equal(SlotSyncStatus.Stale, entry.GetSlot(SupportedLanguage.English).Status);
        // 德文原本為空 -> Missing
        Assert.Equal(SlotSyncStatus.Missing, entry.GetSlot(SupportedLanguage.German).Status);

        // 使用者更新英文 -> Synchronized
        studio.UpdateText("briefing_titel_1", SupportedLanguage.English, "New Title");
        Assert.Equal(SlotSyncStatus.Synchronized, entry.GetSlot(SupportedLanguage.English).Status);
    }

    [Fact]
    public void Studio_ExportAndImport_RoundTripsTables()
    {
        var studio = new MultiLanguageTextStudio();
        studio.UpdateText("briefing_titel_1", SupportedLanguage.TraditionalChinese, "反抗羅馬");
        studio.UpdateText("briefing_titel_1", SupportedLanguage.English, "Against Rome");
        studio.UpdateText("briefing_titel_1", SupportedLanguage.German, "Against Rome DE");

        var exportedTables = studio.ExportToTables();
        Assert.Equal("反抗羅馬", exportedTables[SupportedLanguage.TraditionalChinese].Title1);
        Assert.Equal("Against Rome", exportedTables[SupportedLanguage.English].Title1);
        Assert.Equal("Against Rome DE", exportedTables[SupportedLanguage.German].Title1);

        var newStudio = new MultiLanguageTextStudio();
        newStudio.LoadFromTables(exportedTables);

        Assert.Equal("反抗羅馬", newStudio.GetOrCreateEntry("briefing_titel_1").GetText(SupportedLanguage.TraditionalChinese));
        Assert.Equal("Against Rome", newStudio.GetOrCreateEntry("briefing_titel_1").GetText(SupportedLanguage.English));
    }

    [Fact]
    public void TranslationGlossary_TranslatesCoreTermsAccurately()
    {
        string zhText = "羅馬人進攻了日耳曼人的主屋與神殿！";
        string enTranslated = TranslationGlossary.Translate(zhText, SupportedLanguage.TraditionalChinese, SupportedLanguage.English);
        Assert.Contains("Romans", enTranslated);
        Assert.Contains("Germanics", enTranslated);
        Assert.Contains("Chieftain House", enTranslated);
        Assert.Contains("Temple", enTranslated);

        string deTranslated = TranslationGlossary.Translate(zhText, SupportedLanguage.TraditionalChinese, SupportedLanguage.German);
        Assert.Contains("Römer", deTranslated);
        Assert.Contains("Germanen", deTranslated);
        Assert.Contains("Haupthaus", deTranslated);
        Assert.Contains("Tempel", deTranslated);
    }
}
