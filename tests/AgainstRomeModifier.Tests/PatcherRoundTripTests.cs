using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Tests;

public sealed class PatcherRoundTripTests {
    private static readonly IReadOnlyDictionary<string, double[]> NoStats = new Dictionary<string, double[]>();

    [Fact]
    public void Objdef_capacity_and_build_time_patch_are_exact_and_restore_from_fixture() {
        string[] columns = Enumerable.Repeat("     0", 200).ToArray();
        columns[42] = "   100";
        columns[52] = "BauHau00_Haus".PadLeft(20);
        columns[73] = "  1000";
        columns[74] = "   500";
        columns[156] = "    10";
        byte[] original = SyntheticFixture.Pfil("header1\r\nheader2\r\n" + string.Join(',', columns) + "\r\n");
        var enabled = new ObjdefOptions(false, true, true, true, false, false, false, false, false, false, false, NoStats);

        byte[] patched = ObjdefPatcher.GetPatchedBytes(original, enabled);
        string[] result = SyntheticFixture.Text(patched).Split("\r\n")[2].Split(',');

        Assert.Equal("200", result[156].Trim());
        Assert.Equal("1000", result[42].Trim());
        Assert.Equal("100", result[73].Trim());
        Assert.Equal("50", result[74].Trim());
        Assert.Equal(original, ObjdefPatcher.GetPatchedBytes(original, new(false, false, false, false, false, false, false, false, false, false, false, NoStats)));
    }

    [Fact]
    public void Objdef_leader_glory_multiplies_growth_columns_only_when_enabled() {
        string[] columns = Enumerable.Repeat("     0", 200).ToArray();
        columns[52] = "FigRomAnf00_Anfuehrer".PadLeft(24);
        columns[148] = "     2";
        columns[149] = "     3";
        columns[150] = "     4";
        columns[161] = "     5";
        columns[153] = "   100"; // 升級榮譽需求：依產品決策不修改
        byte[] original = SyntheticFixture.Pfil("header1\r\nheader2\r\n" + string.Join(',', columns) + "\r\n");

        byte[] patched = ObjdefPatcher.GetPatchedBytes(original, new ObjdefOptions(false, false, false, false, false, true, false, false, false, false, false, NoStats));
        string[] result = SyntheticFixture.Text(patched).Split("\r\n")[2].Split(',');

        Assert.Equal("10", result[148].Trim());
        Assert.Equal("15", result[149].Trim());
        Assert.Equal("20", result[150].Trim());
        Assert.Equal("25", result[161].Trim());
        Assert.Equal("100", result[153].Trim());
        Assert.Equal(original, ObjdefPatcher.GetPatchedBytes(original, new(false, false, false, false, false, false, false, false, false, false, false, NoStats)));
    }

    [Fact]
    public void Objdef_unit_range_scaling_touches_range_columns_only_and_never_weapon_angle() {
        // 依 docs/reverse-engineering/objdef-fields.csv：
        //   80 = Weapon1RangeMin (w1_rad1)、81 = Weapon1RangeMax (w1_rad2)、
        //   82 = Weapon1Angle —— 明載 "never scale as range"。
        string[] columns = Enumerable.Repeat("       0", 205).ToArray();
        columns[4] = "    2.00";                            // Moves
        columns[19] = "     100";                           // Hp
        columns[23] = "    2.00";                           // Movsf
        columns[24] = "    1500";                           // Sirad
        columns[52] = "FigKelSch00_Bogen".PadLeft(20);      // ranged_inf
        columns[78] = "       1";                           // Weapon1Akti
        columns[79] = "   10.00";                           // Weapon1Dam
        columns[80] = "  100.00";                           // Weapon1RangeMin
        columns[81] = "  200.00";                           // Weapon1RangeMax
        columns[82] = "   45.00";                           // Weapon1Angle（不可被縮放）
        columns[84] = "     500";                           // Weapon1Relt
        columns[199] = "       1";                          // Weapon1Dtyp = ranged
        byte[] original = SyntheticFixture.Pfil("header1\r\nheader2\r\n" + string.Join(',', columns) + "\r\n");

        // stats: HP, Dmg, VW, AW, Speed, Sight, Relt, Range, SpellRadius；Range 400 → rangeScale = 2
        var unitStats = new Dictionary<string, double[]> {
            ["FigKelSch00_Bogen"] = new double[] { 100, 10, 10, 10, 4.0, 1500, 500, 400, 0 }
        };
        byte[] patched = ObjdefPatcher.GetPatchedBytes(original, new ObjdefOptions(false, false, false, false, false, false, false, false, false, false, false, unitStats));
        string[] result = SyntheticFixture.Text(patched).Split("\r\n")[2].Split(',');

        Assert.Equal("100.00", result[80].Trim()); // RangeMin（最小射程）不縮放，避免放大近身死區
        Assert.Equal("400.00", result[81].Trim()); // RangeMax ×2
        Assert.Equal("45.00", result[82].Trim());  // Angle 必須原封不動
    }

