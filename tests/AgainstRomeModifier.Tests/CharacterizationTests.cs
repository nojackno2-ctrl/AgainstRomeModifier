using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Tests;

public sealed class CharacterizationTests
{
    [Fact]
    public void Custom_unit_layers_do_not_duplicate_the_four_experimental_modifiers()
    {
        double[] fallback = { 100, 20, 10, 10, 4, 1000, 5, 2000, 500 };
        double[] custom = { 150, 30, 15, 15, 99, 1200, 9, 9999, 9999 };

        double[] merged = BackupManager.MergeUnitStatsLayers(
            fallback, custom, supportsSpellRadius: true);

        Assert.Equal(150, merged[0]);
        Assert.Equal(4, merged[4]);
        Assert.Equal(2000, merged[7]);
        Assert.Equal(500, merged[8]);
    }

    private static PatchProfile AllEnabled() => new()
    {
        FocusLoss = true,
        FastCiviProduction = true,
        InfiniteMorale = true,
        FreeProduction = true,
        FreeUpgrade = true,
        NoSpellCost = true,
        MaxPopulation = true,
        RomanEndless = true,
        NativeWidescreen1920x1080 = true,
        CameraZoomOut1 = true,
        Balance = true,
        RangedRange3x = true,
        UnitMovementSpeed2x = true,
        SpellEntireMap = false,
        SpellRange3x = true,
        HousingCapacity20x = true,
        StorageCapacity10x = true,
        HqHp10x = true,
        FastBuildUpgradeRepair = true,
        FoodHealing10x = true,
        VillageBuildRange = true,
        NoSpellAltar = true,
        GameSpeed = 2,
        ToEnglish = true,
        DgVoodoo = true,
        EndlessAiModules = new(StringComparer.OrdinalIgnoreCase)
        {
            ["M1"] = true, ["M2"] = true, ["M3"] = true,
            ["M4"] = true, ["M5"] = true, ["M6"] = true,
        },
    };

    [RequiresBackupZipFact]
    public void Hybrid_fixture_starts_with_all_endless_ai_modules_in_original_state()
    {
        using var fixture = BackupZipGameFixture.Create();
        var orchestrator = new EndlessAiOrchestrator();

        foreach (EndlessAiModule module in orchestrator.UserModules)
        {
            Assert.Equal(PatchState.Original, orchestrator.DetectModule(fixture.RootPath, module));
        }
        Assert.Equal(PatchState.Original, orchestrator.DetectModule(fixture.RootPath, orchestrator.R0));
    }

    [RequiresBackupZipFact]
    public void T1_all_enabled_round_trip_is_detected()
    {
        using var fixture = BackupZipGameFixture.Create();
        var engine = new PatchEngine(new NullLogger());
        using (var rollback = new FileRollbackScope())
        {
            engine.ApplyPatches(fixture.RootPath, AllEnabled(), fixture.Backup, rollback);
            rollback.Commit();
        }

        PatchProfile detected = engine.DetectCurrentPatchState(fixture.RootPath, fixture.Backup);
        Assert.True(detected.FocusLoss);
        Assert.True(detected.FastCiviProduction);
        Assert.True(detected.InfiniteMorale);
        Assert.True(detected.FreeProduction);
        Assert.True(detected.FreeUpgrade);
        Assert.True(detected.NoSpellCost);
        Assert.True(detected.MaxPopulation);
        Assert.True(detected.RomanEndless);
        Assert.True(detected.NativeWidescreen1920x1080);
        Assert.True(detected.CameraZoomOut1);
        Assert.False(detected.Balance); // 平衡表已等同原版；未套用自訂屬性時不會產生可偵測差異。
        Assert.True(detected.RangedRange3x);
        Assert.True(detected.UnitMovementSpeed2x);
        Assert.False(detected.SpellEntireMap);
        Assert.True(detected.SpellRange3x);
        Assert.True(detected.HousingCapacity20x);
        Assert.True(detected.StorageCapacity10x);
        Assert.True(detected.HqHp10x);
        Assert.True(detected.FastBuildUpgradeRepair);
        Assert.True(detected.FoodHealing10x);
        Assert.True(detected.VillageBuildRange);
        Assert.True(detected.NoSpellAltar);
        Assert.True(detected.ToEnglish);
        Assert.True(detected.DgVoodoo);
        Assert.Equal(2, detected.GameSpeed);
        foreach (string id in AllEnabled().EndlessAiModules.Keys)
        {
            Assert.True(detected.GetEndlessAiModule(id), id);
        }
    }

