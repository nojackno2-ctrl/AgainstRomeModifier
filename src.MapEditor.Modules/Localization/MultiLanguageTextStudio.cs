using System.Text;

namespace AgainstRomeMapEditor.Modules.Localization;

/// <summary>
/// 支援的語系代碼列舉。
/// </summary>
public enum SupportedLanguage
{
    TraditionalChinese, // ZH / TW / CHT
    English,            // US / EN
    German,             // DE / GR
    Russian             // RU
}

public static class SupportedLanguageExtensions
{
    public static string ToDirectoryCode(this SupportedLanguage language) => language switch
    {
        SupportedLanguage.TraditionalChinese => "ZH",
        SupportedLanguage.English => "US",
        SupportedLanguage.German => "DE",
        SupportedLanguage.Russian => "RU",
        _ => "US"
    };

    public static string ToEncodingName(this SupportedLanguage language) => language switch
    {
        SupportedLanguage.TraditionalChinese => "BIG5",
        SupportedLanguage.English => "WINDOWS-1252",
        SupportedLanguage.German => "WINDOWS-1252",
        SupportedLanguage.Russian => "WINDOWS-1251",
        _ => "WINDOWS-1252"
    };

    public static string ToDisplayName(this SupportedLanguage language) => language switch
    {
        SupportedLanguage.TraditionalChinese => "繁體中文 (Traditional Chinese)",
        SupportedLanguage.English => "英文 (English)",
        SupportedLanguage.German => "德文 (German)",
        SupportedLanguage.Russian => "俄文 (Russian)",
        _ => language.ToString()
    };
}

/// <summary>
/// 單一語言槽位狀態。
/// </summary>
public enum SlotSyncStatus
{
    Synchronized, // 與基準同步完成
    Missing,      // 缺漏（尚未提供翻譯）
    Stale,        // 基準語系已有變更，此語言需要重新校對
    EncodingError,// 包含無法編碼的字元
    SyntaxError   // 語法或長度超標
}

/// <summary>
/// 單一語言文字槽位。
/// </summary>
public sealed class StudioLanguageSlot
{
    public SupportedLanguage Language { get; }
    public string Text { get; set; } = "";
    public string OriginalText { get; set; } = "";
    public bool IsDirty => !string.Equals(Text, OriginalText, StringComparison.Ordinal);
    public SlotSyncStatus Status { get; set; } = SlotSyncStatus.Synchronized;
    public DateTime LastModified { get; set; } = DateTime.UtcNow;

    public StudioLanguageSlot(SupportedLanguage language, string initialText = "")
    {
        Language = language;
        Text = initialText;
        OriginalText = initialText;
        Status = string.IsNullOrWhiteSpace(initialText) ? SlotSyncStatus.Missing : SlotSyncStatus.Synchronized;
    }
}

/// <summary>
/// 多語系工作台中的單一文字項目（包含中、英、德各槽位）。
/// </summary>
public sealed class StudioTextEntry
{
    public string Key { get; }
    public BriefingVariableDescriptor Descriptor { get; }
    private readonly Dictionary<SupportedLanguage, StudioLanguageSlot> _slots = new();

    public IReadOnlyDictionary<SupportedLanguage, StudioLanguageSlot> Slots => _slots;

    public int MasterVersion { get; set; } = 1;
    public Dictionary<SupportedLanguage, int> AlignedVersions { get; } = new();

    public StudioTextEntry(string key, BriefingVariableDescriptor descriptor)
    {
        Key = key ?? throw new ArgumentNullException(nameof(key));
        Descriptor = descriptor ?? throw new ArgumentNullException(nameof(descriptor));

        foreach (SupportedLanguage lang in Enum.GetValues<SupportedLanguage>())
        {
            _slots[lang] = new StudioLanguageSlot(lang);
            AlignedVersions[lang] = 0;
        }
    }

    public StudioLanguageSlot GetSlot(SupportedLanguage lang) => _slots[lang];

    public string GetText(SupportedLanguage lang) => _slots[lang].Text;

    public void SetText(SupportedLanguage lang, string text)
    {
        var slot = _slots[lang];
        slot.Text = text ?? "";
        slot.LastModified = DateTime.UtcNow;
    }
}

/// <summary>
/// 多語系並排即時編輯器工作階段模型（MultiLanguageTextStudio）。
/// 支援雙向即時編輯、缺漏檢查、基準語言連動與完成率統計。
/// </summary>
public sealed class MultiLanguageTextStudio
{
    private readonly Dictionary<string, StudioTextEntry> _entries = new(StringComparer.OrdinalIgnoreCase);

    public SupportedLanguage MasterLanguage { get; set; } = SupportedLanguage.TraditionalChinese;

    public IReadOnlyCollection<StudioTextEntry> Entries => _entries.Values;

