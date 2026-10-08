using AgainstRomeMapEditor;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class CliffTileCatalogTests
{
    [Fact]
    public void Default_catalog_registers_standard_families_and_facings()
    {
        var catalog = CliffTileCatalog.CreateDefault();

        Assert.Contains("FELS", catalog.Families.Keys);
        Assert.Contains("ITA_FELS", catalog.Families.Keys);
        Assert.Contains("Berg", catalog.Families.Keys);
        Assert.Contains("STEIN", catalog.Families.Keys);

        Assert.NotEmpty(catalog.GetEntries(CliffFacing.North, "FELS"));
        Assert.NotEmpty(catalog.GetEntries(CliffFacing.South, "FELS"));
        Assert.NotEmpty(catalog.GetEntries(CliffFacing.East, "FELS"));
        Assert.NotEmpty(catalog.GetEntries(CliffFacing.West, "FELS"));
        Assert.NotEmpty(catalog.GetEntries(CliffFacing.NorthEastOuter, "FELS"));
        Assert.NotEmpty(catalog.GetEntries(CliffFacing.NorthEastInner, "FELS"));
    }

    [Fact]
    public void PickTile_selects_matching_facing_and_respects_family_preference()
    {
        var catalog = CliffTileCatalog.CreateDefault();

        string? felsNorth = catalog.PickTile(CliffFacing.North, 10, 20, seed: 1, preferredFamily: "FELS");
        Assert.NotNull(felsNorth);
        Assert.StartsWith("FELS_N", felsNorth, StringComparison.OrdinalIgnoreCase);

        string? itaSouth = catalog.PickTile(CliffFacing.South, 10, 20, seed: 1, preferredFamily: "ITA_FELS");
        Assert.NotNull(itaSouth);
        Assert.StartsWith("ITA_FELS_S", itaSouth, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void PickTile_falls_back_gracefully_when_specific_corner_is_missing()
    {
        var catalog = new CliffTileCatalog();
        catalog.RegisterFamily(new CliffFamilyInfo("Custom", "Custom", "BK", "B8"));
        // 僅註冊正向坡與通用印章，無外凸角
        catalog.Register(new CliffTileEntry("Custom_N", CliffFacing.North, "Custom"));
        catalog.Register(new CliffTileEntry("Custom_Generic", CliffFacing.None, "Custom"));

        // 請求東北外凸角，應安全回退至北向坡或通用印章
        string? picked = catalog.PickTile(CliffFacing.NorthEastOuter, 5, 5, seed: 0, preferredFamily: "Custom");
        Assert.NotNull(picked);
        Assert.Equal("Custom_N", picked);
    }

    [Fact]
    public void PickTile_deterministic_variant_distribution_with_seed()
    {
        var catalog = CliffTileCatalog.CreateDefault();

        // 同一座標與種子應產生確定性結果
        string? tileA = catalog.PickTile(CliffFacing.North, 12, 34, seed: 42, preferredFamily: "FELS");
        string? tileB = catalog.PickTile(CliffFacing.North, 12, 34, seed: 42, preferredFamily: "FELS");
        Assert.Equal(tileA, tileB);

        // 測試多種座標是否涵蓋不同變體
        var variants = new HashSet<string>();
        for (int x = 0; x < 20; x++)
        {
            string? tile = catalog.PickTile(CliffFacing.North, x, 0, seed: 42, preferredFamily: "FELS");
            if (tile is not null) variants.Add(tile);
        }

        // FELS_N1 與 FELS_N2 都應出現
        Assert.True(variants.Count > 1, "應均勻挑選多種岩壁變體。");
    }

    [Fact]
    public void BuildAvailable_infers_facings_from_texture_names()
    {
        string[] textures = ["FELS_N1", "FELS_SO_OUT", "ITA_FELS_NW_IN", "Berg1", "OtherGround"];
        var catalog = CliffTileCatalog.BuildAvailable(textures);

        Assert.True(catalog.IsCliffTexture("FELS_N1"));
        Assert.True(catalog.IsCliffTexture("FELS_SO_OUT"));
        Assert.True(catalog.IsCliffTexture("ITA_FELS_NW_IN"));
        Assert.True(catalog.IsCliffTexture("Berg1"));
        Assert.False(catalog.IsCliffTexture("OtherGround"));

        var northEntries = catalog.GetEntries(CliffFacing.North);
        Assert.Contains(northEntries, e => e.Texture == "FELS_N1");

        var southEastEntries = catalog.GetEntries(CliffFacing.SouthEastOuter);
        Assert.Contains(southEastEntries, e => e.Texture == "FELS_SO_OUT");

        var northWestInnerEntries = catalog.GetEntries(CliffFacing.NorthWestInner);
        Assert.Contains(northWestInnerEntries, e => e.Texture == "ITA_FELS_NW_IN");
    }

    [Fact]
    public void FilteredBy_keeps_only_textures_that_exist_in_the_game_library()
    {
        var catalog = CliffTileCatalog.CreateDefault();
        Assert.NotEmpty(catalog.Entries);
        var none = catalog.FilteredBy(_ => false);
        Assert.Empty(none.Entries);
        Assert.NotEmpty(none.Families); // 家族資訊保留

        string kept = catalog.Entries[0].Texture;
        var one = catalog.FilteredBy(name => name.Equals(kept, StringComparison.OrdinalIgnoreCase));
        Assert.All(one.Entries, entry => Assert.Equal(kept, entry.Texture, ignoreCase: true));
        Assert.NotEmpty(one.Entries);
    }
    [Fact]
    public void Real_catalog_uses_archive_spelling_and_only_verified_straight_faces()
    {
        string[] names = ["Fels_AA_008", "fels_aa_004", "Fels_AA_002", "Fels_AA_006", "fels01_00", "fels1", "FELS_N1", "FELS_NE_OUT", "Fels_AA_001"];
        var catalog = CliffTileCatalog.BuildRealNames(names);
        Assert.Equal(4, catalog.Entries.Count);
        Assert.Equal("Fels_AA_008", catalog.PickTile(CliffFacing.North, 1, 1));
        Assert.Equal("fels_aa_004", catalog.PickTile(CliffFacing.East, 1, 1));
        Assert.Equal("Fels_AA_002", catalog.PickTile(CliffFacing.South, 1, 1));
        Assert.Equal("Fels_AA_006", catalog.PickTile(CliffFacing.West, 1, 1));
        foreach (CliffFacing facing in Enum.GetValues<CliffFacing>().Where(f => f == CliffFacing.None || (int)f >= 5))
            Assert.Null(catalog.PickTile(facing, 1, 1));
        Assert.All(catalog.Entries, e => Assert.Contains(e.Texture, names));
        Assert.Empty(CliffTileCatalog.BuildRealNames(null).Entries);
        Assert.Empty(CliffTileCatalog.BuildRealNames(["FELS_N1", "fels1", "fels01_00"]).Entries);
    }

    [Fact]
    public void Filtered_real_catalog_does_not_fall_back_to_another_facing()
    {
        var catalog = CliffTileCatalog.BuildRealNames(["Fels_AA_008", "Fels_AA_002"])
            .FilteredBy(name => name == "Fels_AA_008");
        Assert.Single(catalog.Entries);
        Assert.Null(catalog.PickTile(CliffFacing.South, 0, 0));
        Assert.Null(catalog.PickTile(CliffFacing.NorthEastOuter, 0, 0));
        Assert.Equal("Fels_AA_008", catalog.PickTile(CliffFacing.North, 0, 0));
    }

    [Fact]
    public void Real_plan_skips_unverified_cells_and_limits_collision_and_scree_to_supported_cells()
    {
        var detection = new CliffDetectionResult(16,
            [new CliffCell(3, 3, CliffFacing.North, 80, 100, 0, 0), new CliffCell(12, 12, CliffFacing.NorthEastOuter, 80, 100, 0, 0)], []);
        var catalog = CliffTileCatalog.BuildRealNames(["Fels_AA_008"]);
        var plan = CliffFacePlanner.Plan(16, detection, catalog);
        Assert.True(plan.Succeeded);
        Assert.Single(plan.Issues);
        Assert.Equal("Fels_AA_008", Assert.Single(plan.CliffTiles).Texture);
        Assert.Equal(16, plan.BlockedCollisionPixels.Count);
        Assert.All(plan.BlockedCollisionPixels, p => Assert.True(p.X >= 12 && p.X < 16 && p.Y >= 12 && p.Y < 16));
        Assert.Equal(new ScreePlacement(3, 2, "BK"), Assert.Single(plan.ScreePlacements));
        var unsupported = CliffFacePlanner.Plan(16, new CliffDetectionResult(16, [detection.CliffCells[1]], []), catalog);
        Assert.False(unsupported.Succeeded);
        Assert.Empty(unsupported.CliffTiles);
        Assert.Empty(unsupported.ScreePlacements);
        Assert.Empty(unsupported.BlockedCollisionPixels);
    }}