    [RequiresBackupZipFact]
    public void Test_RangedRange3x_And_Speed2x_Only_Does_Not_Trigger_Balance()
    {
        using var fixture = BackupZipGameFixture.Create();
        var engine = new PatchEngine(new NullLogger());
        var profile = new PatchProfile();
        profile.RangedRange3x = true;
        profile.UnitMovementSpeed2x = true;
        
        using (var rollback = new FileRollbackScope())
        {
            engine.ApplyPatches(fixture.RootPath, profile, fixture.Backup, rollback);
            rollback.Commit();
        }

        var currentObjdefPath = Path.Combine(fixture.RootPath, @"SYSTEM\DATA_MP\DEFAULTS\objdef.dau");
        byte[] raw = File.ReadAllBytes(currentObjdefPath);
        byte[] decomp = GameLZSS.DecompressPfil(raw);
        string currentObjdef = System.Text.Encoding.GetEncoding(1251).GetString(decomp);
        
        var currentRows = new List<string[]>();
        foreach (string line in currentObjdef.Split(new[] { "\r\n", "\n" }, StringSplitOptions.None))
        {
            if (line.Length < 100) continue;
            currentRows.Add(AgainstRomeModifier.Core.Patches.PatchText.ParseCsvLine(line));
        }

        var origUnitRows = fixture.Backup.GetBackupUnitRows();
        var unitRows = new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase);
        foreach (string[] cols in currentRows)
        {
            if (cols.Length < 192) continue;
            string name = cols[52].Trim();
            if (TroopConfig.UnitMeta.ContainsKey(name))
            {
                unitRows[name] = cols;
            }
        }

        var diffs = new List<string>();
        foreach (string key in TroopConfig.UnitMeta.Keys)
        {
            if (!unitRows.ContainsKey(key) || !origUnitRows.ContainsKey(key)) continue;
            string utype = TroopConfig.UnitMeta[key].UnitType;

            string[] cols = unitRows[key];
            string[] origCols = origUnitRows[key];

            double curHp = 0, origHp = 0;
            double.TryParse(cols[(int)ObjdefIndex.Hp].Trim(), System.Globalization.NumberStyles.Any, System.Globalization.CultureInfo.InvariantCulture, out curHp);
            double.TryParse(origCols[(int)ObjdefIndex.Hp].Trim(), System.Globalization.NumberStyles.Any, System.Globalization.CultureInfo.InvariantCulture, out origHp);

            double curVw = 0, origVw = 0;
            double.TryParse(cols[(int)ObjdefIndex.Vw].Trim(), System.Globalization.NumberStyles.Any, System.Globalization.CultureInfo.InvariantCulture, out curVw);
            double.TryParse(origCols[(int)ObjdefIndex.Vw].Trim(), System.Globalization.NumberStyles.Any, System.Globalization.CultureInfo.InvariantCulture, out origVw);

            double curAw = 0, origAw = 0;
            double.TryParse(cols[(int)ObjdefIndex.Aw].Trim(), System.Globalization.NumberStyles.Any, System.Globalization.CultureInfo.InvariantCulture, out curAw);
            double.TryParse(origCols[(int)ObjdefIndex.Aw].Trim(), System.Globalization.NumberStyles.Any, System.Globalization.CultureInfo.InvariantCulture, out origAw);

            double curSight = 0, origSight = 0;
            double.TryParse(cols[(int)ObjdefIndex.Sirad].Trim(), System.Globalization.NumberStyles.Any, System.Globalization.CultureInfo.InvariantCulture, out curSight);
            double.TryParse(origCols[(int)ObjdefIndex.Sirad].Trim(), System.Globalization.NumberStyles.Any, System.Globalization.CultureInfo.InvariantCulture, out origSight);

            if (TroopConfig.UnitMeta[key].Tier != "leader")
            {
                if (Math.Abs(curHp - origHp) > 0.01) diffs.Add($"{key} HP diff: cur={curHp}, orig={origHp}");
                if (Math.Abs(curVw - origVw) > 0.01) diffs.Add($"{key} VW diff: cur={curVw}, orig={origVw}");
                if (Math.Abs(curAw - origAw) > 0.01) diffs.Add($"{key} AW diff: cur={curAw}, orig={origAw}");
                bool hasExpandedRangeSight = profile.RangedRange3x && TroopConfig.SupportsRangedRange3x(utype);
                if (!hasExpandedRangeSight && Math.Abs(curSight - origSight) > 0.01) diffs.Add($"{key} Sight diff: cur={curSight}, orig={origSight}");
            }
        }

