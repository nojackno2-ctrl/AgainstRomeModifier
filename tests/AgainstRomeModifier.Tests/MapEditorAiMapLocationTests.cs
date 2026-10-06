using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class MapEditorAiMapLocationTests
{
    private static readonly string[] Materials = ["BB", "BC", "B9"];

    [Fact]
    public void Named_locations_override_coordinates_and_invalid_linear_features_are_dropped()
    {
        AiMapPlan plan = AiMapPlan.Parse("""
            {"summary":"s","features":[
              {"type":"mountain","location":"north-east","x":1,"y":1,"radius":8,"amount":100},
              {"type":"lake","location":"South-West","radius":5},
              {"type":"river","location":"north-west","toLocation":"south-east","radius":2,"material":"BC"},
              {"type":"river","location":"north-west","radius":2},
              {"type":"ridge","location":"center","toLocation":"center","radius":2},
              {"type":"lake","location":"South-West","radius":5}
            ]}
            """, Materials);

        Assert.Equal(3, plan.Features.Count); // 無終點河流、起訖相同山脊、重複湖泊都被移除
        AiMapFeature mountain = plan.Features[0];
        Assert.Equal(AiMapPlan.Locations["north-east"], (mountain.X, mountain.Y));
        Assert.Equal(AiMapPlan.Locations["south-west"], (plan.Features[1].X, plan.Features[1].Y));
        AiMapFeature river = plan.Features[2];
        Assert.Equal(AiMapPlan.Locations["north-west"], (river.X, river.Y));
        Assert.Equal(AiMapPlan.Locations["south-east"], (river.X2!.Value, river.Y2!.Value));
    }

    [Fact]
    public void North_east_is_the_top_right_of_the_tile_grid()
    {
        (int x, int y) = AiMapPlan.Locations["north-east"];
        Assert.True(x > 32 && y < 32);
        (x, y) = AiMapPlan.Locations["south-west"];
        Assert.True(x < 32 && y > 32);
    }

    [Fact]
    public void Feature_materials_are_painted_along_rivers_and_over_area_features()
    {
        var session = new TerrainHeightEditSession(257, Enumerable.Repeat((byte)100, 257 * 257).ToArray(), null, 0, null);
        AiMapPlan plan = AiMapPlan.Parse("""
            {"summary":"s","features":[
              {"type":"river","location":"west","toLocation":"east","radius":2,"material":"BC"},
              {"type":"mountain","location":"north","radius":10,"amount":90,"material":"B9"}
            ]}
            """, Materials);
        var calls = new List<(string Id, float X, float Y, float Radius)>();

        AiMapApplyResult result = AiMapPlanApplier.Apply(plan, session, 64, 60, (id, x, y, radius) => { calls.Add((id, x, y, radius)); return true; });

        Assert.Contains(calls, call => call.Id == "B9" && call.X == AiMapPlan.Locations["north"].X + .5f && call.Y == AiMapPlan.Locations["north"].Y + .5f);
        List<(string Id, float X, float Y, float Radius)> sand = calls.Where(call => call.Id == "BC").ToList();
        Assert.True(sand.Count >= 10, $"河岸沙地應沿河流塗多點，實際 {sand.Count} 點");
        Assert.All(sand, call => Assert.Equal(AiMapPlan.Locations["west"].Y + .5f, call.Y));
        Assert.Equal(calls.Count, result.MaterialStrokes);
        Assert.Equal(0, result.RejectedMaterialStrokes);
    }

    /// <summary>實際呼叫本機 Ollama；預設略過，設定 ARM_OLLAMA_LIVE=1（可加 ARM_OLLAMA_MODEL）才執行。</summary>
    [Fact]
    public async Task Live_local_ollama_plan_places_requested_regions()
    {
        if (Environment.GetEnvironmentVariable("ARM_OLLAMA_LIVE") != "1") return;
        string model = Environment.GetEnvironmentVariable("ARM_OLLAMA_MODEL") ?? "gemma3:12b";
        using var planner = new OllamaMapPlanner();
        AiMaterialOption[] materials = [new("BB", "草地 / Grass"), new("BC", "沙地 / Sand"), new("B9", "岩地 / Rock")];
        (AiMapPlan plan, string raw) = await planner.GeneratePlanAsync(model,
            "東北方是高聳山脈，西南有一座小湖，一條河從西北流向東南，其餘保留平坦草地。", materials, 60, CancellationToken.None);

        Assert.NotEmpty(plan.Features);
        Assert.Contains(plan.Features, feature => feature.Type is "mountain" or "hill" or "ridge" && feature.X > 32 && feature.Y < 32);
        Assert.Contains(plan.Features, feature => feature.Type == "lake" && feature.X < 32 && feature.Y > 32);
        var session = new TerrainHeightEditSession(257, Enumerable.Repeat((byte)75, 257 * 257).ToArray(), null, 0, null);
        AiMapApplyResult result = AiMapPlanApplier.Apply(plan, session, 64, 60, (_, _, _, _) => true);
        Assert.True(result.HeightSamplesChanged > 1000, "AI 計畫應實際改變地形。原始回應：" + raw);
    }
}
