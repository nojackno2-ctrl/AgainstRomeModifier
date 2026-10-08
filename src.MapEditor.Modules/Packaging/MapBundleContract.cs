using System.Text.RegularExpressions;

namespace AgainstRomeMapEditor.Modules.Packaging;

/// <summary>Only map-local runtime data and editor scenario metadata belong in a bundle.</summary>
internal static class MapBundleContract
{
    private static readonly HashSet<string> RootFiles = new(StringComparer.OrdinalIgnoreCase)
    {
        "boden.bmp", "boden.ini", "boden.txt", "collision.bmp", "emboss.bmp", "vertex.bmp",
        "smooth.bmp", "minimap.bmp", "floortex.dat", "textures.txt", "briefpic.tga",
        "daynight.bmp", "preload.ini", "arm_scenario.json"
    };

    internal static bool IsPayloadPath(string path)
    {
        if (string.IsNullOrWhiteSpace(path) || path.Contains('\\') || path.Contains(':')) return false;
        string[] parts = path.Split('/');
        if (parts.Any(p => p is "" or "." or ".." || p.EndsWith('.') || p.EndsWith(' ') ||
            p.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0 ||
            Regex.IsMatch(p, @"^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\.|$)", RegexOptions.IgnoreCase))) return false;
        if (parts.Length == 1)
            return RootFiles.Contains(path) || Regex.IsMatch(path, @"^Endlos_[A-Za-z0-9_]+_Siedlung[A-Za-z0-9_]*\.sdl$", RegexOptions.IgnoreCase);
        if (parts.Length == 2 && parts[0].Equals("DATA", StringComparison.OrdinalIgnoreCase))
            return Regex.IsMatch(parts[1], @"^[A-Za-z0-9_]+\.dat$", RegexOptions.IgnoreCase);
        if (parts.Length == 2 && parts[0].Equals("SCRIPT", StringComparison.OrdinalIgnoreCase))
            return Regex.IsMatch(parts[1], @"^[A-Za-z0-9_]+\.bci$", RegexOptions.IgnoreCase);
        return parts.Length == 3 && parts[0].Equals("TEXT", StringComparison.OrdinalIgnoreCase) &&
            Regex.IsMatch(parts[1], @"^[A-Za-z]{2}$") &&
            Regex.IsMatch(parts[2], @"^[A-Za-z0-9_]+\.put$", RegexOptions.IgnoreCase);
    }

    internal static void RejectLinks(string directory)
    {
        if ((File.GetAttributes(directory) & FileAttributes.ReparsePoint) != 0)
            throw new IOException("Linked directories are not supported: " + directory);
        foreach (string entry in Directory.EnumerateFileSystemEntries(directory))
        {
            if ((File.GetAttributes(entry) & FileAttributes.ReparsePoint) != 0)
                throw new IOException("Linked payloads are not supported: " + entry);
            if (Directory.Exists(entry)) RejectLinks(entry);
        }
    }
}
