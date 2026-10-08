using System.Text.RegularExpressions;

namespace AgainstRomeMapEditor.Modules.Localization;

/// <summary>
/// 簡報變數類別。
/// </summary>
public enum BriefingVariableCategory
{
    MapHeader,
    BriefingBody,
    AudioVideo,
    TeamName,
    Debriefing,
    Objective,
    Custom
}

/// <summary>
/// 變數數值型別。
/// </summary>
public enum BriefingVariableType
{
    SingleLineString,
    CompositeString,
    Integer
}

/// <summary>
/// 簡報與任務文字變數描述規格。
/// </summary>
public sealed record BriefingVariableDescriptor
{
    public string Key { get; init; }
    public string NameZH { get; init; }
    public string NameEN { get; init; }
    public string NameDE { get; init; }
    public string DescriptionZH { get; init; }
    public string DescriptionEN { get; init; }
    public string DescriptionDE { get; init; }
    public BriefingVariableCategory Category { get; init; }
    public BriefingVariableType ValueType { get; init; }
    public bool IsRequired { get; init; }
    public string DefaultValue { get; init; } = "";
    public int MaxLiteralBytes { get; init; } = 100;

    public BriefingVariableDescriptor(
        string key,
        string nameZH,
        string nameEN,
        string nameDE,
        BriefingVariableCategory category,
        BriefingVariableType valueType,
        string descriptionZH = "",
        string descriptionEN = "",
        string descriptionDE = "",
        bool isRequired = false,
        string defaultValue = "",
        int maxLiteralBytes = 100)
    {
        Key = key ?? throw new ArgumentNullException(nameof(key));
        NameZH = nameZH;
        NameEN = nameEN;
        NameDE = nameDE;
        Category = category;
        ValueType = valueType;
        DescriptionZH = descriptionZH;
        DescriptionEN = descriptionEN;
        DescriptionDE = descriptionDE;
        IsRequired = isRequired;
        DefaultValue = defaultValue;
        MaxLiteralBytes = maxLiteralBytes;
    }

    public string GetDisplayName(string langCode) => langCode.ToUpperInvariant() switch
    {
        "DE" or "GR" => NameDE,
        "EN" or "US" => NameEN,
        _ => NameZH
    };
}

/// <summary>
/// 《反抗羅馬》briefing.put 與任務文字結構定義型錄。
/// 支援原生已知欄位解析、多隊伍名稱、結算文字、影音設定以及自訂擴充變數。
/// </summary>
public static class BriefingTextTableCatalog
{
    public const int MaxTeamCount = 8;

    private static readonly Dictionary<string, BriefingVariableDescriptor> KnownDescriptors = new(StringComparer.OrdinalIgnoreCase);