    public StudioTextEntry GetOrCreateEntry(string key)
    {
        if (_entries.TryGetValue(key, out var entry)) return entry;
        var desc = BriefingTextTableCatalog.GetDescriptor(key);
        entry = new StudioTextEntry(key, desc);
        _entries[key] = entry;
        return entry;
    }

    /// <summary>
    /// 從多個語言的 BriefingTextTable 載入至工作台。
    /// </summary>
    public void LoadFromTables(IReadOnlyDictionary<SupportedLanguage, BriefingTextTable> tables)
    {
        ArgumentNullException.ThrowIfNull(tables);

        // 收集所有出現過的鍵值
        var allKeys = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var desc in BriefingTextTableCatalog.GetAllStandardDescriptors())
        {
            allKeys.Add(desc.Key);
        }

        foreach ((_, BriefingTextTable table) in tables)
        {
            foreach (string k in table.GetAllKeys())
            {
                allKeys.Add(k);
            }
        }

        foreach (string key in allKeys)
        {
            var entry = GetOrCreateEntry(key);
            foreach ((SupportedLanguage lang, BriefingTextTable table) in tables)
            {
                string val = table.GetValue(key) ?? "";
                var slot = entry.GetSlot(lang);
                slot.Text = val;
                slot.OriginalText = val;
                slot.Status = string.IsNullOrWhiteSpace(val) ? SlotSyncStatus.Missing : SlotSyncStatus.Synchronized;
            }
        }

        RefreshParityStatus();
    }

    /// <summary>
    /// 將工作台內容匯出為各語言的 BriefingTextTable。
    /// </summary>
    public Dictionary<SupportedLanguage, BriefingTextTable> ExportToTables()
    {
        var result = new Dictionary<SupportedLanguage, BriefingTextTable>();

        foreach (SupportedLanguage lang in Enum.GetValues<SupportedLanguage>())
        {
            var table = BriefingTextTable.CreateEmpty();
            foreach (var entry in _entries.Values)
            {
                string text = entry.GetText(lang);
                // 若非空，或為標準必要鍵，則寫入
                if (!string.IsNullOrWhiteSpace(text) || entry.Descriptor.IsRequired)
                {
                    table.SetValue(entry.Key, text, isComposite: entry.Descriptor.ValueType == BriefingVariableType.CompositeString);
                }
            }
            result[lang] = table;
        }

        return result;
    }

    /// <summary>
    /// 更新單一語言槽位文字，並自動維護基準語言與其他語言之同步狀態。
    /// </summary>
    public void UpdateText(string key, SupportedLanguage lang, string newText)
    {
        var entry = GetOrCreateEntry(key);
        entry.SetText(lang, newText);

        if (lang == MasterLanguage)
        {
            entry.MasterVersion++;
            entry.AlignedVersions[lang] = entry.MasterVersion;

            // 當基準語言更新時，其他非空語言若未同步則標示為 Stale
            foreach (SupportedLanguage otherLang in Enum.GetValues<SupportedLanguage>())
            {
                if (otherLang == MasterLanguage) continue;
                var otherSlot = entry.GetSlot(otherLang);
                if (string.IsNullOrWhiteSpace(otherSlot.Text))
                {
                    otherSlot.Status = SlotSyncStatus.Missing;
                }
                else
                {
                    otherSlot.Status = SlotSyncStatus.Stale;
                }
            }
        }
        else
        {
            // 編輯從屬語言時，對齊版本
            entry.AlignedVersions[lang] = entry.MasterVersion;
            var slot = entry.GetSlot(lang);
            slot.Status = string.IsNullOrWhiteSpace(newText) ? SlotSyncStatus.Missing : SlotSyncStatus.Synchronized;
        }

        ValidateSlot(entry, lang);
    }

    /// <summary>
    /// 重新計算全庫對齊與缺漏狀態。
    /// </summary>
    public void RefreshParityStatus()
    {
        foreach (var entry in _entries.Values)
        {
            string masterText = entry.GetText(MasterLanguage);
            bool hasMaster = !string.IsNullOrWhiteSpace(masterText);

            foreach (SupportedLanguage lang in Enum.GetValues<SupportedLanguage>())
            {
                if (lang == MasterLanguage)
                {
                    entry.GetSlot(lang).Status = hasMaster ? SlotSyncStatus.Synchronized : SlotSyncStatus.Missing;
                }
                else
                {
                    var slot = entry.GetSlot(lang);
                    if (string.IsNullOrWhiteSpace(slot.Text))
                    {
                        slot.Status = SlotSyncStatus.Missing;
                    }
                    else if (entry.AlignedVersions.TryGetValue(lang, out int v) && v < entry.MasterVersion)
                    {
                        slot.Status = SlotSyncStatus.Stale;
                    }
                    else
                    {
                        slot.Status = SlotSyncStatus.Synchronized;
                    }
                }

                ValidateSlot(entry, lang);
            }
        }
    }

    private static void ValidateSlot(StudioTextEntry entry, SupportedLanguage lang)
    {
        var slot = entry.GetSlot(lang);
        if (string.IsNullOrWhiteSpace(slot.Text)) return;

        try
        {
            Encoding encoding = PutScriptSyntaxValidator.GetStrictEncoding(lang.ToEncodingName());
            encoding.GetByteCount(slot.Text);
        }
        catch (EncoderFallbackException)
        {
            slot.Status = SlotSyncStatus.EncodingError;
            return;
        }

        if (entry.Descriptor.ValueType == BriefingVariableType.SingleLineString && (slot.Text.Contains('\r') || slot.Text.Contains('\n')))
        {
            slot.Status = SlotSyncStatus.SyntaxError;
        }
    }

    /// <summary>
    /// 計算指定語言的翻譯完成率（0.0 ~ 100.0%）。
    /// </summary>
    public double CalculateCoverage(SupportedLanguage lang)
    {
        int total = _entries.Count;
        if (total == 0) return 100.0;

        int filled = _entries.Values.Count(e => !string.IsNullOrWhiteSpace(e.GetText(lang)));
        return (double)filled / total * 100.0;
    }

    /// <summary>
    /// 取得指定語言中所有缺漏或過期的鍵值。
    /// </summary>
    public IReadOnlyList<string> GetMissingOrStaleKeys(SupportedLanguage lang)
    {
        return _entries.Values
            .Where(e => e.GetSlot(lang).Status is SlotSyncStatus.Missing or SlotSyncStatus.Stale)
            .Select(e => e.Key)
            .ToList();
    }
}

