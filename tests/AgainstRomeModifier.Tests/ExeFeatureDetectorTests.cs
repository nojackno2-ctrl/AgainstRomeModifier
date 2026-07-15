using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Features.Exe;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class ExeFeatureDetectorTests
{
    private const int ExeSize = 0x205000;

    [Fact]
    public void Detector_reads_all_exe_backed_features_into_profile()
    {
        string root = Path.Combine(Path.GetTempPath(), "arm-exe-detector-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try
        {
            byte[] exe = new byte[ExeSize];
            Place(exe, ExePatchModel.FocusPatchOffset, ExePatchModel.FocusPatchedBytes);
            Place(exe, ExePatchModel.VillageSetterHookOffset, ExePatchModel.VillageSetterHookPatchedBytes);
            Place(exe, ExePatchModel.VillageSetterCaveOffset, ExePatchModel.VillageSetterCavePatchedBytes);
            foreach (var site in ExePatchModel.SpellAltarPatchSites)
                Place(exe, site.Offset, site.Patched);
            Place(exe, ExePatchModel.RomanEndlessPatchOffset, ExePatchModel.RomanEndlessPatchedBytes);
            Place(exe, ExePatchModel.GameSpeedQpcConstOffset, BitConverter.GetBytes(4_000_000_000.0));
            Place(exe, ExePatchModel.GameSpeedTgtConstOffset, BitConverter.GetBytes(4_000_000.0));
            Place(exe, ExePatchModel.CiviProduce20PatchOffset, ExePatchModel.CiviProduce20PatchedBytes);
            Place(exe, ExePatchModel.UnitRecruit20PatchOffset, ExePatchModel.UnitRecruit20PatchedBytes);
            File.WriteAllBytes(Path.Combine(root, "Against_Rome.exe"), exe);

            var profile = new PatchProfile();
            ExeFeatureDetection detection = new ExeFeatureDetector(new NullLogger()).Detect(root, profile);

            Assert.True(profile.FocusLoss);
            Assert.True(profile.VillageBuildRange);
            Assert.True(profile.NoSpellAltar);
            Assert.Equal(4, profile.GameSpeed);
            Assert.True(profile.CiviProduce20);
            Assert.True(profile.UnitRecruit20);
            Assert.Equal(ExeRomanEndlessPatchState.Patched, detection.RomanEndlessState);
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    private static void Place(byte[] exe, long offset, byte[] bytes) =>
        Buffer.BlockCopy(bytes, 0, exe, (int)offset, bytes.Length);

    private sealed class NullLogger : ILogger
    {
        public void Log(string message) { }
    }
}