    static BriefingTextTableCatalog()
    {
        Register(new BriefingVariableDescriptor(
            "briefing_titel_1",
            "地圖標題", "Map Title", "Kartentitel",
            BriefingVariableCategory.MapHeader,
            BriefingVariableType.SingleLineString,
            "遊戲選單與地圖清單中顯示的主要關卡名稱",
            "Primary scenario name displayed in map selection menu",
            "Hauptszenarioname im Kartenauswahlmenü",
            isRequired: true,
            defaultValue: "New Scenario"));

        Register(new BriefingVariableDescriptor(
            "briefing_titel_2",
            "地圖副標題", "Map Subtitle", "Kartenuntertitel",
            BriefingVariableCategory.MapHeader,
            BriefingVariableType.SingleLineString,
            "關卡副標題、戰役章節或難度提示",
            "Scenario subtitle, campaign chapter or difficulty hint",
            "Szenariountertitel oder Schwierigkeitshinweis",
            defaultValue: ""));

        Register(new BriefingVariableDescriptor(
            "briefing_text",
            "任務簡報本文", "Briefing Text", "Briefing-Text",
            BriefingVariableCategory.BriefingBody,
            BriefingVariableType.CompositeString,
            "開局任務背景、情勢交代與總體說明，支援多行與自動安全切段",
            "Mission background story and briefing, supports multiline with automatic segmenting",
            "Missionshintergrund und Briefing-Haupttext, mehrzeilig",
            defaultValue: ""));

        Register(new BriefingVariableDescriptor(
            "briefing_text_sample_name",
            "簡報語音檔名", "Voice Sample File", "Sprachausgabe-Datei",
            BriefingVariableCategory.AudioVideo,
            BriefingVariableType.SingleLineString,
            "進入簡報畫面時播放的語音檔案名稱（.wav）",
            "Audio file played during briefing screen (.wav)",
            "Audiodatei für Briefing-Sprachausgabe (.wav)",
            defaultValue: ""));

        Register(new BriefingVariableDescriptor(
            "briefing_video_nummmer",
            "影片編號", "Video Number", "Videonummer",
            BriefingVariableCategory.AudioVideo,
            BriefingVariableType.Integer,
            "關卡播放的影片資源索引",
            "Scenario video resource index",
            "Videoressourcen-Index des Szenarios",
            defaultValue: "0"));

        Register(new BriefingVariableDescriptor(
            "briefing_introvideo_nummmer",
            "開場影片編號", "Intro Video Number", "Intro-Videonummer",
            BriefingVariableCategory.AudioVideo,
            BriefingVariableType.Integer,
            "進入地圖前播放的開場過場動畫索引",
            "Opening cinematic video index played before entering map",
            "Eröffnungssequenz-Videoindex vor Betreten der Karte",
            defaultValue: "0"));

        Register(new BriefingVariableDescriptor(
            "briefing_outrovideo_nummmer",
            "結尾影片編號", "Outro Video Number", "Outro-Videonummer",
            BriefingVariableCategory.AudioVideo,
            BriefingVariableType.Integer,
            "通關後播放的結束動畫索引",
            "Ending cinematic video index played after mission complete",
            "Abschluss-Videoindex nach Missionsende",
            defaultValue: "0"));

        Register(new BriefingVariableDescriptor(
            "debriefing_text_win",
            "勝利結算文字", "Victory Debriefing", "Sieg-Nachbesprechung",
            BriefingVariableCategory.Debriefing,
            BriefingVariableType.CompositeString,
            "達成勝利條件時顯示的結算對話文字 (GLOBAL_MISSION_RESULT = 1)",
            "Debriefing text displayed upon victory (GLOBAL_MISSION_RESULT = 1)",
            "Nachbesprechungstext bei Missionssieg (GLOBAL_MISSION_RESULT = 1)",
            defaultValue: ""));

        Register(new BriefingVariableDescriptor(
            "debriefing_text_loss",
            "失敗結算文字", "Defeat Debriefing", "Niederlage-Nachbesprechung",
            BriefingVariableCategory.Debriefing,
            BriefingVariableType.CompositeString,
            "任務失敗時顯示的結算對話文字 (GLOBAL_MISSION_RESULT = 0)",
            "Debriefing text displayed upon defeat (GLOBAL_MISSION_RESULT = 0)",
            "Nachbesprechungstext bei Niederlage (GLOBAL_MISSION_RESULT = 0)",
            defaultValue: ""));

        Register(new BriefingVariableDescriptor(
            "debriefing_text_equal",
            "平手結算文字", "Draw Debriefing", "Unentschieden-Nachbesprechung",
            BriefingVariableCategory.Debriefing,
            BriefingVariableType.CompositeString,
            "平局時顯示的結算對話文字 (GLOBAL_MISSION_RESULT = 2)",
            "Debriefing text displayed upon draw (GLOBAL_MISSION_RESULT = 2)",
            "Nachbesprechungstext bei Unentschieden (GLOBAL_MISSION_RESULT = 2)",
            defaultValue: ""));

        Register(new BriefingVariableDescriptor(
            "debriefing_text_misc",
            "其他結算文字", "Misc Debriefing", "Sonstige Nachbesprechung",
            BriefingVariableCategory.Debriefing,
            BriefingVariableType.CompositeString,
            "其他自訂結果時顯示的結算文字 (GLOBAL_MISSION_RESULT = 3)",
            "Debriefing text displayed for special results (GLOBAL_MISSION_RESULT = 3)",
            "Nachbesprechungstext für spezielle Ergebnisse (GLOBAL_MISSION_RESULT = 3)",
            defaultValue: ""));

        for (int i = 0; i < MaxTeamCount; i++)
        {
            Register(new BriefingVariableDescriptor(
                GetTeamKey(i),
                $"隊伍 {i} 名稱", $"Team {i} Name", $"Team {i} Name",
                BriefingVariableCategory.TeamName,
                BriefingVariableType.SingleLineString,
                $"隊伍 {i} 於遊戲中顯示的自訂陣營名稱",
                $"Custom faction name for team {i} displayed in game",
                $"Benutzerdefinierter Fraktionsname für Team {i}",
                defaultValue: $"Team {i}"));
        }
    }