    [Fact]
    public void Objdef_ranged_range_3x_expands_targeting_sight_to_the_new_maximum_range() {
        string[] columns = Enumerable.Repeat("       0", 205).ToArray();
        columns[19] = "     100";
        columns[24] = "    1500"; // 原始視野小於新射程
        columns[52] = "FigKelSch00_Bogen".PadLeft(20);
        columns[78] = "       1";
        columns[80] = " 1000.00";
        columns[81] = " 2000.00";
        columns[199] = "       1";
        byte[] original = SyntheticFixture.Pfil("header1\r\nheader2\r\n" + string.Join(',', columns) + "\r\n");
        var stats = new Dictionary<string, double[]> {
            ["FigKelSch00_Bogen"] = new double[] { 100, 10, 10, 10, 2, 1500, 500, 2000, 0 }
        };

        byte[] patched = ObjdefPatcher.GetPatchedBytes(original,
            new ObjdefOptions(false, false, false, false, false, false, true, false, false, false, false, stats));
        string[] result = SyntheticFixture.Text(patched).Split("\r\n")[2].Split(',');

        Assert.Equal("1000.00", result[80].Trim()); // 最小射程不隨 3 倍放大
        Assert.Equal("6000.00", result[81].Trim());
        Assert.Equal("6000", result[24].Trim());
    }

    [Fact]
    public void Objdef_entire_map_vision_updates_supported_combat_and_civilian_units_only() {
        static string[] Row(string name, string sight) {
            string[] columns = Enumerable.Repeat("       0", 205).ToArray();
            columns[24] = sight.PadLeft(8);
            columns[52] = name.PadLeft(28);
            return columns;
        }

        string[] soldier = Row("FigRomInf00_Lanze_Schild", "1500");
        string[] civilian = Row("FigZivMan00_Zivilist", "1200");
        string[] building = Row("BauHau00_Haus", "800");
        byte[] original = SyntheticFixture.Pfil(
            "header1\r\nheader2\r\n" +
            string.Join(',', soldier) + "\r\n" +
            string.Join(',', civilian) + "\r\n" +
            string.Join(',', building) + "\r\n");

        byte[] patched = ObjdefPatcher.GetPatchedBytes(original,
            new ObjdefOptions(false, false, false, false, false, false, false, false, false, false, false, NoStats, true));
        string[] rows = SyntheticFixture.Text(patched).Split("\r\n");

        Assert.Equal("30000", rows[2].Split(',')[24].Trim());
        Assert.Equal("30000", rows[3].Split(',')[24].Trim());
        Assert.Equal("800", rows[4].Split(',')[24].Trim());
        Assert.Equal(original, ObjdefPatcher.GetPatchedBytes(original,
            new ObjdefOptions(false, false, false, false, false, false, false, false, false, false, false, NoStats)));
    }

