using System.Globalization;
using System.IO.Compression;
using System.Text;
using System.Text.Json;

namespace AgainstRomeModifier.Core.Services;

internal sealed record SaveBackupManifest(string OrigFolder, string Title, string Level, string BackupTime);

internal static class SaveBackupManifestCodec
{
    internal const string EntryName = "manifest.json";

    internal static SaveBackupManifest? Read(ZipArchive archive)
    {
        ZipArchiveEntry? entry = archive.GetEntry(EntryName);
        if (entry == null) return null;
        using Stream stream = entry.Open();
        using var reader = new StreamReader(stream, Encoding.UTF8);
        return JsonSerializer.Deserialize<SaveBackupManifest>(reader.ReadToEnd());
    }

    internal static string Write(string folder, string title, string level, DateTime backupTime) =>
        JsonSerializer.Serialize(
            new SaveBackupManifest(folder, title, level,
                backupTime.ToString("o", CultureInfo.InvariantCulture)),
            JsonDefaults.Indented);

    internal static string FormatTime(string value)
    {
        return DateTime.TryParse(value, CultureInfo.InvariantCulture, DateTimeStyles.RoundtripKind, out DateTime time)
            ? time.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture)
            : value;
    }
}
