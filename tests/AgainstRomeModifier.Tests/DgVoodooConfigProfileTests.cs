using System.Text;
using AgainstRomeModifier.Core.Features.Install;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Tests;

public sealed class DgVoodooConfigProfileTests
{
    [Fact]
    public void Native_widescreen_profile_preserves_layout_and_is_idempotent()
    {
        const string source = "\uFEFF[General]\r\nFullScreenMode = true\r\nScalingMode       = unspecified\r\nKeepWindowAspectRatio = true\r\nCenterAppWindow = false\r\n[GeneralExt]\r\nWindowedAttributes = \r\nFullscreenAttributes = \r\nFPSLimit = 0\r\n";
        byte[] body = Encoding.UTF8.GetBytes(source.TrimStart('\uFEFF'));
        byte[] original = new byte[Encoding.UTF8.Preamble.Length + body.Length];
        Encoding.UTF8.Preamble.CopyTo(original);
        body.CopyTo(original.AsSpan(Encoding.UTF8.Preamble.Length));

        byte[] transformed = DgVoodooConfigProfile.ApplyNativeWidescreenWindow(original);
        byte[] transformedAgain = DgVoodooConfigProfile.ApplyNativeWidescreenWindow(transformed);
        string text = Encoding.UTF8.GetString(transformed);

        Assert.True(transformed.AsSpan().StartsWith(Encoding.UTF8.Preamble));
        Assert.Contains("ScalingMode       = stretched_ar\r\n", text);
        Assert.Contains("CenterAppWindow = true\r\n", text);
        Assert.Contains("WindowedAttributes = \r\n", text);
        Assert.Contains("FullscreenAttributes = fake\r\n", text);
        Assert.Contains("KeepWindowAspectRatio = true\r\n", text);
        Assert.Equal(transformed, transformedAgain);
    }

    [Fact]
    public void Native_widescreen_profile_rejects_an_unknown_config_shape()
    {
        byte[] missingWindowSetting = Encoding.UTF8.GetBytes("ScalingMode=unspecified\n");

        InvalidDataException error = Assert.Throws<InvalidDataException>(
            () => DgVoodooConfigProfile.ApplyNativeWidescreenWindow(missingWindowSetting));

        Assert.Contains("dgVoodoo setting", error.Message);
    }

    [Fact]
    public void Managed_install_uses_widescreen_window_profile()
    {
        string root = CreateTempDirectory();
        try
        {
            var feature = new DgVoodooFeature(new RecordingLogger());
            feature.Apply(root, enabled: true, nativeWidescreenWindow: true);

            string config = File.ReadAllText(Path.Combine(root, "dgVoodoo.conf"), Encoding.UTF8);
            Assert.Contains("ScalingMode                          = stretched_ar", config);
            Assert.Contains("CenterAppWindow                      = true", config);
            Assert.Contains("WindowedAttributes                   = \r\n", config);
            Assert.Contains("FullscreenAttributes                 = fake", config);
            Assert.True(feature.IsCenteredPresentationConfigured(root));
            Assert.True(feature.IsInstalled(root));
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    [Fact]
    public void Customized_config_keeps_unrelated_values_and_receives_widescreen_profile_on_reapply()
    {
        string root = CreateTempDirectory();
        try
        {
            var logger = new RecordingLogger();
            var feature = new DgVoodooFeature(logger);
            feature.Apply(root, enabled: true);
            string configPath = Path.Combine(root, "dgVoodoo.conf");
            string customized = File.ReadAllText(configPath, Encoding.UTF8)
                .Replace("FPSLimit                             = 0", "FPSLimit                             = 60", StringComparison.Ordinal);
            File.WriteAllText(configPath, customized, new UTF8Encoding(false));

            feature.Apply(root, enabled: false);
            feature.Apply(root, enabled: true, nativeWidescreenWindow: true);

            string reapplied = File.ReadAllText(configPath, Encoding.UTF8);
            Assert.Contains("FPSLimit                             = 60", reapplied);
            Assert.Contains("ScalingMode                          = stretched_ar", reapplied);
            Assert.Contains("CenterAppWindow                      = true", reapplied);
            Assert.Contains("WindowedAttributes                   = \r\n", reapplied);
            Assert.Contains("FullscreenAttributes                 = fake", reapplied);
            Assert.Contains(logger.Messages, message =>
                message.Contains("dgVoodoo.conf", StringComparison.OrdinalIgnoreCase));
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    private static string CreateTempDirectory()
    {
        string path = Path.Combine(Path.GetTempPath(), "AgainstRomeDgVoodooTests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(path);
        return path;
    }

    private sealed class RecordingLogger : ILogger
    {
        internal List<string> Messages { get; } = new();
        public void Log(string message) => Messages.Add(message);
    }
}
