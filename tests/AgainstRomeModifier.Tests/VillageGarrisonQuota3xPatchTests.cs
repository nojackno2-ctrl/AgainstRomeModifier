using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class VillageGarrisonQuota3xPatchTests
{
    [Fact]
    public void P20_redirects_all_four_quota_queries_to_one_3x_helper_and_round_trips()
    {
        byte[] original = BuildFixture();
        byte[] data = original.ToArray();
        var patch = new P20_VillageGarrisonQuotaPatch();

        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.True(patch.Apply(ref data, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(data));

        int helperOffset = data.Length - P20_VillageGarrisonQuotaPatch.HelperWords(3).Length * sizeof(int);
        Assert.Equal(P20_VillageGarrisonQuotaPatch.HelperWords(3), ReadWords(data, helperOffset));

        foreach (int site in FindCallSites(data))
        {
            int callOpcodeOffset = site + 4 * sizeof(int);
            Assert.Equal(120, BitConverter.ToInt32(data, callOpcodeOffset));
            Assert.Equal(
                helperOffset - callOpcodeOffset - 8,
                BitConverter.ToInt32(data, callOpcodeOffset + sizeof(int)));
        }

        Assert.True(patch.Apply(ref data, enabled: false));
        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.Equal(original, data);
    }

    [Fact]
    public void P20_supports_each_selectable_multiplier_and_round_trips()
    {
        foreach (int multiplier in P20_VillageGarrisonQuotaPatch.SupportedMultipliers)
        {
            byte[] original = BuildFixture();
            byte[] data = original.ToArray();
            var patch = new P20_VillageGarrisonQuotaPatch { Multiplier = multiplier };

            Assert.True(patch.Apply(ref data, enabled: true));
            Assert.Equal(PatchState.Ultimate, patch.Detect(data));
            Assert.True(P20_VillageGarrisonQuotaPatch.TryReadInstalledMultiplier(data, out int installed));
            Assert.Equal(multiplier, installed);

            Assert.True(patch.Apply(ref data, enabled: false));
            Assert.Equal(original, data);
        }
    }

    [Fact]
    public void P20_switches_installed_multiplier_in_place_without_moving_code()
    {
        byte[] data = BuildFixture();
        var three = new P20_VillageGarrisonQuotaPatch { Multiplier = 3 };
        Assert.True(three.Apply(ref data, enabled: true));
        int lengthAtThree = data.Length;

        var five = new P20_VillageGarrisonQuotaPatch { Multiplier = 5 };
        Assert.Equal(PatchState.Legacy, five.Detect(data));
        Assert.True(five.Apply(ref data, enabled: true));

        Assert.Equal(lengthAtThree, data.Length);
        Assert.Equal(PatchState.Ultimate, five.Detect(data));
        Assert.Equal(PatchState.Legacy, three.Detect(data));
        Assert.True(P20_VillageGarrisonQuotaPatch.TryReadInstalledMultiplier(data, out int installed));
        Assert.Equal(5, installed);

        // Disabling from a "legacy" (different-multiplier) state must still restore.
        Assert.True(three.Apply(ref data, enabled: false));
        Assert.Equal(PatchState.Original, three.Detect(data));
        Assert.Equal(BuildFixture(), data);
    }

    [Fact]
    public void P20_rejects_unsupported_multiplier_values()
    {
        var patch = new P20_VillageGarrisonQuotaPatch();
        Assert.Throws<ArgumentOutOfRangeException>(() => patch.Multiplier = 1);
        Assert.Throws<ArgumentOutOfRangeException>(() => patch.Multiplier = 4);
        Assert.Throws<ArgumentOutOfRangeException>(() => patch.Multiplier = 0);

        // A helper carrying an unrecognized literal must be Unknown, never rewritten.
        byte[] data = BuildFixture();
        Assert.True(patch.Apply(ref data, enabled: true));
        Assert.True(P20_VillageGarrisonQuotaPatch.TryReadInstalledMultiplier(data, out _));
        int helperOffset = data.Length - P20_VillageGarrisonQuotaPatch.HelperWords(3).Length * sizeof(int);
        BitConverter.GetBytes(7).CopyTo(data, helperOffset + 14 * sizeof(int));
        Assert.Equal(PatchState.Unknown, patch.Detect(data));
        Assert.Throws<InvalidOperationException>(() => patch.Apply(ref data, enabled: false));
    }

    [Fact]
    public void P20_is_idempotent_in_both_states()
    {
        byte[] data = BuildFixture();
        var patch = new P20_VillageGarrisonQuotaPatch();

        Assert.False(patch.Apply(ref data, enabled: false));
        Assert.True(patch.Apply(ref data, enabled: true));
        byte[] first = data.ToArray();
        Assert.False(patch.Apply(ref data, enabled: true));
        Assert.Equal(first, data);
    }

    [Fact]
    public void P20_refuses_unknown_helper_target_without_mutating_bytes()
    {
        byte[] data = BuildFixture();
        var patch = new P20_VillageGarrisonQuotaPatch();
        Assert.True(patch.Apply(ref data, enabled: true));

        int firstSite = FindCallSites(data)[0];
        int operandOffset = firstSite + 5 * sizeof(int);
        BitConverter.GetBytes(BitConverter.ToInt32(data, operandOffset) + 4).CopyTo(data, operandOffset);
        byte[] before = data.ToArray();

        Assert.Equal(PatchState.Unknown, patch.Detect(data));
        Assert.Throws<InvalidOperationException>(() => patch.Apply(ref data, enabled: false));
        Assert.Equal(before, data);
    }

    [Fact]
    public void Workspace_original_Dorfverteidigung_signature_applies_and_round_trips_when_available()
    {
        string path = Path.GetFullPath(Path.Combine(
            AppContext.BaseDirectory,
            "../../../../../遊戲原始檔案/SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci"));
        if (!File.Exists(path)) return; // Proprietary local fixture is intentionally absent in CI.

        byte[] original = GameLZSS.DecompressPfil(File.ReadAllBytes(path));
        byte[] data = original.ToArray();
        var patch = new P20_VillageGarrisonQuotaPatch { Multiplier = 5 };

        Assert.Equal(PatchState.Original, patch.Detect(data));
        Assert.True(patch.Apply(ref data, enabled: true));
        Assert.Equal(PatchState.Ultimate, patch.Detect(data));
        Assert.True(P20_VillageGarrisonQuotaPatch.TryReadInstalledMultiplier(data, out int installed));
        Assert.Equal(5, installed);
        Assert.True(patch.Apply(ref data, enabled: false));
        Assert.Equal(original, data);
    }

    [RequiresBackupZipFact]
    public void Standalone_feature_applies_detects_and_restores_through_patch_engine()
    {
        using var fixture = BackupZipGameFixture.Create();
        var engine = new PatchEngine(new NullLogger());
        var profile = new PatchProfile { VillageGarrisonQuotaMultiplier = 5 };

        using (var rollback = new FileRollbackScope())
        {
            engine.ApplyPatches(fixture.RootPath, profile, fixture.Backup, rollback);
            rollback.Commit();
        }

        PatchProfile detected = engine.DetectCurrentPatchState(fixture.RootPath, fixture.Backup);
        Assert.True(detected.VillageGarrisonQuota3x);
        Assert.Equal(5, detected.VillageGarrisonQuotaMultiplier);

        using (var rollback = new FileRollbackScope())
        {
            engine.RestoreCompatOnly(fixture.RootPath, fixture.Backup, rollback);
            rollback.Commit();
        }

        Assert.False(engine.DetectCurrentPatchState(fixture.RootPath, fixture.Backup).VillageGarrisonQuota3x);
    }

    private static byte[] BuildFixture()
    {
        var words = new List<int>();
        int[] types = { 1, 2, 3, 4 };
        int[] arrayVars = { 8, 10, 12, 14 };
        int[] stores = { 59, 61, 63, 65 };
        for (int i = 0; i < types.Length; i++)
        {
            words.AddRange(new[]
            {
                66, types[i], 81, arrayVars[i],
                128, 151, 73, -2, 86, 91, stores[i],
                unchecked((int)0x6F6F6F6F),
            });
        }
        return Words(words);
    }

    private static int[] FindCallSites(byte[] data)
    {
        int[] types = { 1, 2, 3, 4 };
        var sites = new int[types.Length];
        for (int i = 0; i < types.Length; i++)
        {
            int?[] signature = { 66, types[i], 81, null, null, null, 73, -2, 86, 91, null };
            sites[i] = BciPattern.FindBciWordPattern(data, signature);
            Assert.True(sites[i] >= 0);
        }
        return sites;
    }

    private static int[] ReadWords(byte[] data, int offset)
    {
        var words = new int[P20_VillageGarrisonQuotaPatch.HelperWords(3).Length];
        for (int i = 0; i < words.Length; i++)
            words[i] = BitConverter.ToInt32(data, offset + i * sizeof(int));
        return words;
    }

    private static byte[] Words(IEnumerable<int> words)
    {
        int[] values = words.ToArray();
        byte[] bytes = new byte[values.Length * sizeof(int)];
        for (int i = 0; i < values.Length; i++)
            BitConverter.GetBytes(values[i]).CopyTo(bytes, i * sizeof(int));
        return bytes;
    }
}
