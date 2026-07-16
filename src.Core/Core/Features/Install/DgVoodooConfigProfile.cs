using System.Text;
using System.Text.RegularExpressions;

namespace AgainstRomeModifier.Core.Features.Install;

internal static partial class DgVoodooConfigProfile
{
    internal static byte[] ApplyNativeWidescreenWindow(byte[] configBytes)
    {
        ArgumentNullException.ThrowIfNull(configBytes);

        bool hasUtf8Bom = configBytes.AsSpan().StartsWith(Encoding.UTF8.Preamble);
        string config = Encoding.UTF8.GetString(
            configBytes,
            hasUtf8Bom ? Encoding.UTF8.Preamble.Length : 0,
            configBytes.Length - (hasUtf8Bom ? Encoding.UTF8.Preamble.Length : 0));

        config = ReplaceUniqueSetting(config, "FullScreenMode", "true");
        config = ReplaceUniqueSetting(config, "ScalingMode", "stretched_ar");
        config = ReplaceUniqueSetting(config, "CenterAppWindow", "true");
        config = ReplaceUniqueSetting(config, "WindowedAttributes", "");
        config = ReplaceUniqueSetting(config, "FullscreenAttributes", "fake");

        byte[] transformed = Encoding.UTF8.GetBytes(config);
        if (!hasUtf8Bom)
            return transformed;

        byte[] result = new byte[Encoding.UTF8.Preamble.Length + transformed.Length];
        Encoding.UTF8.Preamble.CopyTo(result);
        transformed.CopyTo(result.AsSpan(Encoding.UTF8.Preamble.Length));
        return result;
    }

    private static string ReplaceUniqueSetting(string config, string key, string value)
    {
        Regex setting = new(
            $@"^(?<prefix>[ \t]*{Regex.Escape(key)}[ \t]*=[ \t]*)(?<value>[^\r\n]*)(?=\r?$)",
            RegexOptions.Multiline | RegexOptions.CultureInvariant);
        MatchCollection matches = setting.Matches(config);
        if (matches.Count != 1)
            throw new InvalidDataException($"Expected exactly one dgVoodoo setting named '{key}', found {matches.Count}.");

        return setting.Replace(config, match => match.Groups["prefix"].Value + value, 1);
    }
}
