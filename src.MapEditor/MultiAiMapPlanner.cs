using System.Text.Json;

namespace AgainstRomeMapEditor;

internal sealed record AiMapRoleProgress(AiMapDesignRole Role, string Model, int CompletedRoles, bool Finished, AiMapRoleStatus? Status = null);

internal enum AiMapDesignRole { Terrain, Water, Materials }
internal enum AiMapRoleStatus { Succeeded, Failed, Cancelled }

internal sealed record MultiAiMapRoleRequest(AiMapDesignRole Role, string Model);
internal sealed record AiMapRoleResult(AiMapDesignRole Role, string Model, AiMapRoleStatus Status, int AcceptedFeatures,
    string? Error, IReadOnlyList<string> Warnings, string? RawResponse);

internal sealed record MultiAiMapPlanResult(AiMapPlan? Plan, IReadOnlyList<AiMapRoleResult> Roles, bool IsCancelled)
{
    public bool HasFailures => Roles.Any(role => role.Status != AiMapRoleStatus.Succeeded);
    public IReadOnlyList<string> Warnings => Roles.SelectMany(role => role.Warnings).ToArray();
    public string RawResponse => string.Join("\n\n", Roles.Select(role => $"[{RoleName(role.Role)} / {role.Model} / {role.Status}]\n{role.Error ?? role.RawResponse}"));
    internal static string RoleName(AiMapDesignRole role) => role switch
    {
        AiMapDesignRole.Terrain => "地形", AiMapDesignRole.Water => "水系", _ => "材質",
    };
}

/// <summary>獨立、可注入的多角色協調器；只產生計畫，絕不操作 UI、地圖或檔案。</summary>
internal sealed class MultiAiMapPlanner
{
    internal delegate Task<(AiMapPlan Plan, string RawResponse)> GenerateRolePlan(string model, string description,
        IReadOnlyList<AiMaterialOption> materials, float waterLevelSample, CancellationToken cancellationToken);

    private readonly GenerateRolePlan _generate;
    private static readonly AiMapDesignRole[] RoleOrder = [AiMapDesignRole.Terrain, AiMapDesignRole.Water, AiMapDesignRole.Materials];
    internal const int FeaturesPerRole = AiMapPlan.MaxFeatures / 3;

    public MultiAiMapPlanner(OllamaMapPlanner planner) : this(planner.GeneratePlanAsync) { }
    public MultiAiMapPlanner(GenerateRolePlan generate) => _generate = generate ?? throw new ArgumentNullException(nameof(generate));

    public Task<MultiAiMapPlanResult> GeneratePlanAsync(string model, string description,
        IReadOnlyList<AiMaterialOption> materials, float waterLevelSample, CancellationToken cancellationToken)
    {
        if (string.IsNullOrWhiteSpace(model)) throw new ArgumentException("請選擇 AI 模型。", nameof(model));
        return GeneratePlanAsync(RoleOrder.Select(role => new MultiAiMapRoleRequest(role, model)).ToArray(),
            description, materials, waterLevelSample, cancellationToken);
    }