    private static void Register(BriefingVariableDescriptor descriptor)
    {
        KnownDescriptors[descriptor.Key] = descriptor;
    }

    public static string GetTeamKey(int teamIndex) => $"briefing_text_teamname{teamIndex}";

    public static bool TryGetTeamIndex(string key, out int index)
    {
        index = -1;
        var match = Regex.Match(key, @"^briefing_text_teamname(\d+)$", RegexOptions.IgnoreCase);
        if (match.Success && int.TryParse(match.Groups[1].Value, out int idx))
        {
            index = idx;
            return true;
        }
        return false;
    }

    public static string GetObjectiveKey(string slot, int index)
    {
        string safeSlot = string.IsNullOrWhiteSpace(slot) ? "000" : slot.ToLowerInvariant().Replace("endl_", "");
        return $"endl_{safeSlot}_text_{index:00}";
    }

    public static bool TryGetObjectiveIndex(string key, out string slot, out int index)
    {
        slot = "";
        index = -1;
        var match = Regex.Match(key, @"^endl_([a-zA-Z0-9]+)_text_(\d+)$", RegexOptions.IgnoreCase);
        if (match.Success && int.TryParse(match.Groups[2].Value, out int idx))
        {
            slot = match.Groups[1].Value;
            index = idx;
            return true;
        }
        return false;
    }

    public static bool TryGetDescriptor(string key, out BriefingVariableDescriptor? descriptor)
    {
        return KnownDescriptors.TryGetValue(key, out descriptor);
    }

    public static BriefingVariableDescriptor GetDescriptor(string key)
    {
        if (KnownDescriptors.TryGetValue(key, out var desc)) return desc;

        if (TryGetTeamIndex(key, out int teamIdx))
        {
            return new BriefingVariableDescriptor(
                key,
                $"隊伍 {teamIdx} 名稱", $"Team {teamIdx} Name", $"Team {teamIdx} Name",
                BriefingVariableCategory.TeamName,
                BriefingVariableType.SingleLineString,
                defaultValue: $"Team {teamIdx}");
        }

        if (TryGetObjectiveIndex(key, out string slot, out int objIdx))
        {
            return new BriefingVariableDescriptor(
                key,
                $"任務目標 {objIdx}", $"Objective {objIdx}", $"Missionsziel {objIdx}",
                BriefingVariableCategory.Objective,
                BriefingVariableType.CompositeString,
                $"槽位 {slot} 任務目標第 {objIdx} 項敘述",
                $"Objective description {objIdx} for slot {slot}",
                $"Zielbeschreibung {objIdx} für Slot {slot}");
        }

        return new BriefingVariableDescriptor(
            key,
            $"自訂變數 ({key})", $"Custom ({key})", $"Benutzerdefiniert ({key})",
            BriefingVariableCategory.Custom,
            BriefingVariableType.CompositeString,
            "使用者自訂或腳本專用變數",
            "User defined or script specific variable",
            "Benutzerdefinierte oder skriptspezifische Variable");
    }

