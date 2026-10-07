using System.Net.Http.Json;
using System.Text;
using System.Text.Json;

namespace AgainstRomeMapEditor;

/// <summary>可供 AI 使用的材質（ID 給模型輸出、名稱給模型理解）。</summary>
internal sealed record AiMaterialOption(string Id, string Name);

/// <summary>
/// 透過本機 Ollama（預設 http://localhost:11434，可用環境變數 ARM_OLLAMA_URL 覆寫）產生 <see cref="AiMapPlan"/>。
/// 只傳送使用者的描述、材質清單與地圖參數；不傳送任何地圖檔案，也不連線到外部服務。
/// </summary>
internal sealed class OllamaMapPlanner : IDisposable
{
    private readonly HttpClient _http;
    private static readonly SemaphoreSlim InferenceGate = new(1, 1);

    public OllamaMapPlanner(string? baseUrl = null, HttpMessageHandler? handler = null)
    {
        string url = baseUrl ?? Environment.GetEnvironmentVariable("ARM_OLLAMA_URL") ?? "http://localhost:11434";
        _http = handler is null ? new HttpClient() : new HttpClient(handler);
        _http.BaseAddress = new Uri(url.TrimEnd('/') + "/");
        _http.Timeout = TimeSpan.FromMinutes(10);
    }

    public Uri Endpoint => _http.BaseAddress!;

    public async Task<IReadOnlyList<string>> ListModelsAsync(CancellationToken cancellationToken)
    {
        using JsonDocument document = JsonDocument.Parse(await _http.GetStringAsync("api/tags", cancellationToken));
        return document.RootElement.TryGetProperty("models", out JsonElement models)
            ? models.EnumerateArray().Select(model => model.GetProperty("name").GetString()).OfType<string>().OrderBy(name => name, StringComparer.OrdinalIgnoreCase).ToArray()
            : Array.Empty<string>();
    }

    public async Task<(AiMapPlan Plan, string RawResponse)> GeneratePlanAsync(string model, string description, IReadOnlyList<AiMaterialOption> materials,
        float waterLevelSample, CancellationToken cancellationToken)
    {
        if (string.IsNullOrWhiteSpace(model)) throw new ArgumentException("請選擇 AI 模型。", nameof(model));
        if (string.IsNullOrWhiteSpace(description)) throw new ArgumentException("請輸入地圖描述。", nameof(description));
        string[] ids = materials.Select(item => item.Id).ToArray();
        var request = new Dictionary<string, object>
        {
            ["model"] = model,
            ["stream"] = false,
            ["think"] = false,
            ["format"] = AiMapPlan.JsonSchema(ids),
            ["options"] = new Dictionary<string, object> { ["temperature"] = 0.4, ["repeat_penalty"] = 1.15, ["num_predict"] = 2048 },
            ["messages"] = new object[]
            {
                new Dictionary<string, string> { ["role"] = "system", ["content"] = BuildSystemPrompt(materials, waterLevelSample) },
                new Dictionary<string, string> { ["role"] = "user", ["content"] = description.Trim() },
            },
        };
        await InferenceGate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            using HttpResponseMessage response = await _http.PostAsJsonAsync("api/chat", request, cancellationToken);
            string body = await response.Content.ReadAsStringAsync(cancellationToken);
            if (!response.IsSuccessStatusCode) throw new HttpRequestException($"Ollama 回應錯誤 {(int)response.StatusCode}：{body}");
            using JsonDocument document = JsonDocument.Parse(body);
            string content = document.RootElement.GetProperty("message").GetProperty("content").GetString() ?? "";
            return (AiMapPlan.Parse(content, ids), content);
        }
        finally { InferenceGate.Release(); }
    }

    internal static string BuildSystemPrompt(IReadOnlyList<AiMaterialOption> materials, float waterLevelSample)
    {
        var text = new StringBuilder();
        text.AppendLine("You design terrain for a 64x64-tile map of the RTS game Against Rome. Reply ONLY with a JSON map plan matching the schema.");
        text.AppendLine("Place every feature with \"location\" (one of: " + string.Join(", ", AiMapPlan.Locations.Keys) + "). Map north is the top edge, east is the right edge.");
        text.AppendLine("Linear features (river, ridge, blocked line) go from \"location\" to \"toLocation\". Do not output x/y numbers. Every river and ridge MUST have a toLocation different from its location. Never repeat a feature; use 4-16 features in total.");
        text.AppendLine("A feature's optional material is also painted over that feature (e.g. sand along a river, rock on a mountain).");
        text.AppendLine($"Heights use a 0-255 scale. Ground below {waterLevelSample:0} becomes water; typical dry land is {waterLevelSample + 15:0}-{waterLevelSample + 90:0}.");
        text.AppendLine("Optional baseHeight flattens the whole map first (use it for a fresh map, e.g. a value a little above the water level). Optional baseMaterial repaints the whole ground first.");
        text.AppendLine("Feature types (radius in tiles, 1-32):");
        text.AppendLine("- hill / mountain: raise around the location; amount = height gain (hill ~30-60, mountain ~80-140).");
        text.AppendLine("- valley: lower around the location by amount.");
        text.AppendLine("- plateau: flatten the area to absolute height amount.");
        text.AppendLine("- lake: carve a lake below water; amount = depth below the water level (5-30).");
        text.AppendLine("- ridge: raised line from location to toLocation; radius = half width; amount = height gain.");
        text.AppendLine("- river: water channel from location to toLocation; radius = half width (1-3); amount = depth below water.");
        text.AppendLine("- material: paint ground material inside the circle; material must be one of the ids below.");
        text.AppendLine("- blocked / passable: mark impassable or passable ground (line if toLocation given).");
        text.AppendLine("Keep settlements playable: leave several large flat dry areas, avoid covering the whole map with water or mountains, and give each region a distinct location.");
        text.AppendLine("Available materials (id: name):");
        foreach (AiMaterialOption material in materials) text.AppendLine($"- {material.Id}: {material.Name}");
        text.AppendLine("Write the summary in the same language as the user's request.");
        return text.ToString();
    }

    public void Dispose() => _http.Dispose();
}