    public async Task<MultiAiMapPlanResult> GeneratePlanAsync(IReadOnlyList<MultiAiMapRoleRequest> requests, string description,
        IReadOnlyList<AiMaterialOption> materials, float waterLevelSample, CancellationToken cancellationToken, IProgress<AiMapRoleProgress>? progress = null)
    {
        ArgumentNullException.ThrowIfNull(requests);
        if (requests.Count == 0 || requests.Any(request => !RoleOrder.Contains(request.Role)) || requests.Select(r => r.Role).Distinct().Count() != requests.Count)
            throw new ArgumentException("請選擇至少一個角色，每個角色只能指定一個模型。", nameof(requests));
        if (requests.Any(request => string.IsNullOrWhiteSpace(request.Model)))
            throw new ArgumentException("每個角色都必須選擇 AI 模型。", nameof(requests));
        if (string.IsNullOrWhiteSpace(description)) throw new ArgumentException("請輸入地圖描述。", nameof(description));
        ArgumentNullException.ThrowIfNull(materials);
        // Snapshot inputs so UI changes during generation cannot change different roles' contracts.
        AiMaterialOption[] snapshot = materials.ToArray();
        MultiAiMapRoleRequest[] roleRequests = requests.ToArray();
        var results = new List<(AiMapPlan? Plan, AiMapRoleResult Report)>();
        // 本機硬體一次只處理一個推論；角色仍保留獨立模型、配額與診斷。
        foreach (AiMapDesignRole role in RoleOrder.Where(role => roleRequests.Any(request => request.Role == role)))
        {
            string model = roleRequests.Single(request => request.Role == role).Model;
            if (!cancellationToken.IsCancellationRequested) progress?.Report(new(role, model, results.Count, false));
            var result = await GenerateAsync(role, model, description, snapshot, waterLevelSample, cancellationToken).ConfigureAwait(false);
            results.Add(result);
            progress?.Report(new(role, model, results.Count, true, result.Report.Status));
        }
        AiMapRoleResult[] reports = results.Select(result => result.Report).ToArray();
        if (cancellationToken.IsCancellationRequested) return new(null, reports, true);
        if (results.All(result => result.Plan is null)) return new(null, reports, false);
        var merged = new AiMapPlan
        {
            BaseHeight = results.FirstOrDefault(result => result.Report.Role == AiMapDesignRole.Terrain).Plan?.BaseHeight,
            BaseMaterial = results.FirstOrDefault(result => result.Report.Role == AiMapDesignRole.Materials).Plan?.BaseMaterial,
            Features = results.SelectMany(result => result.Plan?.Features ?? []).ToList(),
            Summary = string.Join("；", results.Select(result => result.Plan is null ? null :
                $"{MultiAiMapPlanResult.RoleName(result.Report.Role)}：{result.Plan.Summary ?? "完成"}").OfType<string>()),
        };
        // Use the same normalization as the single-AI path. The disjoint scopes and per-role budget
        // guarantee this final pass cannot silently discard another role due to MaxFeatures.
        return new(AiMapPlan.Parse(JsonSerializer.Serialize(merged), snapshot.Select(item => item.Id).ToArray()), reports, false);
    }

