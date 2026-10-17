using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed class MapDesignValidatorTests
{
    [Fact]
    public void Objects_outside_map_bounds_are_detected()
    {
        var id1 = Guid.NewGuid();
        var id2 = Guid.NewGuid();
        var id3 = Guid.NewGuid();

        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", -10f, 5000f, 0) { Id = id1 },
            new("GER_INF00", 8000f, 16385f, 0, Count: 10) { Id = id2 },
            new("ROM_HAU00", float.NaN, 5000f, 1) { Id = id3 }
        };

        var request = new MapDesignValidationRequest(
            Objects: spawns,
            Events: Array.Empty<ScenarioEvent>());

        var issues = MapDesignValidator.Validate(request);

        var boundsIssues = issues.Where(i => i.Code == "bounds").ToList();
        Assert.Equal(3, boundsIssues.Count);
        Assert.Contains(boundsIssues, i => i.ObjectAlias == "GER_HAU00" && i.ObjectId == id1);
        Assert.Contains(boundsIssues, i => i.ObjectAlias == "GER_INF00" && i.ObjectId == id2);
        Assert.Contains(boundsIssues, i => i.ObjectAlias == "ROM_HAU00" && i.ObjectId == id3);
    }

    [Fact]
    public void Placed_object_on_water_is_detected()
    {
        // 257x257 height grid where height byte is 10, step = 4 => ground height = 40.0.
        // Water level = 50.0. Ground height 40.0 < 50.0 => submerged.
        int heightSize = 257;
        var heights = new byte[heightSize * heightSize];
        Array.Fill(heights, (byte)10);

        var objId = Guid.NewGuid();
        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", 8000f, 8000f, 0) { Id = objId },
            new("GER_INF00", 8100f, 8000f, 0, Count: 10)
        };

        var request = new MapDesignValidationRequest(
            Objects: spawns,
            Events: [new ScenarioEvent("TestEvent") { Actions = [new ScenarioAction(ScenarioActionKind.Victory), new ScenarioAction(ScenarioActionKind.Defeat)] }],
            Heights: heights,
            HeightSize: heightSize,
            HeightStep: 4.0f,
            WaterLevel: 50.0f);

        var issues = MapDesignValidator.Validate(request);

        var waterIssues = issues.Where(i => i.Code == "water").ToList();
        Assert.Equal(2, waterIssues.Count);
        Assert.Contains(waterIssues, i => i.ObjectId == objId && i.Chinese.Contains("位於水下") && i.Chinese.Contains("40.0"));
    }

    [Fact]
    public void Placed_object_on_impassable_cell_is_detected()
    {
        // 256x256 collision grid where (128, 128) is blocked (255)
        int colSize = 256;
        var collision = new byte[colSize * colSize];
        // 8000 / 16384 * 256 = 125
        int targetX = (int)(8000f / 16384f * colSize);
        int targetZ = (int)(8000f / 16384f * colSize);
        collision[targetZ * colSize + targetX] = 255;

        var objId = Guid.NewGuid();
        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", 8000f, 8000f, 0) { Id = objId },
            new("GER_INF00", 2000f, 2000f, 0, Count: 10) // on clear cell
        };

        var request = new MapDesignValidationRequest(
            Objects: spawns,
            Events: [new ScenarioEvent("TestEvent") { Actions = [new ScenarioAction(ScenarioActionKind.Victory), new ScenarioAction(ScenarioActionKind.Defeat)] }],
            Collision: collision,
            CollisionSize: colSize);

        var issues = MapDesignValidator.Validate(request);

        var blockIssues = issues.Where(i => i.Code == "impassable").ToList();
        Assert.Single(blockIssues);
        Assert.Equal(objId, blockIssues[0].ObjectId);
        Assert.Contains("阻擋格", blockIssues[0].Chinese);
    }

    [Fact]
    public void Player_team_without_main_house_or_units_warns_appropriately()
    {
        // Team 0 has a tower but no main house and no units
        var spawns = new List<ScenarioSpawn>
        {
            new("BauGerTur00_Turm", 5000f, 5000f, 0)
        };

        var request = new MapDesignValidationRequest(
            Objects: spawns,
            Events: Array.Empty<ScenarioEvent>());

        var issues = MapDesignValidator.Validate(request);

        Assert.Contains(issues, i => i.Code == "team-no-main-house" && i.Team == 0);
        Assert.Contains(issues, i => i.Code == "team-no-units" && i.Team == 0);
    }

    [Fact]
    public void Event_spawned_units_satisfy_team_units_requirement()
    {
        // Team 0 has main house, but no placed units; however, an enabled event spawns units for team 0
        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", 5000f, 5000f, 0)
        };

        var ev = new ScenarioEvent("SpawnReinforcements", Enabled: true)
        {
            Actions = [new ScenarioAction(ScenarioActionKind.SpawnUnit, Team: 0, Count: 10)]
        };

        var request = new MapDesignValidationRequest(
            Objects: spawns,
            Events: [ev]);

        var issues = MapDesignValidator.Validate(request);

        Assert.DoesNotContain(issues, i => i.Code == "team-no-units" && i.Team == 0);
        Assert.DoesNotContain(issues, i => i.Code == "team-no-main-house" && i.Team == 0);
    }

    [Fact]
    public void Scenario_missing_victory_and_defeat_events_is_detected()
    {
        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", 5000f, 5000f, 0),
            new("GER_INF00", 5100f, 5000f, 0, Count: 10)
        };

        // No events
        var request1 = new MapDesignValidationRequest(Objects: spawns, Events: Array.Empty<ScenarioEvent>());
        var issues1 = MapDesignValidator.Validate(request1);
        Assert.Contains(issues1, i => i.Code == "scenario-no-win-loss");

        // Victory only
        var victoryOnly = new ScenarioEvent("VictoryEvent") { Actions = [new ScenarioAction(ScenarioActionKind.Victory)] };
        var request2 = new MapDesignValidationRequest(Objects: spawns, Events: [victoryOnly]);
        var issues2 = MapDesignValidator.Validate(request2);
        Assert.Contains(issues2, i => i.Code == "scenario-no-defeat");
        Assert.DoesNotContain(issues2, i => i.Code == "scenario-no-victory");

        // Defeat only
        var defeatOnly = new ScenarioEvent("DefeatEvent") { Actions = [new ScenarioAction(ScenarioActionKind.Defeat)] };
        var request3 = new MapDesignValidationRequest(Objects: spawns, Events: [defeatOnly]);
        var issues3 = MapDesignValidator.Validate(request3);
        Assert.Contains(issues3, i => i.Code == "scenario-no-victory");
        Assert.DoesNotContain(issues3, i => i.Code == "scenario-no-defeat");

        // Both present
        var both = new ScenarioEvent("Both") { Actions = [new ScenarioAction(ScenarioActionKind.Victory), new ScenarioAction(ScenarioActionKind.Defeat)] };
        var request4 = new MapDesignValidationRequest(Objects: spawns, Events: [both]);
        var issues4 = MapDesignValidator.Validate(request4);
        Assert.DoesNotContain(issues4, i => i.Code.StartsWith("scenario-no-", StringComparison.Ordinal));
    }

    [Fact]
    public void Multi_team_design_validations_are_independent()
    {
        // Team 0 has complete setup; Team 1 has only a main house but no units
        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", 2000f, 2000f, 0),
            new("GER_INF00", 2100f, 2000f, 0, Count: 10),
            new("ROM_HAU00", 12000f, 12000f, 1)
        };

        var events = new List<ScenarioEvent>
        {
            new("EndConditions") { Actions = [new ScenarioAction(ScenarioActionKind.Victory), new ScenarioAction(ScenarioActionKind.Defeat)] }
        };

        var request = new MapDesignValidationRequest(Objects: spawns, Events: events);
        var issues = MapDesignValidator.Validate(request);

        Assert.DoesNotContain(issues, i => i.Team == 0);
        Assert.Contains(issues, i => i.Code == "team-no-units" && i.Team == 1);
        Assert.DoesNotContain(issues, i => i.Code == "team-no-main-house" && i.Team == 1);
    }

    [Fact]
    public void IsMainHouse_identifies_all_four_tribes_and_namedefs()
    {
        Assert.True(MapDesignValidator.IsMainHouse("GER_HAU00"));
        Assert.True(MapDesignValidator.IsMainHouse("ROM_HAU00"));
        Assert.True(MapDesignValidator.IsMainHouse("KEL_HAU00"));
        Assert.True(MapDesignValidator.IsMainHouse("HUN_HAU00"));
        Assert.True(MapDesignValidator.IsMainHouse("BauGerHau00_Haupthaus"));
        Assert.True(MapDesignValidator.IsMainHouse("BauRomHau00_Hauptzelt"));
        Assert.True(MapDesignValidator.IsMainHouse("BauKelHau00_Haupthaus"));
        Assert.True(MapDesignValidator.IsMainHouse("BauHunHau00_Haupthaus"));

        Assert.False(MapDesignValidator.IsMainHouse("GER_INF00"));
        Assert.False(MapDesignValidator.IsMainHouse("BauGerTur00_Turm"));
        Assert.False(MapDesignValidator.IsMainHouse("BauRomWaf00_Waffenschmiede"));
        Assert.False(MapDesignValidator.IsMainHouse(null));
        Assert.False(MapDesignValidator.IsMainHouse(""));
    }

    [Fact]
    public void Fixture_193_192_maps_world_coordinates_to_actual_layer_dimensions_correctly()
    {
        // 193x193 height and 192x192 collision (dimension = 48)
        int colSize = 192;
        int heightSize = 193;
        var collision = new byte[colSize * colSize];
        var heights = new byte[heightSize * heightSize];
        Array.Fill(heights, (byte)50); // 50 * 4 = 200 > waterLevel 50

        // Set cell at (cx=96, cz=96) to blocked
        // World coordinate: normX = 96 / 192 = 0.5 -> 0.5 * 16384 = 8192
        collision[96 * colSize + 96] = 255;

        var blockedId = Guid.NewGuid();
        var clearId = Guid.NewGuid();
        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", 8192f, 8192f, 0) { Id = blockedId },
            new("GER_INF00", 2000f, 2000f, 0, Count: 10) { Id = clearId }
        };

        var request = new MapDesignValidationRequest(
            Objects: spawns,
            Events: [new ScenarioEvent("E") { Actions = [new ScenarioAction(ScenarioActionKind.Victory), new ScenarioAction(ScenarioActionKind.Defeat)] }],
            Collision: collision,
            CollisionSize: colSize,
            Heights: heights,
            HeightSize: heightSize,
            HeightStep: 4.0f,
            WaterLevel: 50.0f);

        var issues = MapDesignValidator.Validate(request);

        var impassable = issues.Where(i => i.Code == "impassable").ToList();
        Assert.Single(impassable);
        Assert.Equal(blockedId, impassable[0].ObjectId);
        Assert.DoesNotContain(issues, i => i.Code == "water");
    }

    [Fact]
    public void Bilinear_water_detection_matches_TerrainHeightEditSession_logic()
    {
        // Height grid 193x193.
        // Place an object at fractional vertex coordinates between a low and high vertex.
        // Vertex (96, 96)=10, (97, 96)=20, (96, 97)=10, (97, 97)=20.
        // At gx=96.5, gz=96.0, bilinear height is 15.0.
        // HeightStep = 4.0 -> groundHeight = 60.0.
        // With waterLevel = 62.0:
        // Bilinear ground height 60.0 < 62.0 -> submerged!
        // (Nearest neighbor with MathF.Round(96.5)=97 would sample vertex 97: 20 * 4 = 80 > 62 -> would falsely report dry land).
        int heightSize = 193;
        var heights = new byte[heightSize * heightSize];
        Array.Fill(heights, (byte)50);
        heights[96 * heightSize + 96] = 10;
        heights[96 * heightSize + 97] = 20;
        heights[97 * heightSize + 96] = 10;
        heights[97 * heightSize + 97] = 20;

        float normX = 96.5f / 192f;
        float normZ = 96.0f / 192f;
        float worldX = normX * 16384f;
        float worldZ = normZ * 16384f;

        var subObjId = Guid.NewGuid();
        var dryObjId = Guid.NewGuid();
        // Dry object placed at integer vertex 97 (height 20 * 4 = 80 >= 62)
        float dryWorldX = (97.0f / 192f) * 16384f;
        float dryWorldZ = (96.0f / 192f) * 16384f;

        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", worldX, worldZ, 0) { Id = subObjId },
            new("GER_INF00", dryWorldX, dryWorldZ, 0, Count: 10) { Id = dryObjId }
        };

        var request = new MapDesignValidationRequest(
            Objects: spawns,
            Events: [new ScenarioEvent("E") { Actions = [new ScenarioAction(ScenarioActionKind.Victory), new ScenarioAction(ScenarioActionKind.Defeat)] }],
            Heights: heights,
            HeightSize: heightSize,
            HeightStep: 4.0f,
            WaterLevel: 62.0f);

        var issues = MapDesignValidator.Validate(request);

        var waterIssues = issues.Where(i => i.Code == "water").ToList();
        Assert.Single(waterIssues);
        Assert.Equal(subObjId, waterIssues[0].ObjectId);
        Assert.Contains("60.0", waterIssues[0].Chinese);
    }

    [Fact]
    public void Inconsistent_layer_sizes_are_rejected_and_skipped_gracefully()
    {
        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", 5000f, 5000f, 0),
            new("GER_INF00", 5100f, 5000f, 0, Count: 10)
        };
        var events = new List<ScenarioEvent>
        {
            new("E") { Actions = [new ScenarioAction(ScenarioActionKind.Victory), new ScenarioAction(ScenarioActionKind.Defeat)] }
        };

        // 1. Buffer count does not match declared size
        var badCollision = new byte[100];
        var req1 = new MapDesignValidationRequest(spawns, events, Collision: badCollision, CollisionSize: 192);
        var issues1 = MapDesignValidator.Validate(req1);
        Assert.DoesNotContain(issues1, i => i.Code == "impassable");

        // 2. Heights buffer count does not match declared size
        var badHeights = new byte[100];
        var req2 = new MapDesignValidationRequest(spawns, events, Heights: badHeights, HeightSize: 193, WaterLevel: 500f);
        var issues2 = MapDesignValidator.Validate(req2);
        Assert.DoesNotContain(issues2, i => i.Code == "water");

        // 3. Mutually inconsistent sizes: Collision 192 vs Heights 257 (192 != 257 - 1)
        var col192 = new byte[192 * 192];
        Array.Fill(col192, (byte)255); // all blocked
        var height257 = new byte[257 * 257];
        Array.Fill(height257, (byte)0); // all underwater
        var req3 = new MapDesignValidationRequest(spawns, events,
            Collision: col192, CollisionSize: 192,
            Heights: height257, HeightSize: 257,
            HeightStep: 4.0f, WaterLevel: 50.0f);

        // Inconsistent sizes should be gracefully rejected/skipped without throwing exceptions or reporting false issues
        var issues3 = MapDesignValidator.Validate(req3);
        Assert.DoesNotContain(issues3, i => i.Code == "impassable");
        Assert.DoesNotContain(issues3, i => i.Code == "water");
    }

    [Fact]
    public void Real_game_map_layer_format_verification()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_TERRAIN_READONLY_PATH") ?? Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrEmpty(game) || !Directory.Exists(game))
        {
            // Fallback to standard install directory if present
            string defaultPath = @"C:\Program Files (x86)\Against Rome";
            if (Directory.Exists(defaultPath)) game = defaultPath;
            else return;
        }

        string mapDir = Path.Combine(game, "MAPS", "MP_000");
        if (!Directory.Exists(mapDir)) return;

        string colPath = Path.Combine(mapDir, "collision.bmp");
        string bodPath = Path.Combine(mapDir, "boden.bmp");
        string iniPath = Path.Combine(mapDir, "boden.ini");

        Assert.True(File.Exists(colPath), "collision.bmp must exist on real map");
        Assert.True(File.Exists(bodPath), "boden.bmp must exist on real map");
        Assert.True(File.Exists(iniPath), "boden.ini must exist on real map");

        var colLayer = TerrainLayerFiles.Read(colPath);
        Assert.NotNull(colLayer);
        Assert.Equal(256, colLayer.Width);
        Assert.Equal(256, colLayer.Height);

        var bodLayer = TerrainLayerFiles.Read(bodPath);
        Assert.NotNull(bodLayer);
        Assert.Equal(257, bodLayer.Width);
        Assert.Equal(257, bodLayer.Height);

        var ini = BodenIniDocument.Load(iniPath);
        string? waterStr = ini.GetValue("Waterlevel");
        Assert.NotNull(waterStr);
        Assert.True(float.TryParse(waterStr, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out float waterLevel));

        // Test running validator on objects using real layers
        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", 8000f, 8000f, 0),
            new("GER_INF00", 8100f, 8000f, 0, Count: 10)
        };

        var events = new List<ScenarioEvent>
        {
            new("Events") { Actions = [new ScenarioAction(ScenarioActionKind.Victory), new ScenarioAction(ScenarioActionKind.Defeat)] }
        };

        var request = new MapDesignValidationRequest(
            Objects: spawns,
            Events: events,
            Collision: colLayer.Green,
            CollisionSize: colLayer.Width,
            Heights: bodLayer.Green,
            HeightSize: bodLayer.Width,
            HeightStep: 4.0f,
            WaterLevel: waterLevel);

        var issues = MapDesignValidator.Validate(request);
        Assert.NotNull(issues);
    }

    [Fact]
    public void Real_TUTOR_01_read_only_layer_verification()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_TERRAIN_READONLY_PATH") ?? Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (string.IsNullOrEmpty(game) || !Directory.Exists(game))
        {
            string defaultPath = @"C:\Program Files (x86)\Against Rome";
            if (Directory.Exists(defaultPath)) game = defaultPath;
            else return;
        }

        string mapDir = Path.Combine(game, "MAPS", "TUTOR_01");
        if (!Directory.Exists(mapDir)) return;

        string colPath = Path.Combine(mapDir, "collision.bmp");
        string bodPath = Path.Combine(mapDir, "boden.bmp");
        string iniPath = Path.Combine(mapDir, "boden.ini");

        Assert.True(File.Exists(colPath), "collision.bmp must exist in TUTOR_01");
        Assert.True(File.Exists(bodPath), "boden.bmp must exist in TUTOR_01");
        Assert.True(File.Exists(iniPath), "boden.ini must exist in TUTOR_01");

        byte[] bodenBytesBefore = File.ReadAllBytes(bodPath);
        byte[] collisionBytesBefore = File.ReadAllBytes(colPath);

        var bodLayer = TerrainLayerFiles.Read(bodPath);
        var colLayer = TerrainLayerFiles.Read(colPath);

        Assert.NotNull(bodLayer);
        Assert.NotNull(colLayer);
        // TUTOR_01 has dimension 48: boden is 193x193, collision is 192x192
        Assert.Equal(193, bodLayer.Width);
        Assert.Equal(193, bodLayer.Height);
        Assert.Equal(192, colLayer.Width);
        Assert.Equal(192, colLayer.Height);
        Assert.Equal(bodLayer.Width - 1, colLayer.Width);

        var ini = BodenIniDocument.Load(iniPath);
        string? waterStr = ini.GetValue("Waterlevel");
        Assert.NotNull(waterStr);
        Assert.True(float.TryParse(waterStr, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out float waterLevel));
        Assert.Equal(110f, waterLevel);

        string? stepStr = ini.GetValue("Heightmapstep");
        Assert.NotNull(stepStr);
        Assert.True(float.TryParse(stepStr, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out float heightStep));
        Assert.Equal(4f, heightStep);

        var spawns = new List<ScenarioSpawn>
        {
            new("GER_HAU00", 8000f, 8000f, 0),
            new("GER_INF00", 8100f, 8000f, 0, Count: 10)
        };
        var events = new List<ScenarioEvent>
        {
            new("Events") { Actions = [new ScenarioAction(ScenarioActionKind.Victory), new ScenarioAction(ScenarioActionKind.Defeat)] }
        };

        var request = new MapDesignValidationRequest(
            Objects: spawns,
            Events: events,
            Collision: colLayer.Green,
            CollisionSize: colLayer.Width,
            Heights: bodLayer.Green,
            HeightSize: bodLayer.Width,
            HeightStep: heightStep,
            WaterLevel: waterLevel);

        var issues = MapDesignValidator.Validate(request);
        Assert.NotNull(issues);

        // Strict read-only guarantee: source files must never be modified
        Assert.Equal(bodenBytesBefore, File.ReadAllBytes(bodPath));
        Assert.Equal(collisionBytesBefore, File.ReadAllBytes(colPath));
    }
}
