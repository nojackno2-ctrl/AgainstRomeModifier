using System.Text.Json;

namespace AgainstRomeModifier.Core.Services;

/// <summary>
/// 共用的 <see cref="JsonSerializerOptions"/> 實例。
/// 本專案所有寫出的 JSON（安裝標記、備份 manifest、地圖 manifest、settings.json）
/// 都是給人讀的小檔案，格式一致採縮排輸出。
/// JsonSerializerOptions 每次 new 都會重建序列化快取，故一律共用單一唯讀實例。
/// </summary>
public static class JsonDefaults
{
    /// <summary>縮排輸出；建立後不再變更，可安全跨執行緒共用。</summary>
    public static readonly JsonSerializerOptions Indented = new() { WriteIndented = true };
}
