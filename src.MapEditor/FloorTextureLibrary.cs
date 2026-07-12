using System.IO.Compression;

namespace AgainstRomeMapEditor;

internal sealed class FloorTextureLibrary : IDisposable
{
    private readonly ZipArchive? _archive;
    private readonly Dictionary<string, ZipArchiveEntry> _entries = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, Bitmap> _textures = new(StringComparer.OrdinalIgnoreCase);

    public FloorTextureLibrary(string archivePath)
    {
        if (!File.Exists(archivePath)) return;
        _archive = ZipFile.OpenRead(archivePath);
        foreach (ZipArchiveEntry entry in _archive.Entries)
        {
            if (!entry.FullName.EndsWith(".bmp", StringComparison.OrdinalIgnoreCase)) continue;
            string name = Path.GetFileNameWithoutExtension(entry.Name);
            if (!string.IsNullOrWhiteSpace(name)) _entries.TryAdd(name, entry);
        }
    }

    public bool IsAvailable => _archive is not null && _entries.Count > 0;

    public Bitmap? Get(string textureName)
    {
        if (_textures.TryGetValue(textureName, out Bitmap? cached)) return cached;
        if (!_entries.TryGetValue(textureName, out ZipArchiveEntry? entry)) return null;
        using Stream stream = entry.Open();
        using var source = new Bitmap(stream);
        var texture = new Bitmap(source);
        _textures[textureName] = texture;
        return texture;
    }

    public void Dispose()
    {
        foreach (Bitmap texture in _textures.Values) texture.Dispose();
        _textures.Clear();
        _archive?.Dispose();
    }
}