        Assert.Empty(diffs);
    }

    [RequiresBackupZipFact]
    public void T2_restore_all_returns_original_fixture_bytes()
    {
        using var fixture = BackupZipGameFixture.Create();
        Dictionary<string, byte[]> original = Snapshot(fixture.RootPath);
        var engine = new PatchEngine(new NullLogger());
        Apply(engine, fixture);

        using (var rollback = new FileRollbackScope())
        {
            engine.RestoreOriginalFiles(fixture.RootPath, fixture.Backup, rollback);
            rollback.Commit();
        }

        foreach (var (path, bytes) in original)
        {
            Assert.True(File.Exists(Path.Combine(fixture.RootPath, path)), path);
            byte[] actual = File.ReadAllBytes(Path.Combine(fixture.RootPath, path));
            bool equal = bytes.SequenceEqual(actual);
            if (!equal && IsPfil(bytes) && IsPfil(actual))
            {
                equal = GameLZSS.DecompressPfil(bytes).SequenceEqual(GameLZSS.DecompressPfil(actual));
            }
            Assert.True(equal, path);
        }
        Assert.False(File.Exists(Path.Combine(fixture.RootPath, "D3D8.dll")));
        Assert.False(File.Exists(Path.Combine(fixture.RootPath, "DDraw.dll")));
        Assert.False(File.Exists(Path.Combine(fixture.RootPath, "dgVoodooCpl.exe")));
    }

    [RequiresBackupZipFact]
    public void T3_category_restore_keeps_the_other_category_enabled()
    {
        using var statsFixture = BackupZipGameFixture.Create();
        var statsEngine = new PatchEngine(new NullLogger());
        Apply(statsEngine, statsFixture);
        using (var rollback = new FileRollbackScope())
        {
            statsEngine.RestoreStatsOnly(statsFixture.RootPath, statsFixture.Backup, rollback);
            rollback.Commit();
        }
        PatchProfile afterStats = statsEngine.DetectCurrentPatchState(statsFixture.RootPath, statsFixture.Backup);
        Assert.False(afterStats.FastCiviProduction);
        Assert.False(afterStats.FreeProduction);
        Assert.False(afterStats.Balance);
        Assert.False(afterStats.RangedRange3x);
        Assert.False(afterStats.UnitMovementSpeed2x);
        Assert.False(afterStats.SpellEntireMap);
        Assert.False(afterStats.SpellRange3x);
        Assert.False(afterStats.MaxPopulation);
        Assert.False(afterStats.RomanEndless);
        Assert.True(afterStats.NativeWidescreen1920x1080);
        Assert.True(afterStats.CameraZoomOut1);
        Assert.False(afterStats.FoodHealing10x);
        Assert.True(afterStats.FocusLoss);
        Assert.True(afterStats.VillageBuildRange);
        Assert.Equal(2, afterStats.GameSpeed);
 
        using var compatFixture = BackupZipGameFixture.Create();
        var compatEngine = new PatchEngine(new NullLogger());
        Apply(compatEngine, compatFixture);
        using (var rollback = new FileRollbackScope())
        {
            compatEngine.RestoreCompatOnly(compatFixture.RootPath, compatFixture.Backup, rollback);
            rollback.Commit();
        }
        PatchProfile afterCompat = compatEngine.DetectCurrentPatchState(compatFixture.RootPath, compatFixture.Backup);
        Assert.False(afterCompat.FocusLoss);
        Assert.False(afterCompat.VillageBuildRange);
        Assert.Equal(1, afterCompat.GameSpeed);
        Assert.True(afterCompat.FastCiviProduction);
        Assert.True(afterCompat.FreeProduction);
        Assert.False(afterCompat.Balance); // 固定平衡表已是原版數值。
        Assert.True(afterCompat.RangedRange3x);
        Assert.True(afterCompat.UnitMovementSpeed2x);
        Assert.False(afterCompat.SpellEntireMap);
        Assert.True(afterCompat.SpellRange3x);
        Assert.True(afterCompat.MaxPopulation);
        Assert.True(afterCompat.RomanEndless);
        Assert.False(afterCompat.NativeWidescreen1920x1080);
        Assert.False(afterCompat.CameraZoomOut1);
        Assert.True(afterCompat.FoodHealing10x);
    }

    [RequiresBackupZipFact]
    public void T4_repeated_apply_is_byte_idempotent()
    {
        using var fixture = BackupZipGameFixture.Create();
        var engine = new PatchEngine(new NullLogger());
        Apply(engine, fixture);
        Dictionary<string, byte[]> first = Snapshot(fixture.RootPath);
        Apply(engine, fixture);
        Dictionary<string, byte[]> second = Snapshot(fixture.RootPath);

        Assert.Equal(first.Keys.OrderBy(x => x), second.Keys.OrderBy(x => x));
        foreach (string path in first.Keys) Assert.Equal(first[path], second[path]);
    }



    private static void Apply(PatchEngine engine, BackupZipGameFixture fixture)
    {
        using var rollback = new FileRollbackScope();
        engine.ApplyPatches(fixture.RootPath, AllEnabled(), fixture.Backup, rollback);
        rollback.Commit();
    }

    private static Dictionary<string, byte[]> Snapshot(string root) =>
        Directory.GetFiles(root, "*", SearchOption.AllDirectories)
            .ToDictionary(path => Path.GetRelativePath(root, path), File.ReadAllBytes, StringComparer.OrdinalIgnoreCase);

    private static bool IsPfil(byte[] bytes) =>
        bytes.Length >= 4 && bytes[0] == (byte)'P' && bytes[1] == (byte)'F' && bytes[2] == (byte)'I' && bytes[3] == (byte)'L';

    [RequiresBackupZipFact]
    public void Test_Unit_And_Civilian_Movement_Speed_Combinations()
    {
        System.Text.Encoding.RegisterProvider(System.Text.CodePagesEncodingProvider.Instance);
        
        // 情境 1: Balance = true, UnitMovementSpeed2x = false
        {
            using var fixture = BackupZipGameFixture.Create();
            var engine = new PatchEngine(new NullLogger());
            var profile = new PatchProfile { Balance = true, UnitMovementSpeed2x = false };
            using (var rollback = new FileRollbackScope())
            {
                engine.ApplyPatches(fixture.RootPath, profile, fixture.Backup, rollback);
                rollback.Commit();
            }

            // 讀取 objdef.dau 做精確比對
            string objdefPath = Path.Combine(fixture.RootPath, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau");
            byte[] fileBytes = File.ReadAllBytes(objdefPath);
            byte[] decomp = GameLZSS.DecompressPfil(fileBytes);
            string text = System.Text.Encoding.GetEncoding(1251).GetString(decomp);
            string lineEnding = text.Contains("\r\n") ? "\r\n" : "\n";
            string[] lines = text.Split(new string[] { lineEnding }, StringSplitOptions.None);
            
            double? romInfSpeed = null;
            double? civilianSpeed = null;
            for (int idx = 2; idx < lines.Length; idx++) {
                string line = lines[idx];
                if (line.Length < 100) continue;
                string[] cols = line.Split(',');
                if (cols.Length < 192) continue;
                string name = cols[52].Trim();
                if (name == "FigRomInf00_Lanze_Schild") {
                    romInfSpeed = double.Parse(cols[(int)ObjdefIndex.Moves].Trim(), System.Globalization.CultureInfo.InvariantCulture);
                }
                if (name == "FigZivMan00_Zivilist") {
                    civilianSpeed = double.Parse(cols[(int)ObjdefIndex.Moves].Trim(), System.Globalization.CultureInfo.InvariantCulture);
                }
            }
            // 驗證即使有自訂屬性平衡，速度仍應維持原速 (羅馬輕裝步兵=1.60, 平民=1.30)
            Assert.NotNull(romInfSpeed);
            Assert.Equal(1.60, romInfSpeed.Value, 2);
            Assert.NotNull(civilianSpeed);
            Assert.Equal(1.30, civilianSpeed.Value, 2);

            // 確保 Detector 偵測狀態正常
            var detected = engine.DetectCurrentPatchState(fixture.RootPath, fixture.Backup);
            Assert.False(detected.Balance); // 固定平衡表已是原版數值。
            Assert.False(detected.UnitMovementSpeed2x);
        }

        // 情境 2: Balance = true, UnitMovementSpeed2x = true
        {
            using var fixture = BackupZipGameFixture.Create();
            var engine = new PatchEngine(new NullLogger());
            var profile = new PatchProfile { Balance = true, UnitMovementSpeed2x = true };
            using (var rollback = new FileRollbackScope())
            {
                engine.ApplyPatches(fixture.RootPath, profile, fixture.Backup, rollback);
                rollback.Commit();
            }

            string objdefPath = Path.Combine(fixture.RootPath, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau");
            byte[] fileBytes = File.ReadAllBytes(objdefPath);
            byte[] decomp = GameLZSS.DecompressPfil(fileBytes);
            string text = System.Text.Encoding.GetEncoding(1251).GetString(decomp);
            string lineEnding = text.Contains("\r\n") ? "\r\n" : "\n";
            string[] lines = text.Split(new string[] { lineEnding }, StringSplitOptions.None);
            
            double? romInfSpeed = null;
            double? civilianSpeed = null;
            for (int idx = 2; idx < lines.Length; idx++) {
                string line = lines[idx];
                if (line.Length < 100) continue;
                string[] cols = line.Split(',');
                if (cols.Length < 192) continue;
                string name = cols[52].Trim();
                if (name == "FigRomInf00_Lanze_Schild") {
                    romInfSpeed = double.Parse(cols[(int)ObjdefIndex.Moves].Trim(), System.Globalization.CultureInfo.InvariantCulture);
                }
                if (name == "FigZivMan00_Zivilist") {
                    civilianSpeed = double.Parse(cols[(int)ObjdefIndex.Moves].Trim(), System.Globalization.CultureInfo.InvariantCulture);
                }
            }
            // 驗證速度變為 2 倍速 (羅馬輕裝步兵=3.20, 平民=2.60)
            Assert.NotNull(romInfSpeed);
            Assert.Equal(3.20, romInfSpeed.Value, 2);
            Assert.NotNull(civilianSpeed);
            Assert.Equal(2.60, civilianSpeed.Value, 2);

            var detected = engine.DetectCurrentPatchState(fixture.RootPath, fixture.Backup);
            Assert.False(detected.Balance); // 固定平衡表已是原版數值。
            Assert.True(detected.UnitMovementSpeed2x);
        }
    }
}
