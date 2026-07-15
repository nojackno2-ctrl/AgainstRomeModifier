using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Features.Ini;
using AgainstRomeModifier.Core.Patches;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class RessFeatureDetectorTests
{
    [Fact]
    public void Detector_reads_all_ress_backed_features_from_patcher_output()
    {
        string root = Path.Combine(Path.GetTempPath(), "arm-ress-detector-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(root, "SYSTEM"));
        try
        {
            string unit = "FigRomInf00_Lanze_Schild," + string.Join(',', Enumerable.Repeat("5", 28));
            string priest = "FigGerPri00_Priester," + string.Join(',', Enumerable.Repeat("5", 28));
            string upgrade = "Ger_Kampf," + string.Join(',', Enumerable.Repeat("5", 300));
            byte[] original = SyntheticFixture.Pfil("[objres]\n" + unit + "\n" + priest + "\n[volkres]\n" + upgrade + "\n");
            byte[] patched = RessPatcher.GetPatchedBytes(original, new RessOptions(true, true, true));
            File.WriteAllBytes(Path.Combine(root, "SYSTEM", "ress.ini"), patched);

            var profile = new PatchProfile();
            new RessFeatureDetector(new NullLogger()).Detect(root, profile);

            Assert.True(profile.FreeProduction);
            Assert.True(profile.FreeUpgrade);
            Assert.True(profile.NoSpellCost);
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    private sealed class NullLogger : ILogger
    {
        public void Log(string message) { }
    }
}
