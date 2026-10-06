using System.Net;
using System.Text;
using System.Text.Json;
using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class MapEditorAiMapTests
{
    private static readonly string[] MaterialIds = ["Grass", "Sand"];
    private static readonly AiMaterialOption[] Materials = [new("Grass", "Meadow"), new("Sand", "Shore")];

    [Fact]
    public void Parse_accepts_json_fences_and_leading_prose()
    {
        const string json = "{\"baseHeight\":70,\"features\":[]}";
        foreach (string response in new[] { "```json\n" + json + "\n```", "Here is your plan:\n" + json, "Here is your plan:\n```json\n" + json + "\n```" })
            Assert.Equal(70, AiMapPlan.Parse(response, MaterialIds).BaseHeight);
    }

    [Fact]
    public void Parse_normalizes_coordinates_radii_amounts_and_base_height()
    {
        var source = new AiMapPlan
        {
            BaseHeight = 999,
            Features =
            [
                new() { Type = " HILL ", X = -9, Y = 99, X2 = 99, Y2 = -9, Radius = 99, Amount = 999 },
                new() { Type = "lake", X = 99, Y = -9, X2 = -9, Y2 = 99, Radius = 0, Amount = -9 },
                new() { Type = "plateau", Radius = -9 },
                new() { Type = "hill", Radius = 1 },
            ],
        };
        AiMapPlan plan = AiMapPlan.Parse(JsonSerializer.Serialize(source), MaterialIds);
        Assert.Equal(255, plan.BaseHeight);
        AiMapFeature first = plan.Features[0], second = plan.Features[1];
        Assert.Equal("hill", first.Type);
        Assert.Equal((0, 63, (int?)63, (int?)0), (first.X, first.Y, first.X2, first.Y2));
        Assert.Equal((63, 0, (int?)0, (int?)63), (second.X, second.Y, second.X2, second.Y2));
        Assert.Equal(255, first.Amount);
        Assert.Equal(0, second.Amount);
        Assert.Equal(new[] { 32, 4, 4, 1 }, plan.Features.Select(feature => feature.Radius).ToArray());
        Assert.Equal(0, AiMapPlan.Parse("{\"baseHeight\":-9}", MaterialIds).BaseHeight);
    }

    [Fact]
    public void Parse_drops_unknown_features_and_materials_and_preserves_canonical_ids()
    {
        AiMapPlan plan = AiMapPlan.Parse("""
            {"baseMaterial":" sand ","features":[
              {"type":"volcano"},
              {"type":"material","material":"Unknown"},
              {"type":"material"},
              {"type":" MATERIAL ","material":" grass "},
              {"type":"hill","material":"SAND"}
            ]}
            """, MaterialIds);
        Assert.Equal("Sand", plan.BaseMaterial);
        Assert.Equal(2, plan.Features.Count);
        Assert.Equal("material", plan.Features[0].Type);
        Assert.Equal("Grass", plan.Features[0].Material);
        Assert.Equal("Sand", plan.Features[1].Material);
        Assert.Null(AiMapPlan.Parse("{\"baseHeight\":70,\"baseMaterial\":\"Unknown\"}", MaterialIds).BaseMaterial);
    }

    [Fact]
    public void Parse_truncates_features_to_MaxFeatures()
    {
        var source = new AiMapPlan
        {
            Features = Enumerable.Range(0, AiMapPlan.MaxFeatures + 10)
                .Select(index => new AiMapFeature { Type = "hill", X = index, Radius = 2 }).ToList(),
        };
        AiMapPlan plan = AiMapPlan.Parse(JsonSerializer.Serialize(source), MaterialIds);
        Assert.Equal(AiMapPlan.MaxFeatures, plan.Features.Count);
        Assert.Equal(Enumerable.Range(0, AiMapPlan.MaxFeatures).ToArray(), plan.Features.Select(feature => feature.X).ToArray());
    }

    [Fact]
    public void Parse_rejects_empty_non_json_malformed_and_unusable_plans()
    {
        foreach (string response in new[] { "", " \r\n", "not JSON", "{invalid}", "{\"features\":[}", "{}", "{\"features\":[]}", "{\"baseMaterial\":\"Unknown\",\"features\":[{\"type\":\"unknown\"},{\"type\":\"material\",\"material\":\"Unknown\"}]}" })
            Assert.Throws<InvalidDataException>(() => AiMapPlan.Parse(response, MaterialIds));
    }

    [Fact]
    public void JsonSchema_serializes_feature_types_and_material_ids()
    {
        using JsonDocument document = JsonDocument.Parse(JsonSerializer.Serialize(AiMapPlan.JsonSchema(MaterialIds)));
        Assert.Equal(JsonValueKind.Object, document.RootElement.ValueKind);
        JsonElement properties = document.RootElement.GetProperty("properties").GetProperty("features")
            .GetProperty("items").GetProperty("properties");
        Assert.Equal(AiMapPlan.FeatureTypes, properties.GetProperty("type").GetProperty("enum").EnumerateArray().Select(item => item.GetString()).ToArray());
        Assert.Equal(MaterialIds, properties.GetProperty("material").GetProperty("enum").EnumerateArray().Select(item => item.GetString()).ToArray());
    }

    [Fact]
    public void Apply_hill_raises_center_by_amount_and_preserves_far_corner()
    {
        TerrainHeightEditSession session = FreshSession();
        AiMapApplyResult result = Apply(new() { Features = [new() { Type = "hill", X = 32, Y = 32, Radius = 6, Amount = 50 }] }, session);
        Assert.InRange(session.Heights[VertexIndex(32, 32)] - 100, 48, 52);
        Assert.Equal(100, session.Heights[0]);
        Assert.True(result.HeightSamplesChanged > 0);
    }

    [Fact]
    public void Apply_lake_carves_below_water_and_preserves_far_point()
    {
        TerrainHeightEditSession session = FreshSession();
        Apply(new() { Features = [new() { Type = "lake", X = 10, Y = 10, Radius = 5, Amount = 12 }] }, session);
        Assert.InRange(session.Heights[VertexIndex(10, 10)], 0, 48);
        Assert.Equal(100, session.Heights[VertexIndex(60, 60)]);
    }

    [Fact]
    public void Apply_river_carves_entire_segment_below_water()
    {
        TerrainHeightEditSession session = FreshSession();
        Apply(new() { Features = [new() { Type = "river", X = 5, Y = 40, X2 = 60, Y2 = 40, Radius = 2 }] }, session);
        for (int x = 5; x <= 60; x++)
            Assert.True(session.Heights[VertexIndex(x, 40)] < 60, $"River at x={x} must be below water.");
        Assert.Equal(100, session.Heights[0]);
    }

    [Fact]
    public void Apply_plateau_sets_center_to_absolute_amount()
    {
        TerrainHeightEditSession session = FreshSession();
        Apply(new() { Features = [new() { Type = "plateau", X = 32, Y = 32, Radius = 6, Amount = 175 }] }, session);
        Assert.Equal(175, session.Heights[VertexIndex(32, 32)]);
        Assert.Equal(100, session.Heights[0]);
    }

    [Fact]
    public void Apply_base_height_without_features_sets_every_vertex()
    {
        TerrainHeightEditSession session = FreshSession();
        AiMapApplyResult result = Apply(new() { BaseHeight = 70 }, session);
        Assert.All(session.Heights, height => Assert.Equal(70, height));
        Assert.Equal(257 * 257, result.HeightSamplesChanged);
    }

    [Fact]
    public void Apply_material_callbacks_use_tile_centers_and_count_rejections()
    {
        TerrainHeightEditSession session = FreshSession();
        var calls = new List<(string Id, float X, float Y, float Radius)>();
        var plan = new AiMapPlan
        {
            BaseMaterial = "Grass",
            Features = [new() { Type = "material", Material = "Sand", X = 10, Y = 20, Radius = 5 }],
        };
        AiMapApplyResult result = AiMapPlanApplier.Apply(plan, session, 64, 60, (id, x, y, radius) =>
        {
            calls.Add((id, x, y, radius));
            return id != "Sand";
        });
        Assert.Equal(new[] { ("Grass", 32f, 32f, 64f), ("Sand", 10.5f, 20.5f, 5f) }, calls.ToArray());
        Assert.Equal(2, result.MaterialStrokes);
        Assert.Equal(1, result.RejectedMaterialStrokes);
        Assert.Equal(0, result.HeightSamplesChanged);
        Assert.All(session.Heights, height => Assert.Equal(100, height));
    }

    [Fact]
    public void Apply_blocked_line_sets_collision_and_passable_clears_it()
    {
        TerrainHeightEditSession session = FreshSession();
        var feature = new AiMapFeature { Type = "blocked", X = 0, Y = 0, X2 = 10, Y2 = 0, Radius = 1 };
        var plan = new AiMapPlan { Features = [feature] };
        AiMapApplyResult blocked = Apply(plan, session);
        Assert.True(blocked.CollisionPixelsChanged > 0);
        for (int x = 0; x <= 10 * 4 + 2; x++) Assert.True(session.Collision![x] > 0, $"Collision row at x={x} must be blocked.");
        Assert.Equal(0, session.Collision![255 * 256 + 255]);
        feature.Type = "passable";
        AiMapApplyResult cleared = Apply(plan, session);
        Assert.True(cleared.CollisionPixelsChanged > 0);
        Assert.All(session.Collision!, value => Assert.Equal(0, value));
    }

    [Fact]
    public void Apply_same_plan_to_fresh_sessions_is_deterministic()
    {
        AiMapPlan plan = MixedPlan();
        TerrainHeightEditSession first = FreshSession(), second = FreshSession();
        Apply(plan, first);
        Apply(plan, second);
        Assert.True(first.HeightsDirty);
        Assert.Equal(first.Heights.ToArray(), second.Heights.ToArray());
    }

    [Fact]
    public void Apply_leaves_changes_pending_as_one_undoable_stroke()
    {
        TerrainHeightEditSession session = FreshSession();
        byte[] original = session.Heights.ToArray();
        Apply(MixedPlan(), session);
        Assert.True(session.HeightsDirty);
        Assert.True(session.CommitStroke()); // Fails if Apply already committed internally.
        Assert.False(session.CommitStroke());
        Assert.NotNull(session.Undo());
        Assert.Equal(original, session.Heights.ToArray());
        Assert.False(session.HeightsDirty);
        Assert.False(session.CanUndo);
        Assert.Null(session.Undo());
    }

    [Fact]
    public async Task ListModelsAsync_parses_models_and_sorts_names()
    {
        using var handler = new FakeHandler(request =>
        {
            Assert.Equal(HttpMethod.Get, request.Method);
            Assert.Equal("/api/tags", request.RequestUri!.AbsolutePath);
            return Task.FromResult(JsonResponse("{\"models\":[{\"name\":\"b\"},{\"name\":\"a\"}]}"));
        });
        using var planner = new OllamaMapPlanner("http://ollama.test", handler);
        Assert.Equal(new[] { "a", "b" }, await planner.ListModelsAsync(CancellationToken.None));
        Assert.Equal(1, handler.RequestCount);
    }

    [Fact]
    public async Task GeneratePlanAsync_posts_schema_and_messages_and_parses_response()
    {
        const string planJson = "{\"baseHeight\":70,\"baseMaterial\":\"grass\",\"features\":[{\"type\":\"HILL\",\"x\":32,\"y\":32,\"radius\":6,\"amount\":50}]}";
        using var handler = new FakeHandler(async request =>
        {
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal(new Uri("http://ollama.test/api/chat"), request.RequestUri);
            using JsonDocument body = JsonDocument.Parse(await request.Content!.ReadAsStringAsync());
            JsonElement root = body.RootElement;
            Assert.Equal("test-model", root.GetProperty("model").GetString());
            Assert.False(root.GetProperty("stream").GetBoolean());
            Assert.Equal(JsonValueKind.Object, root.GetProperty("format").ValueKind);
            JsonElement messages = root.GetProperty("messages");
            Assert.Equal(2, messages.GetArrayLength());
            Assert.Equal("system", messages[0].GetProperty("role").GetString());
            foreach (string id in MaterialIds) Assert.Contains(id, messages[0].GetProperty("content").GetString()!);
            Assert.Equal("user", messages[1].GetProperty("role").GetString());
            Assert.Equal("A green valley", messages[1].GetProperty("content").GetString());
            return JsonResponse(JsonSerializer.Serialize(new { message = new { content = planJson } }));
        });
        using var planner = new OllamaMapPlanner("http://ollama.test/", handler);
        (AiMapPlan plan, string raw) = await planner.GeneratePlanAsync("test-model", " \nA green valley\t ", Materials, 60, CancellationToken.None);
        Assert.Equal(planJson, raw);
        Assert.Equal(70, plan.BaseHeight);
        Assert.Equal("Grass", plan.BaseMaterial);
        AiMapFeature feature = Assert.Single(plan.Features);
        Assert.Equal("hill", feature.Type);
        Assert.Equal((32, 32, 6, (int?)50), (feature.X, feature.Y, feature.Radius, feature.Amount));
        Assert.Equal(1, handler.RequestCount);
    }

    [Fact]
    public async Task ListModelsAsync_throws_for_non_success_status()
    {
        using var handler = new FakeHandler(_ => Task.FromResult(JsonResponse("{}", HttpStatusCode.ServiceUnavailable)));
        using var planner = new OllamaMapPlanner("http://ollama.test", handler);
        await Assert.ThrowsAsync<HttpRequestException>(() => planner.ListModelsAsync(CancellationToken.None));
    }

    [Fact]
    public async Task GeneratePlanAsync_throws_for_non_success_status()
    {
        using var handler = new FakeHandler(_ => Task.FromResult(JsonResponse("{}", HttpStatusCode.BadRequest)));
        using var planner = new OllamaMapPlanner("http://ollama.test", handler);
        await Assert.ThrowsAsync<HttpRequestException>(() => planner.GeneratePlanAsync("test-model", "A valley", Materials, 60, CancellationToken.None));
    }

    [Fact]
    public async Task GeneratePlanAsync_rejects_blank_model_or_description_before_http()
    {
        using var handler = new FakeHandler(_ => Task.FromResult(JsonResponse("{}")));
        using var planner = new OllamaMapPlanner("http://ollama.test", handler);
        foreach (string blank in new[] { "", " \t\n" })
        {
            ArgumentException modelError = await Assert.ThrowsAsync<ArgumentException>(() => planner.GeneratePlanAsync(blank, "A valley", Materials, 60, CancellationToken.None));
            Assert.Equal("model", modelError.ParamName);
            ArgumentException descriptionError = await Assert.ThrowsAsync<ArgumentException>(() => planner.GeneratePlanAsync("test-model", blank, Materials, 60, CancellationToken.None));
            Assert.Equal("description", descriptionError.ParamName);
        }
        Assert.Equal(0, handler.RequestCount);
    }

    [Fact]
    public void BuildSystemPrompt_mentions_water_level_and_every_material_id()
    {
        string prompt = OllamaMapPlanner.BuildSystemPrompt(Materials, 73);
        Assert.Contains("Ground below 73 becomes water", prompt);
        foreach (string id in MaterialIds) Assert.Contains(id, prompt);
    }

    private static TerrainHeightEditSession FreshSession() =>
        new(257, Enumerable.Repeat((byte)100, 257 * 257).ToArray(), null, 256, new byte[256 * 256]);

    private static int VertexIndex(int x, int y) => (y * 4 + 2) * 257 + x * 4 + 2;

    private static AiMapApplyResult Apply(AiMapPlan plan, TerrainHeightEditSession session) =>
        AiMapPlanApplier.Apply(plan, session, 64, 60, (_, _, _, _) => true);

    private static AiMapPlan MixedPlan() => new()
    {
        Features =
        [
            new() { Type = "hill", X = 32, Y = 32, Radius = 6, Amount = 50 },
            new() { Type = "lake", X = 10, Y = 10, Radius = 5, Amount = 12 },
            new() { Type = "river", X = 5, Y = 40, X2 = 60, Y2 = 40, Radius = 2 },
            new() { Type = "plateau", X = 48, Y = 16, Radius = 4, Amount = 140 },
        ],
    };

    private static HttpResponseMessage JsonResponse(string json, HttpStatusCode status = HttpStatusCode.OK) =>
        new(status) { Content = new StringContent(json, Encoding.UTF8, "application/json") };

    private sealed class FakeHandler(Func<HttpRequestMessage, Task<HttpResponseMessage>> respond) : HttpMessageHandler
    {
        public int RequestCount { get; private set; }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            RequestCount++;
            return respond(request);
        }
    }
}
