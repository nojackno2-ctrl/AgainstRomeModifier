using System.IO.Compression;
using System.Reflection;

namespace AgainstRomeModifier.Core.Services;

internal enum BackupZipSource
{
    None,
    Embedded,
    Local
}

internal sealed record BackupZipLoadResult(
    BackupZipSource Source,
    IReadOnlyDictionary<string, byte[]> Files);

internal sealed class BackupSourceLoader
{
    private readonly Assembly _assembly;
    private readonly string _baseDirectory;

    internal BackupSourceLoader(Assembly assembly, string baseDirectory)
    {
        _assembly = assembly;
        _baseDirectory = baseDirectory;
    }

    internal BackupZipLoadResult TryLoadPreferredZip()
    {
        string? resourceName = _assembly.GetManifestResourceNames()
            .FirstOrDefault(name => name.EndsWith("Backup.zip", StringComparison.Ordinal));
        if (resourceName != null)
        {
            using Stream stream = _assembly.GetManifestResourceStream(resourceName)!;
            return new(BackupZipSource.Embedded, LoadZip(stream));
        }

        string localPath = Path.Combine(_baseDirectory, "Backup.zip");
        if (File.Exists(localPath))
        {
            using FileStream stream = File.OpenRead(localPath);
            return new(BackupZipSource.Local, LoadZip(stream));
        }

        return new(BackupZipSource.None,
            new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase));
    }

    internal static IReadOnlyDictionary<string, byte[]> LoadZip(Stream stream)
    {
        var files = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
        using var archive = new ZipArchive(stream, ZipArchiveMode.Read, leaveOpen: true);
        foreach (ZipArchiveEntry entry in archive.Entries)
        {
            if (entry.Name.Length == 0) continue;
            string key = entry.FullName.Replace('\\', '/');
            if (Path.IsPathRooted(key) || key.StartsWith('/') ||
                key.Split('/').Any(part => part == ".."))
                throw new InvalidDataException("Backup.zip contains an unsafe entry path: " + entry.FullName);

            using Stream entryStream = entry.Open();
            using var buffer = new MemoryStream();
            entryStream.CopyTo(buffer);
            files[key] = buffer.ToArray();
        }

        return files;
    }
}