    [Fact]
    public void Objdef_entire_map_vision_is_the_final_sirad_override_when_range_and_spell_features_are_enabled() {
        static string[] UnitRow(string name) {
            string[] columns = Enumerable.Repeat("       0", 205).ToArray();
            columns[19] = "     100";
            columns[24] = "    1500";
            columns[52] = name.PadLeft(28);
            columns[78] = "       1";
            columns[79] = "   10.00";
            columns[84] = "     500";
            return columns;
        }

        string[] archer = UnitRow("FigKelSch00_Bogen");
        archer[80] = " 1000.00";
        archer[81] = " 2000.00";
        archer[199] = "       1";
        string[] priest = UnitRow("FigKelPri00_Priester");
        priest[199] = "       0";
        byte[] original = SyntheticFixture.Pfil(
            "header1\r\nheader2\r\n" +
            string.Join(',', archer) + "\r\n" +
            string.Join(',', priest) + "\r\n");
        var stats = new Dictionary<string, double[]> {
            ["FigKelSch00_Bogen"] = new double[] { 100, 10, 10, 10, 2, 1500, 500, 2000 },
            ["FigKelPri00_Priester"] = new double[] { 100, 10, 10, 10, 2, 1500, 500, 1500 },
        };

        byte[] beforeFinalOverride = ObjdefPatcher.GetPatchedBytes(original,
            new ObjdefOptions(false, false, false, false, false, false, true, false, true, false, false, stats));
        string[] beforeRows = SyntheticFixture.Text(beforeFinalOverride).Split("\r\n");
        Assert.Equal("6000", beforeRows[2].Split(',')[24].Trim());
        Assert.Equal("30000", beforeRows[3].Split(',')[24].Trim());

        byte[] withFinalOverride = ObjdefPatcher.GetPatchedBytes(original,
            new ObjdefOptions(false, false, false, false, false, false, true, false, true, false, false, stats, true));
        string[] finalRows = SyntheticFixture.Text(withFinalOverride).Split("\r\n");
        Assert.Equal("30000", finalRows[2].Split(',')[24].Trim());
        Assert.Equal("6000.00", finalRows[2].Split(',')[81].Trim());
        Assert.Equal("30000", finalRows[3].Split(',')[24].Trim());
    }

    [Fact]
    public void Objdef_range_scaling_skips_melee_sidearm_and_special_weapons_of_ranged_units() {
        // 原版弓兵配置：W1 近戰副武器 (dtyp=0)、W2 弓 (dtyp=1)、W3 特殊武器 (dtyp=5)。
        // 射程縮放只能動遠程武器 (dtyp 1~4) 的最大射程，否則弓兵會從三倍距離外揮刀。
        string[] columns = Enumerable.Repeat("       0", 205).ToArray();
        columns[19] = "     100";
        columns[24] = "    1500";
        columns[52] = "FigKelSch00_Bogen".PadLeft(20);
        columns[78] = "       1"; columns[80] = "  128.00"; columns[81] = "   64.00"; // W1 近戰
        columns[86] = "       1"; columns[88] = "  100.00"; columns[89] = " 1100.00"; // W2 弓
        columns[94] = "       1"; columns[96] = "    1.00"; columns[97] = "  400.00"; // W3 特殊
        columns[199] = "       0"; columns[200] = "       1"; columns[201] = "       5";
        byte[] original = SyntheticFixture.Pfil("header1\r\nheader2\r\n" + string.Join(',', columns) + "\r\n");
        var stats = new Dictionary<string, double[]> {
            ["FigKelSch00_Bogen"] = new double[] { 100, 0, 0, 0, 2, 1500, 0, 1100, 0 }
        };

        byte[] patched = ObjdefPatcher.GetPatchedBytes(original,
            new ObjdefOptions(false, false, false, false, false, false, true, false, false, false, false, stats));
        string[] result = SyntheticFixture.Text(patched).Split("\r\n")[2].Split(',');

        Assert.Equal("128.00", result[80].Trim());  // W1 近戰射程不可動
        Assert.Equal("64.00", result[81].Trim());
        Assert.Equal("100.00", result[88].Trim());  // W2 最小射程不可動
        Assert.Equal("3300.00", result[89].Trim()); // W2 最大射程 ×3
        Assert.Equal("1.00", result[96].Trim());    // W3 特殊武器不可動
        Assert.Equal("400.00", result[97].Trim());
        Assert.Equal("3300", result[24].Trim());    // 視野同步涵蓋新射程
    }

