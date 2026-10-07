using System.Globalization;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class MapEditorObjectPlacementTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "AgainstRomeObjectPlacementTests_" + Guid.NewGuid().ToString("N"));

    [Fact]
    public void Build_Collects_distinct_types_counts_occurrences_and_prefers_onload_templates()
    {
        WriteSdl("ENDL_000", "first.sdl", Settlement +
            Object(0, "BauGerHouse", "10", "onload=0\r\nvariant=ordinary\r\n") +
            Object(1, "BauGerHouse", "11", "onload=1\r\nvariant=preferred\r\n") +
            Object(2, "VerHunIcoSoldier", "20") + Object(3, "FigKelLeader", "30"));
        WriteSdl("HIST_001", "second.sdl", Settlement +
            Object(0, "baugerhouse", "12", "onload=0\r\nvariant=later\r\n") +
            Object(1, "BauGerHouse", "13", "onload=1\r\nvariant=later-preferred\r\n") +
            Object(2, "BauRomTower", "40") + Object(3, "FXFire", "50") +
            Object(4, "Tree", "60") + Object(5, "VerRomSoldier", "70") +
            "[object0006]\r\ndef=99\r\n" +
            "[object0007]\r\nnamedef=MissingDefinition\r\n" +
            Object(8, "InvalidDefinition", "oops") + Object(9, "NegativeDefinition", "-1") +
            Object(10, "", "99"));
        string customPath = WriteSdl("ENDL_005", "custom.sdl", Settlement +
            Object(0, "CustomOnly", "100") + Object(1, "BauGerHouse", "999", "onload=1\r\n"));
        File.WriteAllText(Path.Combine(Path.GetDirectoryName(customPath)!, CustomMapManifest.MarkerFileName), "{}");

        IReadOnlyList<SdlObjectType> catalog = SdlObjectCatalog.Build(_root);

        Assert.Equal(7, catalog.Count);
        AssertType(catalog, "BauGerHouse", 11, SdlObjectCategory.Building, "Ger", 4);
        AssertType(catalog, "VerHunIcoSoldier", 20, SdlObjectCategory.UnitGroup, "Hun", 1);
        AssertType(catalog, "FigKelLeader", 30, SdlObjectCategory.Figure, "Kel", 1);
        AssertType(catalog, "BauRomTower", 40, SdlObjectCategory.Building, "Rom", 1);
        AssertType(catalog, "FXFire", 50, SdlObjectCategory.Effect, "", 1);
        AssertType(catalog, "Tree", 60, SdlObjectCategory.Other, "", 1);
        AssertType(catalog, "VerRomSoldier", 70, SdlObjectCategory.Other, "Rom", 1);
        Assert.Equal("preferred", catalog.Single(type => type.NameDef == "BauGerHouse").TemplateFields["variant"]);
    }

    [Fact]
    public void Build_Returns_empty_when_MAPS_is_missing()
    {
        Directory.CreateDirectory(_root);
        Assert.False(Directory.Exists(Path.Combine(_root, "MAPS")));
        Assert.Empty(SdlObjectCatalog.Build(_root));
    }

    [Fact]
    public void Render_Writes_absolute_objects_and_preserves_other_template_fields()
    {
        SdlObjectType type = CreateType(SdlObjectCategory.Building);
        SdlPlacedObject[] objects =
        [
            new(type, 12.25f, -3.5f, 1024, -1, 45.5f, 25),
            new(type, 0, 4, 9.75f, 15, -90, 50),
        ];
        CultureInfo previousCulture = CultureInfo.CurrentCulture;
        string rendered;
        try
        {
            CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("fr-FR");
            rendered = SdlPlacedObjectsFile.Render(objects);
        }
        finally { CultureInfo.CurrentCulture = previousCulture; }
        SdlDocument document = ReadRendered(rendered);

        Assert.Contains("[settlement]\r\n", rendered);
        Assert.Equal("0,0,0", document.Settlement["refpos"]);
        Assert.Equal(SdlPlacedObjectsFile.SettlementName, document.Settlement["name"]);
        Assert.Equal(2, document.Objects.Count);
        string[] positions = ["12.25,-3.50,1024.00", "0.00,4.00,9.75"];
        string[] angles = ["45.50", "-90.00"];
        for (int index = 0; index < objects.Length; index++)
        {
            Assert.Contains($"[object{index:0000}]\r\n", rendered);
            SdlObjectSection section = document.Objects[index];
            Assert.Equal(index, section.Index);
            Assert.Equal(positions[index], section.GetValue("pos"));
            Assert.Equal(objects[index].Team.ToString(CultureInfo.InvariantCulture), section.GetValue("team"));
            Assert.Equal(angles[index], section.GetValue("angle"));
            Assert.Equal("1", section.GetValue("onload"));
            Assert.Equal("", section.GetValue("name"));
            Assert.Equal(type.TemplateFields.Count, section.Fields.Count);
            foreach ((string key, string value) in type.TemplateFields)
                if (key is not ("pos" or "team" or "angle" or "onload" or "name"))
                    Assert.Equal(value, section.GetValue(key));
        }
        Assert.Equal("template-name", type.TemplateFields["name"]);
        Assert.Equal("100,200,300", type.TemplateFields["pos"]);
    }

    [Fact]
    public void Render_Clamps_positive_unit_counts_and_preserves_remaining_anzv_values()
    {
        SdlObjectType type = CreateType(SdlObjectCategory.UnitGroup);
        int[] counts = [1, 17, 50, 51, int.MaxValue];
        SdlDocument document = ReadRendered(SdlPlacedObjectsFile.Render(
            counts.Select(count => new SdlPlacedObject(type, 1, 2, 3, 0, UnitCount: count)).ToArray()));

        string[] expected = ["1,7,9", "17,7,9", "50,7,9", "50,7,9", "50,7,9"];
        Assert.Equal(expected, document.Objects.Select(section => section.GetValue("anzv")).ToArray());
        Assert.Equal("6,7,9", type.TemplateFields["anzv"]);
    }

    // 設計：UnitCount ≤ 0 代表「沿用原版範本人數」，編輯器 UI 一律傳入 1–50。
    [Fact]
    public void Render_Keeps_template_unit_count_when_count_is_not_positive()
    {
        SdlObjectType type = CreateType(SdlObjectCategory.UnitGroup);
        foreach (int count in new[] { 0, -1, int.MinValue })
        {
            SdlDocument document = ReadRendered(SdlPlacedObjectsFile.Render(
                [new SdlPlacedObject(type, 1, 2, 3, 0, UnitCount: count)]));
            Assert.Equal("6,7,9", Assert.Single(document.Objects).GetValue("anzv"));
        }
    }

    [Fact]
    public void Render_Keeps_anzv_unchanged_for_every_non_unit_category()
    {
        foreach (SdlObjectCategory category in Enum.GetValues<SdlObjectCategory>().Where(category => category != SdlObjectCategory.UnitGroup))
        {
            SdlObjectType type = CreateType(category);
            SdlDocument document = ReadRendered(SdlPlacedObjectsFile.Render(
                [new SdlPlacedObject(type, 1, 2, 3, 0, UnitCount: 99)]));
            Assert.Equal("6,7,9", Assert.Single(document.Objects).GetValue("anzv"));
        }
    }

    [Fact]
    public void Render_Rejects_invalid_teams()
    {
        SdlObjectType type = CreateType(SdlObjectCategory.Building);
        foreach (int team in new[] { int.MinValue, -2, 16, int.MaxValue })
            Assert.Throws<InvalidDataException>(() => SdlPlacedObjectsFile.Render([new SdlPlacedObject(type, 1, 2, 3, team)]));
    }

    [Fact]
    public void Render_Rejects_non_finite_coordinates_on_every_axis()
    {
        SdlObjectType type = CreateType(SdlObjectCategory.Building);
        foreach (float value in new[] { float.NaN, float.PositiveInfinity, float.NegativeInfinity })
        {
            Assert.Throws<InvalidDataException>(() => SdlPlacedObjectsFile.Render([new SdlPlacedObject(type, value, 2, 3, 0)]));
            Assert.Throws<InvalidDataException>(() => SdlPlacedObjectsFile.Render([new SdlPlacedObject(type, 1, value, 3, 0)]));
            Assert.Throws<InvalidDataException>(() => SdlPlacedObjectsFile.Render([new SdlPlacedObject(type, 1, 2, value, 0)]));
        }
    }

    [Fact]
    public void Save_Writes_PFIL_with_source_header_and_Load_round_trips_catalog_types()
    {
        string source = WriteSdl("ENDL_000", "original.sdl", Settlement +
            Object(0, "VerRomIcoSoldier", "21", "onload=1\r\nanzv=6,7,9\r\n") +
            Object(1, "BauGerHouse", "22", "anzv=0,0,0\r\n"));
        byte[] sourceBytes = File.ReadAllBytes(source);
        // Give every opaque header byte a recognizable value; bytes 16..19 are the payload length.
        for (int index = 4; index < 64; index++)
            if (index is < 16 or >= 20) sourceBytes[index] = (byte)(index + 80);
        File.WriteAllBytes(source, sourceBytes);
        IReadOnlyList<SdlObjectType> catalog = SdlObjectCatalog.Build(_root);
        string map = Path.GetDirectoryName(source)!;
        Assert.Empty(SdlPlacedObjectsFile.Load(map, catalog));
        SdlPlacedObject[] objects =
        [
            new(catalog.Single(type => type.HasUnitCount), 12.25f, -3.5f, 25.75f, 15, 45.5f, 17),
            new(catalog.Single(type => !type.HasUnitCount), 50, 6.25f, 70.5f, -1, -90, 0),
        ];
        using (var rollback = new FileRollbackScope())
        {
            SdlPlacedObjectsFile.Save(map, objects, rollback);
            rollback.Commit();
        }

        Assert.Equal("ARM_Placed.sdl", SdlPlacedObjectsFile.FileName);
        byte[] saved = File.ReadAllBytes(Path.Combine(map, SdlPlacedObjectsFile.FileName));
        Assert.Equal("PFIL"u8.ToArray(), saved.Take(4).ToArray());
        Assert.True(saved.Length > 64);
        for (int index = 0; index < 64; index++)
            if (index is < 16 or >= 20) Assert.Equal(sourceBytes[index], saved[index]);
        string text = SyntheticFixture.Text(saved);
        Assert.Equal(MapTextEncoding.Game.GetByteCount(text), BitConverter.ToInt32(saved, 16));
        Assert.Contains("[object0001]", text);
        IReadOnlyList<SdlPlacedObject> loaded = SdlPlacedObjectsFile.Load(map, catalog);
        Assert.Equal(objects.Length, loaded.Count);
        for (int index = 0; index < objects.Length; index++)
        {
            Assert.Equal(objects[index], loaded[index]);
            Assert.Same(objects[index].Type, loaded[index].Type);
        }
        Assert.Equal(sourceBytes, File.ReadAllBytes(source));
    }

    [Fact]
    public void Save_Empty_list_deletes_existing_file_when_committed()
    {
        string map = CreatePlacementMap();
        string path = WriteSdl("ENDL_000", SdlPlacedObjectsFile.FileName, Settlement + Object(0, "BauRomHouse", "1"));
        using (var rollback = new FileRollbackScope())
        {
            SdlPlacedObjectsFile.Save(map, [], rollback);
            Assert.False(File.Exists(path));
            rollback.Commit();
        }
        Assert.False(File.Exists(path));
        Assert.Empty(SdlPlacedObjectsFile.Load(map, []));
    }

    [Fact]
    public void Save_Rollback_restores_absent_file()
    {
        string map = CreatePlacementMap();
        string path = Path.Combine(map, SdlPlacedObjectsFile.FileName);
        Assert.False(File.Exists(path));
        using (var rollback = new FileRollbackScope())
        {
            SdlPlacedObjectsFile.Save(map, [new SdlPlacedObject(CreateType(SdlObjectCategory.Building), 1, 2, 3, 0)], rollback);
            Assert.True(File.Exists(path));
            Assert.Single(SdlPlacedObjectsFile.Load(map, []));
        }
        Assert.False(File.Exists(path));
    }

    [Fact]
    public void Save_Rollback_restores_previous_bytes_after_replacement()
    {
        string map = CreatePlacementMap();
        string path = WriteSdl("ENDL_000", SdlPlacedObjectsFile.FileName, Settlement + Object(0, "OldType", "99"));
        byte[] previous = File.ReadAllBytes(path);
        using (var rollback = new FileRollbackScope())
        {
            SdlPlacedObjectsFile.Save(map, [new SdlPlacedObject(CreateType(SdlObjectCategory.Building), 1, 2, 3, 0)], rollback);
            Assert.False(previous.SequenceEqual(File.ReadAllBytes(path)));
        }
        Assert.Equal(previous, File.ReadAllBytes(path));
    }

    [Fact]
    public void Save_Rollback_restores_previous_bytes_after_deletion()
    {
        string map = CreatePlacementMap();
        string path = WriteSdl("ENDL_000", SdlPlacedObjectsFile.FileName, Settlement + Object(0, "OldType", "99"));
        byte[] previous = File.ReadAllBytes(path);
        using (var rollback = new FileRollbackScope())
        {
            SdlPlacedObjectsFile.Save(map, [], rollback);
            Assert.False(File.Exists(path));
        }
        Assert.Equal(previous, File.ReadAllBytes(path));
    }

    [Fact]
    public void SceneCatalog_LoadDirectory_and_LoadSettlementOrigins_ignore_ARM_Placed()
    {
        string original = WriteSdl("ENDL_000", "original.sdl", "[settlement]\r\nrefpos=100,20,300\r\n" +
            Object(0, "BauRomHouse", "1", "pos=10,2,30\r\nteam=3\r\n"));
        string placed = WriteSdl("ENDL_000", SdlPlacedObjectsFile.FileName, Settlement +
            Object(0, "FigGerLeader", "2", "pos=40,5,60\r\nteam=4\r\n"));
        Assert.Single(SdlSceneCatalog.Load(placed)); // Valid and in bounds, so exclusion must be by filename.
        string map = Path.GetDirectoryName(original)!;

        MapSceneObject sceneObject = Assert.Single(SdlSceneCatalog.LoadDirectory(map));
        Assert.Equal("original.sdl", sceneObject.SourceFile);
        Assert.Equal("BauRomHouse", sceneObject.Name);
        Assert.Equal((110f, 22f, 330f), (sceneObject.WorldX, sceneObject.WorldY, sceneObject.WorldZ));
        IReadOnlyDictionary<string, SdlVector3> origins = SdlSceneCatalog.LoadSettlementOrigins(map);
        Assert.Single(origins);
        Assert.Equal(new SdlVector3(100, 20, 300), origins["original.sdl"]);
        Assert.False(origins.ContainsKey(SdlPlacedObjectsFile.FileName));
    }

    private const string Settlement = "[settlement]\r\nname=synthetic\r\nrefpos=0,0,0\r\n";

    private static string Object(int index, string name, string definition, string fields = "")
        => $"[object{index:0000}]\r\nnamedef={name}\r\ndef={definition}\r\n" + fields;

    private string WriteSdl(string mapName, string fileName, string text)
    {
        string map = Path.Combine(_root, "MAPS", mapName);
        Directory.CreateDirectory(map);
        string path = Path.Combine(map, fileName);
        File.WriteAllBytes(path, SyntheticFixture.Pfil(text));
        return path;
    }

    private SdlDocument ReadRendered(string text)
        => SdlDocument.Load(WriteSdl("RENDER", Guid.NewGuid().ToString("N") + ".sdl", text));

    private string CreatePlacementMap()
        => Path.GetDirectoryName(WriteSdl("ENDL_000", "original.sdl", Settlement + Object(0, "BauRomHouse", "1")))!;

    private static SdlObjectType CreateType(SdlObjectCategory category)
        => new(category == SdlObjectCategory.UnitGroup ? "VerRomIcoSoldier" : "BauRomHouse", 42, category, "Rom", 1,
            new Dictionary<string, string>
            {
                ["namedef"] = category == SdlObjectCategory.UnitGroup ? "VerRomIcoSoldier" : "BauRomHouse",
                ["def"] = "42", ["name"] = "template-name", ["pos"] = "100,200,300",
                ["team"] = "8", ["angle"] = "123", ["onload"] = "0", ["anzv"] = "6,7,9",
                ["nation"] = "3", ["objdefn0"] = "FigRomSoldier", ["objdefv0"] = "926298413",
                ["unknown"] = "opaque value", ["empty"] = "",
            });

    private static void AssertType(IReadOnlyList<SdlObjectType> catalog, string name, int definition,
        SdlObjectCategory category, string tribe, int occurrences)
    {
        SdlObjectType type = Assert.Single(catalog, item => item.NameDef == name);
        Assert.Equal(definition, type.Definition);
        Assert.Equal(category, type.Category);
        Assert.Equal(tribe, type.Tribe);
        Assert.Equal(occurrences, type.Occurrences);
        Assert.Equal(category == SdlObjectCategory.UnitGroup, type.HasUnitCount);
    }

    public void Dispose()
    {
        if (Directory.Exists(_root)) Directory.Delete(_root, true);
    }
}