    private async Task<(AiMapPlan? Plan, AiMapRoleResult Report)> GenerateAsync(AiMapDesignRole role, string model,
        string description, IReadOnlyList<AiMaterialOption> materials, float water, CancellationToken token)
    {
        string? raw = null;
        var warnings = new List<string>();
        string name = MultiAiMapPlanResult.RoleName(role);
        try
        {
            token.ThrowIfCancellationRequested();
            var generated = await _generate(model, BuildRolePrompt(role, description), materials, water, token)
                .WaitAsync(token).ConfigureAwait(false);
            raw = generated.RawResponse;
            token.ThrowIfCancellationRequested();
            int originalCount = generated.Plan.Features.Count;
            // Ollama's parser already normalizes. Inspect the raw feature count to expose its drops
            // (including invalid items, duplicates and MaxFeatures truncation) instead of losing them.
            int start = raw.IndexOf('{'), end = raw.LastIndexOf('}');
            if (start >= 0 && end > start)
            {
                try
                {
                    using JsonDocument document = JsonDocument.Parse(raw[start..(end + 1)], new JsonDocumentOptions
                    { AllowTrailingCommas = true, CommentHandling = JsonCommentHandling.Skip });
                    if (document.RootElement.TryGetProperty("features", out var features) && features.ValueKind == JsonValueKind.Array)
                        originalCount = Math.Max(originalCount, features.GetArrayLength());
                }
                catch (JsonException) { /* The injected planner's normalized plan is still validated below. */ }
            }
            AiMapPlan normalized = AiMapPlan.Parse(JsonSerializer.Serialize(generated.Plan), materials.Select(item => item.Id).ToArray());
            if (originalCount > normalized.Features.Count)
                warnings.Add($"{name}：正規化移除或截斷 {originalCount - normalized.Features.Count} 個無效、重複或超量特徵。");
            string[] allowed = role switch
            {
                AiMapDesignRole.Terrain => ["hill", "mountain", "valley", "plateau", "ridge", "blocked", "passable"],
                AiMapDesignRole.Water => ["lake", "river"],
                _ => ["material"],
            };
            List<AiMapFeature> scoped = normalized.Features.Where(feature => allowed.Contains(feature.Type)).ToList();
            int rejected = normalized.Features.Count - scoped.Count;
            if (rejected > 0) warnings.Add($"{name}：忽略 {rejected} 個超出角色範圍的特徵。");
            if (role != AiMapDesignRole.Terrain && normalized.BaseHeight is not null)
                warnings.Add($"{name}：忽略超出角色範圍的 baseHeight。");
            if (role != AiMapDesignRole.Materials && normalized.BaseMaterial is not null)
                warnings.Add($"{name}：忽略超出角色範圍的 baseMaterial。");
            if (role != AiMapDesignRole.Materials)
            {
                int attached = scoped.Count(feature => feature.Material is not null);
                if (attached > 0) warnings.Add($"{name}：忽略 {attached} 個附帶材質，材質由材質角色負責。");
                foreach (AiMapFeature feature in scoped) feature.Material = null;
            }
            int beforeScopeDeduplication = scoped.Count;
            scoped = scoped.DistinctBy(feature => (feature.Type, feature.X, feature.Y, feature.X2, feature.Y2,
                feature.Radius, feature.Amount, feature.Material)).ToList();
            if (scoped.Count < beforeScopeDeduplication)
                warnings.Add($"{name}：移除角色範圍處理後重複的 {beforeScopeDeduplication - scoped.Count} 個特徵。");
            if (scoped.Count > FeaturesPerRole)
                warnings.Add($"{name}：超過角色上限 {FeaturesPerRole}，截斷 {scoped.Count - FeaturesPerRole} 個特徵。");
            var plan = new AiMapPlan
            {
                Summary = normalized.Summary,
                BaseHeight = role == AiMapDesignRole.Terrain ? normalized.BaseHeight : null,
                BaseMaterial = role == AiMapDesignRole.Materials ? normalized.BaseMaterial : null,
                Features = scoped.Take(FeaturesPerRole).ToList(),
            };
            if (plan.Features.Count == 0 && plan.BaseHeight is null && plan.BaseMaterial is null)
                throw new InvalidDataException("角色未產生任何可套用的範圍內內容。");
            return (plan, new(role, model, AiMapRoleStatus.Succeeded, plan.Features.Count, null, warnings, raw));
        }
        catch (OperationCanceledException ex)
        {
            return (null, new(role, model, AiMapRoleStatus.Cancelled, 0, token.IsCancellationRequested ? "已取消。" : "角色請求逾時或取消：" + ex.Message, warnings, raw));
        }
        catch (Exception ex)
        {
            return (null, new(role, model, AiMapRoleStatus.Failed, 0, ex.Message, warnings, raw));
        }
    }

    internal static string BuildRolePrompt(AiMapDesignRole role, string description)
    {
        string scope = role switch
        {
            AiMapDesignRole.Terrain => "Design ONLY hill, mountain, valley, plateau, ridge, blocked and passable features. You alone may set baseHeight. Do not set baseMaterial or any material field. Do not design rivers or lakes.",
            AiMapDesignRole.Water => "Design ONLY lake and river features. Do not set baseHeight, baseMaterial or any material field. Preserve dry settlement space.",
            _ => "Design ONLY material features. You alone may set baseMaterial. Do not set baseHeight or create height, water or collision features. Use materials available in the schema.",
        };
        return $"You are the {role} specialist in a concurrent map design team. Other specialists work from the SAME map request and compass locations. {scope}\nReturn at most {FeaturesPerRole} features. Your role restrictions override general suggestions about feature count or attached materials.\nMap request:\n{description.Trim()}";
    }
}