    [Fact]
    public void Objdef_ranged_range_3x_expands_legionary_throwing_spear_but_not_melee_weapon() {
        string[] columns = Enumerable.Repeat("       0", 205).ToArray();
        columns[19] = "     100";
        columns[24] = "    1500";
        columns[52] = "FigRomSch00_Speer_Schild".PadLeft(26); // Legionary, hybrid_inf
        columns[78] = "       1"; columns[80] = "   64.00"; columns[81] = "  128.00"; // W1 melee
        columns[86] = "       1"; columns[88] = "  100.00"; columns[89] = "  900.00"; // W2 throwing spear
        columns[199] = "       0"; columns[200] = "       1";
        byte[] original = SyntheticFixture.Pfil("header1\r\nheader2\r\n" + string.Join(',', columns) + "\r\n");
        var stats = new Dictionary<string, double[]> {
            ["FigRomSch00_Speer_Schild"] = new double[] { 100, 15, 80, 50, 3.2, 1500, 500, 900, 0 }
        };

        byte[] patched = ObjdefPatcher.GetPatchedBytes(original,
            new ObjdefOptions(false, false, false, false, false, false, true, false, false, false, false, stats));
        string[] result = SyntheticFixture.Text(patched).Split("\r\n")[2].Split(',');

        Assert.Equal("64.00", result[80].Trim());
        Assert.Equal("128.00", result[81].Trim());
        Assert.Equal("100.00", result[88].Trim());
        Assert.Equal("2700.00", result[89].Trim());
        Assert.Equal("2700", result[24].Trim());
    }

    [Fact]
    public void Objdef_priest_casting_distance_uses_sight_not_weapon_range_or_spell_radius() {
        string[] columns = Enumerable.Repeat("       0", 205).ToArray();
        columns[4] = "    2.00"; columns[23] = "    2.00"; columns[191] = "    2.00";
        columns[52] = "FigKelPri00_Priester".PadLeft(20);
        columns[78] = "       1"; columns[79] = "   10.00";
        columns[80] = "   64.00"; columns[81] = "  128.00"; columns[82] = "   96.00";
        columns[199] = "       0"; // priest cast weapon is not a ranged projectile
        byte[] original = SyntheticFixture.Pfil("header1\r\nheader2\r\n" + string.Join(',', columns) + "\r\n");
        var stats = new Dictionary<string, double[]> {
            ["FigKelPri00_Priester"] = new double[] { 100, 10, 10, 10, 2, 1500, 500, 128 }
        };

        byte[] map = ObjdefPatcher.GetPatchedBytes(original,
            new ObjdefOptions(false, false, false, false, false, false, false, false, true, false, false, stats));
        string[] mapRow = SyntheticFixture.Text(map).Split("\r\n")[2].Split(',');
        Assert.Equal("30000", mapRow[24].Trim());
        Assert.Equal("64.00", mapRow[80].Trim());
        Assert.Equal("128.00", mapRow[81].Trim());
        Assert.Equal("96.00", mapRow[82].Trim());

        byte[] triple = ObjdefPatcher.GetPatchedBytes(original,
            new ObjdefOptions(false, false, false, false, false, false, false, false, false, true, false, stats));
        string[] tripleRow = SyntheticFixture.Text(triple).Split("\r\n")[2].Split(',');
        Assert.Equal("64.00", tripleRow[80].Trim());
        Assert.Equal("128.00", tripleRow[81].Trim());
        Assert.Equal("96.00", tripleRow[82].Trim());
    }

    [Fact]
    // arc（emit ×2）由 ProjectileArcHeight 驅動；accuracy（drad ×2）已整合進 RangedRange3x。
    public void Objdef_projectile_arc_and_accuracy_scale_emit_and_drad_only_for_projectile_weapons() {
        // 依 docs/reverse-engineering/projectile-ballistics.md：
        //   85 = w1_emit（拋射垂直初速，16.16 定點）、164 = w1_drad（落點傷害半徑）。
        //   emit>0 才是拋射武器；w1 為近戰（emit=0）時兩欄都不可動。
        string[] columns = Enumerable.Repeat("       0", 205).ToArray();
        columns[52] = "FigKelSch00_Bogen".PadLeft(20);
        columns[78] = "       1";  // w1 akti（近戰，emit=0 → 不可動）
        columns[164] = "      45"; // w1 drad
        columns[86] = "       1";  // w2 akti（拋射）
        columns[93] = " 7208960"; // w2 emit = 110.0
        columns[166] = "      45"; // w2 drad
        columns[102] = "       1"; // w4 akti（攻擊建築用火把，dtyp=5）
        columns[109] = " 9830400"; // w4 emit = 150.0
        columns[170] = "      45"; // w4 drad
        columns[202] = "       5"; // w4 dtyp = building torch
        byte[] original = SyntheticFixture.Pfil("header1\r\nheader2\r\n" + string.Join(',', columns) + "\r\n");

        byte[] patched = ObjdefPatcher.GetPatchedBytes(original, new ObjdefOptions(false, false, false, false, false, false, true, false, false, false, true, NoStats));
        string[] result = SyntheticFixture.Text(patched).Split("\r\n")[2].Split(',');

        Assert.Equal("14417920", result[93].Trim()); // w2 emit ×2
        Assert.Equal("90", result[166].Trim());      // w2 drad ×2
        Assert.Equal("19660800", result[109].Trim());// 火把 w4 emit ×2
        Assert.Equal("90", result[170].Trim());      // 火把 w4 drad ×2
        Assert.Equal("0", result[85].Trim());        // w1 emit 維持 0
        Assert.Equal("45", result[164].Trim());      // w1（近戰）drad 不可動
        Assert.Equal(original, ObjdefPatcher.GetPatchedBytes(original, new(false, false, false, false, false, false, false, false, false, false, false, NoStats)));
    }

