using System.IO.Compression;

namespace AgainstRomeMapEditor;

internal sealed class FloorTextureLibrary : IDisposable
{
    private ZipArchive? _archive;
    private Dictionary<string, ZipArchiveEntry> _entries = new(StringComparer.OrdinalIgnoreCase);
    private Dictionary<string, Bitmap> _textures = new(StringComparer.OrdinalIgnoreCase);

    public FloorTextureLibrary(string archivePath)
    {
        if (!File.Exists(archivePath)) return;
        var stream = new FileStream(archivePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
        try
        {
            _archive = new ZipArchive(stream, ZipArchiveMode.Read);
            foreach (ZipArchiveEntry entry in _archive.Entries)
            {
                if (!entry.FullName.EndsWith(".bmp", StringComparison.OrdinalIgnoreCase)) continue;
                string name = Path.GetFileNameWithoutExtension(entry.Name);
                if (!string.IsNullOrWhiteSpace(name)) _entries.TryAdd(name, entry);
            }
        }
        catch { _archive?.Dispose(); stream.Dispose(); throw; }
    }

    public bool IsAvailable => _archive is not null && _entries.Count > 0;

    /// <summary>floortex.dat 內全部可用的地表材質名稱（排序後），供「完整材質庫」調色盤列舉。</summary>
    public IReadOnlyList<string> Names => _names ??= _entries.Keys.OrderBy(x => x, StringComparer.OrdinalIgnoreCase).ToArray();
    private IReadOnlyList<string>? _names;

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

    /// <summary>Adopt a validated archive in place so borrowed library references remain valid. The candidate then owns the old resources.</summary>
    public void ReplaceWith(FloorTextureLibrary candidate)
    {
        ArgumentNullException.ThrowIfNull(candidate);
        if (ReferenceEquals(this, candidate)) return;
        (_archive, candidate._archive) = (candidate._archive, _archive);
        (_entries, candidate._entries) = (candidate._entries, _entries);
        (_textures, candidate._textures) = (candidate._textures, _textures);
        (_names, candidate._names) = (candidate._names, _names);
    }

    public void Dispose()
    {
        foreach (Bitmap texture in _textures.Values) texture.Dispose();
        _textures.Clear();
        _archive?.Dispose();
        _archive = null; _entries.Clear(); _names = null;
    }
}
