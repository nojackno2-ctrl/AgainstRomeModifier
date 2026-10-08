using System.Text;
using AgainstRomeModifier;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeMapEditor.Modules.Localization;

/// <summary>
/// 多語系簡報與任務文字存檔安全交易服務。
/// 提供儲存前置檢查、多語系原子性回滾、AST 無損排版與註解保留、PFIL 容器無損壓縮維持。
/// </summary>
public static class BriefingSaveTransaction
{
    private static bool IsPfil(byte[] bytes) =>
        bytes.Length >= 64 && bytes[0] == 'P' && bytes[1] == 'F' && bytes[2] == 'I' && bytes[3] == 'L';

    /// <summary>
    /// 執行儲存前安全預檢（Fail-Fast）。
    /// 若有任何語系包含語法錯誤、常值長度超標（> 100 bytes）或編碼不支援字元，立即擲出例外並阻擋存檔。
    /// </summary>
    public static void PreflightValidate(IReadOnlyDictionary<SupportedLanguage, BriefingTextTable> languageTables)
    {
        ArgumentNullException.ThrowIfNull(languageTables);

        foreach ((SupportedLanguage lang, BriefingTextTable table) in languageTables)
        {
            string encodingName = lang.ToEncodingName();
            var report = PutScriptSyntaxValidator.ValidateTable(table, encodingName);

            if (!report.IsValid)
            {
                var firstError = report.Issues.First(i => i.Severity == PutIssueSeverity.Error);
                throw new InvalidDataException(
                    $"[{lang.ToDisplayName()}] 簡報變數 '{firstError.Key}' 驗證失敗: {firstError.Message}" +
                    (firstError.OffendingFragment is not null ? $" (片段: {firstError.OffendingFragment})" : ""));
            }
        }
    }

    /// <summary>
    /// 執行多語系簡報交易式覆寫。
    /// 保留既有檔案開頭註解、縮排與未修改變數，支援原子性回滾與 PFIL 壓縮標頭還原。
    /// </summary>
    public static void SaveMultiLanguageBriefings(
        string mapDirectory,
        IReadOnlyDictionary<SupportedLanguage, BriefingTextTable> languageTables,
        global::AgainstRomeModifier.FileRollbackScope rollback)
    {
        ArgumentNullException.ThrowIfNull(mapDirectory);
        ArgumentNullException.ThrowIfNull(languageTables);
        ArgumentNullException.ThrowIfNull(rollback);

        // 1. 預檢：確保所有語系資料均合法且安全
        PreflightValidate(languageTables);

        // 2. 逐一覆寫各語系資料夾
        string textRoot = Path.Combine(mapDirectory, "TEXT");

        foreach ((SupportedLanguage lang, BriefingTextTable table) in languageTables)
        {
            string langDirName = lang.ToDirectoryCode();
            string langDir = Path.Combine(textRoot, langDirName);
            string briefingPath = Path.Combine(langDir, "briefing.put");

            Encoding encoding = PutScriptSyntaxValidator.GetStrictEncoding(lang.ToEncodingName());
            byte[] finalBytes = PrepareBriefingBytes(briefingPath, table, encoding);

            SafeFileWriter.WriteAllBytes(briefingPath, finalBytes, rollback);
        }
    }

    /// <summary>
    /// 準備待寫入的 briefing.put 位元組資料。
    /// 若原檔案存在，採用 AST 無損合併；若不存在，產生新腳本；若原本為 PFIL 壓縮，重新壓縮。
    /// </summary>
    internal static byte[] PrepareBriefingBytes(string filePath, BriefingTextTable newTable, Encoding encoding)
    {
        byte[]? originalBytes = null;
        PutScriptDocument docToSave;

        if (File.Exists(filePath))
        {
            originalBytes = File.ReadAllBytes(filePath);
            byte[] textBytes = IsPfil(originalBytes) ? GameLZSS.DecompressPfil(originalBytes) : originalBytes;
            string originalText = encoding.GetString(textBytes);

            // 解析既有腳本 AST 保留註解與格式
            docToSave = PutScriptDocument.Parse(originalText);

            // 將新表格中的變更無損套用至 AST
            foreach (var newVar in newTable.Document.Variables)
            {
                if (newVar.IntegerValue.HasValue)
                {
                    docToSave.SetVariable(newVar.Key, newVar.IntegerValue.Value);
                }
                else if (newVar.StringValue is not null)
                {
                    docToSave.SetVariable(newVar.Key, newVar.StringValue, newVar.IsComposite, encoding);
                }
            }
        }
        else
        {
            // 全新建立
            docToSave = newTable.Document;
        }

        string serializedText = docToSave.ToScriptText();
        byte[] rawBytes = encoding.GetBytes(serializedText);

        if (originalBytes is not null && IsPfil(originalBytes))
        {
            return GameLZSS.CompressPfil(rawBytes, originalBytes);
        }

        return rawBytes;
    }
}