    [Fact]
    public void Partgeo_arc_patch_scales_ysub_for_all_projectile_rows_and_restores() {
        static string Row(int idx, string name, string ysub) =>
            $"{idx,5},    1,{name,30},        0,        0,   983040,        0,        0,        0,        0,        0,        0,{ysub,9},        0,        0,        0";
        string text = "[ParticleGeometrieDefault]\r\n;header\r\n" +
            Row(0, "Rauch", "  -983040") + "\r\n" +
            Row(20, "Wurfspeer00", "  5832704") + "\r\n" +
            Row(21, "Wurfaxt00", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n" +
            Row(75, "Fackel", "  8126464") + "\r\n";
        byte[] original = SyntheticFixture.Pfil(text);

        string patched = SyntheticFixture.Text(PartgeoPatcher.GetPatchedBytes(original, new PartgeoOptions(true)));
        string[] lines = patched.Split("\r\n");
        Assert.Contains("-983040", lines[2]);                       // 非拋射物（煙霧）不可動
        Assert.Equal("11665408", lines[3].Split(',')[12].Trim());   // Wurfspeer00 ×2
        Assert.Equal("17956864", lines[5].Split(',')[12].Trim());   // Katapultstein00 ×2
        Assert.Equal("11665408", lines[6].Split(',')[12].Trim());   // Pfeil00 ×2
        Assert.Equal("16252928", lines[9].Split(',')[12].Trim());   // 建築火把 Fackel ×2，與 w4 emit 同步
        Assert.Equal(original, PartgeoPatcher.GetPatchedBytes(original, new PartgeoOptions(false)));
    }

    [Fact]
    public void Partgeo_arc_patch_throws_when_projectile_rows_are_missing() {
        byte[] original = SyntheticFixture.Pfil("[ParticleGeometrieDefault]\r\n;header\r\n   23,    1,                       Pfeil00,0,0,0,0,0,0,0,0,0,  5832704,0,0,0\r\n");
        Assert.Throws<InvalidDataException>(() => PartgeoPatcher.GetPatchedBytes(original, new PartgeoOptions(true)));
    }

    [Fact]
    public void Epara_range3x_zeros_variance_on_move_and_restores() {
        const string text = ";comment\r\n[ProjectileInitSpeedFactor]\r\n1.5\r\n\r\n;lead scatter\r\n[ProjectileVarianceOnMove]\r\n0.5\r\n\r\n[ProjectileVarianceMaximumAngle]\r\n45\r\n";
        byte[] original = SyntheticFixture.Pfil(text);

        string patched = SyntheticFixture.Text(EparaPatcher.GetPatchedBytes(original, rangedRange3x: true));
        Assert.Contains("[ProjectileVarianceOnMove]\r\n0.0\r\n", patched);
        Assert.Contains("[ProjectileVarianceMaximumAngle]\r\n45\r\n", patched);
        // 未啟用射程 3 倍時完全還原
        Assert.Equal(original, EparaPatcher.GetPatchedBytes(original, rangedRange3x: false));
    }

    [Fact]
    public void Epara_range3x_raises_init_speed_so_projectiles_reach_extended_range() {
        const string text = ";comment\r\n[ProjectileInitSpeedFactor]\r\n1.5\r\n\r\n;lead scatter\r\n[ProjectileVarianceOnMove]\r\n0.5\r\n\r\n[ProjectileVarianceMaximumAngle]\r\n45\r\n";
        byte[] original = SyntheticFixture.Pfil(text);

        // 射程 3 倍一併套用兩項命中修正：拉高拋射初速覆蓋放大後的射程 + 預判散布歸零
        string range3x = SyntheticFixture.Text(EparaPatcher.GetPatchedBytes(original, rangedRange3x: true));
        Assert.Contains("[ProjectileInitSpeedFactor]\r\n1.62\r\n", range3x);
        Assert.Contains("[ProjectileVarianceOnMove]\r\n0.0\r\n", range3x);
        Assert.Contains("[ProjectileVarianceMaximumAngle]\r\n45\r\n", range3x);
    }

    [Fact]
    public void Ress_cost_patch_zeros_only_selected_fields_and_restores() {
        string building = "BauHau00," + string.Join(',', Enumerable.Repeat("10", 12));
        string unit = "FigGerPri00_Priester," + string.Join(',', Enumerable.Repeat("5", 28));
        byte[] original = SyntheticFixture.Pfil("[objres]\n" + building + "\n" + unit + "\n");

        byte[] patched = RessPatcher.GetPatchedBytes(original, new(true, true, true));
        string[] lines = SyntheticFixture.Text(patched).Split('\n');
        Assert.All(lines[1].Split(',').Skip(1), value => Assert.Equal("0", value));
        Assert.All(lines[2].Split(',').Skip(13).Take(6), value => Assert.Equal("0", value));
        Assert.All(lines[2].Split(',').Skip(25).Take(4), value => Assert.Equal("0", value));
        Assert.Equal(original, RessPatcher.GetPatchedBytes(original, new(false, false, false)));
    }

    [Fact]
    public void ClScript_patch_changes_expected_values_and_restore_uses_canonical_fixture() {
        const string text = "CiviDelay  =GER, 5000      ; c\nRadius     =KEL, Spell0, 500       ; r\nValue      =KEL, Spell1, 2         ; h\nValue      =GER, SAbility0, 4         ; s\nMoralsDecFlee=GER, 10; m\n";
        byte[] original = SyntheticFixture.Pfil(text);
        var options = new ClScriptOptions(true, true, true, true, true, true, 1, 2.5, 1, original);

        string patched = SyntheticFixture.Text(ClScriptPatcher.GetPatchedBytes(original, options));
        Assert.Contains("GER, 500", patched);
        Assert.Contains("KEL, Spell0, 1250", patched);
        Assert.Contains("KEL, Spell1, 20", patched);
        Assert.Contains("GER, SAbility0, 20", patched);
        Assert.Contains("MoralsDecFlee=GER, 0", patched);
        Assert.Equal(original, ClScriptPatcher.GetPatchedBytes(original, new(false, false, false, false, false, false, 1, 1, 1, original)));
    }

    [Fact]
    public void ClScript_spell_range_3x_scales_radius_not_casting_distance() {
        const string text = "Radius     =KEL, Spell0, 500       ; effect\nValue      =KEL, Spell1, 65        ; heal\n";
        byte[] original = SyntheticFixture.Pfil(text);
        byte[] patched = ClScriptPatcher.GetPatchedBytes(original,
            new ClScriptOptions(false, false, false, false, false, false, 1, 1, 1, original, true));
        string result = SyntheticFixture.Text(patched);
        Assert.Contains("Radius     =KEL, Spell0, 1500", result);
        Assert.Contains("Value      =KEL, Spell1, 65", result);
    }

    [Fact]
    public void TeamDat_patch_sets_positive_population_and_restores() {
        byte[] original = SyntheticFixture.Pfil("[teamdata]\nA,B,C,D,80,F\nA,B,C,D,0,F\n");
        string patched = SyntheticFixture.Text(TeamDatPatcher.GetPatchedBytes(original, new(true)));
        Assert.Contains("A,B,C,D,1600,F", patched);
        Assert.Contains("A,B,C,D,0,F", patched);
        Assert.Equal(original, TeamDatPatcher.GetPatchedBytes(original, new(false)));
    }
}