    public static IReadOnlyCollection<BriefingVariableDescriptor> GetAllStandardDescriptors()
    {
        return KnownDescriptors.Values.ToList();
    }
}

/// <summary>
/// 封裝 briefing.put 或 text.put 的結構化存取實體。
/// </summary>
public sealed class BriefingTextTable
{
    public PutScriptDocument Document { get; }

    public BriefingTextTable(PutScriptDocument document)
    {
        Document = document ?? throw new ArgumentNullException(nameof(document));
    }

    public static BriefingTextTable CreateEmpty() => new(new PutScriptDocument());

    public static BriefingTextTable Parse(string script) => new(PutScriptDocument.Parse(script));

    public string Title1
    {
        get => Document.GetStringValue("briefing_titel_1") ?? "";
        set => Document.SetVariable("briefing_titel_1", value, isComposite: false);
    }

    public string Title2
    {
        get => Document.GetStringValue("briefing_titel_2") ?? "";
        set => Document.SetVariable("briefing_titel_2", value, isComposite: false);
    }

    public string BriefingText
    {
        get => Document.GetStringValue("briefing_text") ?? "";
        set => Document.SetVariable("briefing_text", value, isComposite: true);
    }

    public string SampleName
    {
        get => Document.GetStringValue("briefing_text_sample_name") ?? "";
        set => Document.SetVariable("briefing_text_sample_name", value, isComposite: false);
    }

    public int? VideoNumber
    {
        get => Document.GetIntegerValue("briefing_video_nummmer");
        set { if (value.HasValue) Document.SetVariable("briefing_video_nummmer", value.Value); else Document.RemoveVariable("briefing_video_nummmer"); }
    }

    public int? IntroVideoNumber
    {
        get => Document.GetIntegerValue("briefing_introvideo_nummmer");
        set { if (value.HasValue) Document.SetVariable("briefing_introvideo_nummmer", value.Value); else Document.RemoveVariable("briefing_introvideo_nummmer"); }
    }

    public int? OutroVideoNumber
    {
        get => Document.GetIntegerValue("briefing_outrovideo_nummmer");
        set { if (value.HasValue) Document.SetVariable("briefing_outrovideo_nummmer", value.Value); else Document.RemoveVariable("briefing_outrovideo_nummmer"); }
    }

    public string DebriefingWin
    {
        get => Document.GetStringValue("debriefing_text_win") ?? "";
        set => Document.SetVariable("debriefing_text_win", value, isComposite: true);
    }

    public string DebriefingLoss
    {
        get => Document.GetStringValue("debriefing_text_loss") ?? "";
        set => Document.SetVariable("debriefing_text_loss", value, isComposite: true);
    }

    public string DebriefingEqual
    {
        get => Document.GetStringValue("debriefing_text_equal") ?? "";
        set => Document.SetVariable("debriefing_text_equal", value, isComposite: true);
    }

    public string DebriefingMisc
    {
        get => Document.GetStringValue("debriefing_text_misc") ?? "";
        set => Document.SetVariable("debriefing_text_misc", value, isComposite: true);
    }

    public string GetTeamName(int teamIndex)
    {
        return Document.GetStringValue(BriefingTextTableCatalog.GetTeamKey(teamIndex)) ?? "";
    }

    public void SetTeamName(int teamIndex, string name)
    {
        Document.SetVariable(BriefingTextTableCatalog.GetTeamKey(teamIndex), name, isComposite: false);
    }

    public string? GetValue(string key) => Document.GetStringValue(key);

    public void SetValue(string key, string value, bool isComposite = false)
    {
        var desc = BriefingTextTableCatalog.GetDescriptor(key);
        Document.SetVariable(key, value, isComposite || desc.ValueType == BriefingVariableType.CompositeString);
    }

    public IReadOnlyList<string> GetAllKeys()
    {
        return Document.Variables.Select(v => v.Key).Distinct(StringComparer.OrdinalIgnoreCase).ToList();
    }

    public string ToScriptText() => Document.ToScriptText();
}