/// <summary>
/// 專為《反抗羅馬》設計的專用遊戲術語翻譯記憶體與對照字典。
/// </summary>
public static class TranslationGlossary
{
    private static readonly List<GlossaryTerm> Terms = new();

    static TranslationGlossary()
    {
        // 陣營與派系
        Add("羅馬人", "Romans", "Römer");
        Add("日耳曼人", "Germanics", "Germanen");
        Add("凱爾特人", "Celts", "Kelten");
        Add("匈人", "Huns", "Hunnen");

        // 建築與聚落
        Add("主屋", "Chieftain House", "Haupthaus");
        Add("聚落", "Settlement", "Siedlung");
        Add("兵營", "Barracks", "Kaserne");
        Add("瞭望塔", "Watchtower", "Wachturm");
        Add("木樁柵欄", "Palisade", "Palisade");
        Add("鐵匠鋪", "Blacksmith", "Schmiede");
        Add("神殿", "Temple", "Tempel");

        // 部隊與角色
        Add("步兵", "Infantry", "Infanterie");
        Add("弓箭手", "Archers", "Bogenschützen");
        Add("騎兵", "Cavalry", "Kavallerie");
        Add("首領", "Chieftain", "Häuptling");
        Add("祭司", "Druid", "Priester");

        // 任務目標與遊戲狀態
        Add("消滅所有敵方部隊", "Eliminate all enemy troops", "Vernichte alle feindlichen Truppen");
        Add("摧毀敵方聚落", "Destroy enemy settlement", "Zerstöre die feindliche Siedlung");
        Add("保護首領生存", "Protect the chieftain", "Beschütze den Häuptling");
        Add("佔領祭壇", "Capture the altar", "Erobere den Altar");
        Add("勝利", "Victory", "Sieg");
        Add("失敗", "Defeat", "Niederlage");
        Add("平手", "Draw", "Unentschieden");
    }

    private static void Add(string zh, string en, string de) => Terms.Add(new GlossaryTerm(zh, en, de));

    public static string Translate(string sourceText, SupportedLanguage from, SupportedLanguage to)
    {
        if (string.IsNullOrWhiteSpace(sourceText)) return sourceText;
        if (from == to) return sourceText;

        string result = sourceText;
        foreach (var term in Terms)
        {
            string fromStr = term.GetText(from);
            string toStr = term.GetText(to);
            if (!string.IsNullOrEmpty(fromStr) && !string.IsNullOrEmpty(toStr))
            {
                result = result.Replace(fromStr, toStr, StringComparison.OrdinalIgnoreCase);
            }
        }
        return result;
    }

    public static IReadOnlyList<GlossaryTerm> FindMatchingTerms(string text)
    {
        if (string.IsNullOrWhiteSpace(text)) return Array.Empty<GlossaryTerm>();
        return Terms.Where(t =>
            text.Contains(t.ZH, StringComparison.OrdinalIgnoreCase) ||
            text.Contains(t.EN, StringComparison.OrdinalIgnoreCase) ||
            text.Contains(t.DE, StringComparison.OrdinalIgnoreCase)).ToList();
    }

    public sealed record GlossaryTerm(string ZH, string EN, string DE)
    {
        public string GetText(SupportedLanguage lang) => lang switch
        {
            SupportedLanguage.TraditionalChinese => ZH,
            SupportedLanguage.English => EN,
            SupportedLanguage.German => DE,
            _ => EN
        };
    }
}
