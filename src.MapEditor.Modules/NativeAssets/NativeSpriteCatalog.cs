using System.Globalization;
using System.IO.Compression;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// objdef row fields that select a native sprite: column 5 alrid, 8 alrml (second ALR layer),
/// 10 mltyp (layer type), 14 aptix, 17 palty and 52 name.
/// </summary>
internal sealed record NativeSpriteDefinition(int TypeId, string Name, int AlrId, int AptIndex, int PaletteType,
    int LayerAlrId = -1, int LayerType = -1);

/// <summary>
/// A cropped, straight-alpha ARGB still image. AnchorX/AnchorY is the ground contact
/// point in sprite pixels (may lie outside the opaque bounds).
/// </summary>
internal sealed record NativeSprite(int Width, int Height, uint[] ArgbPixels, int AnchorX, int AnchorY, string AssetName);

/// <summary>
/// Resolves map object names to still frames of the original ALR/APT assets.
/// Frame choice follows layouts verified against decoded game assets (2026-10-07):
/// ALR frames are ((animation * directions) + direction) * LayoutColumns + frame with the
/// ground point at the anchor canvas centre; APT layout axis 0 is the construction stage,
/// so the last stage with all other axes 0 is the intact, finished building.
/// In-game template matching (ENDL_005, 2026-10-07) confirmed the finished APT frame, the
/// palette channel order, team N = palette variant N and the ground anchor; the general
/// scenario-angle to direction-row mapping is still unknown beyond Angle 0 = row 14.
/// </summary>
internal sealed class NativeSpriteCatalog : IDisposable
{
    private readonly Dictionary<string, NativeSpriteDefinition> _definitions;
    private readonly IReadOnlyDictionary<int, string> _alrNames, _aptNames;
    private readonly Func<string, byte[]?> _readAlr, _readApt;
    private readonly IDisposable? _owner;
    private readonly Dictionary<string, NativeAlrDocument?> _alrDocuments = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, NativeAptDocument?> _aptDocuments = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<(string Name, int Variant, int Direction), NativeSprite?> _sprites = new();
    private readonly object _gate = new();

    private NativeSpriteCatalog(Dictionary<string, NativeSpriteDefinition> definitions,
        IReadOnlyDictionary<int, string> alrNames, IReadOnlyDictionary<int, string> aptNames,
        Func<string, byte[]?> readAlr, Func<string, byte[]?> readApt, IDisposable? owner)
    {
        _definitions = definitions; _alrNames = alrNames; _aptNames = aptNames;
        _readAlr = readAlr; _readApt = readApt; _owner = owner;
    }

    public int DefinitionCount => _definitions.Count;

    public static NativeSpriteCatalog FromText(string objdefText, string alrListText, string aptListText,
        Func<string, byte[]?> readAlr, Func<string, byte[]?> readApt, IDisposable? owner = null)
        => new(ParseObjdef(objdefText), ParseNameList(alrListText), ParseNameList(aptListText), readAlr, readApt, owner);

    /// <summary>
    /// Open the game's sprite sources read-only. Returns null when any required file is missing;
    /// archives stay open (shared read) until the catalog is disposed.
    /// </summary>
    public static NativeSpriteCatalog? Open(string gamePath)
    {
        string objdef = Path.Combine(gamePath, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau");
        string alrList = Path.Combine(gamePath, "SYSTEM", "cl_alr.ini"), aptList = Path.Combine(gamePath, "SYSTEM", "cl_apt.ini");
        string alrPath = Path.Combine(gamePath, "alr.dat"), aptPath = Path.Combine(gamePath, "apt.dat");
        if (!new[] { objdef, alrList, aptList, alrPath, aptPath }.All(File.Exists)) return null;
        var archives = new ArchivePair(OpenArchive(alrPath), OpenArchive(aptPath));
        try
        {
            return FromText(ReadGameText(objdef), ReadGameText(alrList), ReadGameText(aptList),
                name => archives.Read(archives.Alr, "SYSTEM/DATA/ALR/" + name),
                name => archives.Read(archives.Apt, "SYSTEM/DATA/APT/" + name), archives);
        }
        catch { archives.Dispose(); throw; }
    }

    public bool TryGetDefinition(string name, out NativeSpriteDefinition definition)
        => _definitions.TryGetValue(name.Trim(), out definition!);

    /// <summary>
    /// Direction row a unit spawned with scenario Angle 0 shows in game (template-matched against
    /// gerinf01 in-game, 2026-10-07). Taken modulo each asset's row count, so single-row assets use row 0.
    /// </summary>
    public const int DefaultDirection = 14;

    /// <summary>Still sprite for a map object name; null when the object has no decodable native asset.</summary>
    public NativeSprite? GetSprite(string objectName, int team = 0, int direction = DefaultDirection)
    {
        if (!TryGetDefinition(objectName, out NativeSpriteDefinition? definition)) return null;
        lock (_gate)
        {
            // Trees and similar objects pair a base ALR (trunk) with an alrml layer (crown) drawn over it;
            // objects with only alrml (ground cover) use that layer alone.
            int primary = definition.AlrId >= 0 ? definition.AlrId : definition.LayerAlrId;
            int overlay = definition.AlrId >= 0 ? definition.LayerAlrId : -1;
            if (primary >= 0 && _alrNames.TryGetValue(primary, out string? alrName))
            {
                NativeAlrDocument? document = LoadAlr(alrName);
                if (document is null) return null;
                string? overlayName = overlay >= 0 && overlay != primary && _alrNames.TryGetValue(overlay, out string? layer) ? layer : null;
                NativeAlrDocument? overlayDocument = overlayName is null ? null : LoadAlr(overlayName);
                // palty 1 objects carry per-team palette variants; others use the first palette.
                int variant = definition.PaletteType == 1 ? Math.Clamp(team, 0, document.PaletteVariantCount - 1) : 0;
                int directions = (int)Math.Max(1, document.LayoutRows);
                int dir = ((direction % directions) + directions) % directions;
                string assetName = overlayDocument is null ? alrName : alrName + "+" + overlayName;
                var key = (assetName, variant, dir);
                if (!_sprites.TryGetValue(key, out NativeSprite? sprite))
                    _sprites[key] = sprite = DecodeAlr(assetName, document, overlayDocument, variant, dir);
                return sprite;
            }
            if (definition.AptIndex >= 0 && _aptNames.TryGetValue(definition.AptIndex, out string? aptName))
            {
                NativeAptDocument? document = LoadApt(aptName);
                if (document is null) return null;
                var key = (aptName, 0, 0);
                if (!_sprites.TryGetValue(key, out NativeSprite? sprite))
                    _sprites[key] = sprite = DecodeApt(aptName, document);
                return sprite;
            }
            return null;
        }
    }

    internal static int AlrStillFrame(NativeAlrDocument document, int direction)
    {
        long frame = (long)direction * Math.Max(1, document.LayoutColumns);
        return frame < document.Frames.Count ? (int)frame : 0;
    }

    internal static int AptFinishedFrame(IReadOnlyList<uint> layout, int frameCount)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(frameCount);
        long product = 1;
        foreach (uint size in layout) product *= Math.Max(1u, size);
        uint stages = layout.Count > 0 ? Math.Max(1u, layout[0]) : 1u;
        long frame = (stages - 1) * (product / stages);
        return frame < frameCount ? (int)frame : frameCount - 1;
    }

    private static NativeSprite? DecodeAlr(string name, NativeAlrDocument document, NativeAlrDocument? overlay, int variant, int direction)
    {
        Layer? bottomLayer = DecodeAlrLayer(document, variant, direction);
        if (bottomLayer is null) return null;
        Layer? topLayer = overlay is null ? null : DecodeAlrLayer(overlay, variant, direction);
        if (topLayer is null)
            return Crop(name.Split('+')[0], bottomLayer.Width, bottomLayer.Height, bottomLayer.Pixels, bottomLayer.AnchorX, bottomLayer.AnchorY);
        // Union of both layers in ground-anchored coordinates; the overlay's opaque pixels win.
        int left = Math.Min(-bottomLayer.AnchorX, -topLayer.AnchorX), top = Math.Min(-bottomLayer.AnchorY, -topLayer.AnchorY);
        int right = Math.Max(bottomLayer.Width - bottomLayer.AnchorX, topLayer.Width - topLayer.AnchorX);
        int bottom = Math.Max(bottomLayer.Height - bottomLayer.AnchorY, topLayer.Height - topLayer.AnchorY);
        int width = right - left, height = bottom - top;
        var pixels = new uint[width * height];
        foreach (Layer layer in new[] { bottomLayer, topLayer })
        {
            int ox = -layer.AnchorX - left, oy = -layer.AnchorY - top;
            for (int y = 0; y < layer.Height; y++)
                for (int x = 0; x < layer.Width; x++)
                {
                    uint color = layer.Pixels[y * layer.Width + x];
                    if (color >> 24 != 0) pixels[(oy + y) * width + ox + x] = color;
                }
        }
        return Crop(name, width, height, pixels, -left, -top);
    }

    private sealed record Layer(int Width, int Height, IReadOnlyList<uint> Pixels, int AnchorX, int AnchorY);

    private static Layer? DecodeAlrLayer(NativeAlrDocument document, int variant, int direction)
    {
        try
        {
            int index = AlrStillFrame(document, direction % (int)Math.Max(1, document.LayoutRows));
            NativeAlrFrameInfo info = document.Frames[index];
            NativeAlrIndexedFrame frame = document.DecodeFrame(index, Math.Min(variant, document.PaletteVariantCount - 1));
            // Ground contact is the anchor canvas centre; frame pixels are placed at their stored offset.
            return new Layer(frame.Width, frame.Height, frame.ArgbPixels,
                (int)document.AnchorWidth / 2 - info.OffsetX, (int)document.AnchorHeight / 2 - info.OffsetY);
        }
        catch (Exception error) when (error is InvalidDataException or NotSupportedException or ArgumentException) { return null; }
    }

    private static NativeSprite? DecodeApt(string name, NativeAptDocument document)
    {
        try
        {
            NativeAptIndexedImage image = document.DecodeFrame(AptFinishedFrame(document.Layout, document.Frames.Count));
            return Crop(name, image.Width, image.Height, image.ArgbPixels, document.AnchorX, document.AnchorY);
        }
        catch (Exception error) when (error is InvalidDataException or NotSupportedException or ArgumentException) { return null; }
    }

    internal static NativeSprite? Crop(string name, int width, int height, IReadOnlyList<uint> pixels, int anchorX, int anchorY)
    {
        int minX = width, minY = height, maxX = -1, maxY = -1;
        for (int y = 0; y < height; y++)
            for (int x = 0; x < width; x++)
            {
                if (pixels[y * width + x] >> 24 == 0) continue;
                minX = Math.Min(minX, x); maxX = Math.Max(maxX, x); minY = Math.Min(minY, y); maxY = Math.Max(maxY, y);
            }
        if (maxX < 0) return null;
        int w = maxX - minX + 1, h = maxY - minY + 1;
        var cropped = new uint[w * h];
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++) cropped[y * w + x] = pixels[(y + minY) * width + x + minX];
        return new NativeSprite(w, h, cropped, anchorX - minX, anchorY - minY, name);
    }

    private NativeAlrDocument? LoadAlr(string name)
    {
        if (_alrDocuments.TryGetValue(name, out NativeAlrDocument? cached)) return cached;
        NativeAlrDocument? document = null;
        try { byte[]? bytes = _readAlr(name); if (bytes is not null) document = NativeAlrDocument.Parse(bytes); }
        catch (Exception error) when (error is InvalidDataException or NotSupportedException) { }
        return _alrDocuments[name] = document;
    }

    private NativeAptDocument? LoadApt(string name)
    {
        if (_aptDocuments.TryGetValue(name, out NativeAptDocument? cached)) return cached;
        NativeAptDocument? document = null;
        try { byte[]? bytes = _readApt(name); if (bytes is not null) document = NativeAptDocument.Parse(bytes); }
        catch (Exception error) when (error is InvalidDataException or NotSupportedException) { }
        return _aptDocuments[name] = document;
    }

    internal static Dictionary<string, NativeSpriteDefinition> ParseObjdef(string text)
    {
        var result = new Dictionary<string, NativeSpriteDefinition>(StringComparer.OrdinalIgnoreCase);
        foreach (string line in text.Split('\n'))
        {
            if (line.Length < 100 || line.TrimStart().StartsWith(';') || line.StartsWith('[')) continue;
            string[] columns = line.Split(',');
            if (columns.Length <= 52 || !Int(columns[0], out int id) || !Int(columns[5], out int alr) ||
                !Int(columns[14], out int apt) || !Int(columns[17], out int palette)) continue;
            int layer = Int(columns[8], out int parsedLayer) ? parsedLayer : -1;
            int layerType = Int(columns[10], out int parsedType) ? parsedType : -1;
            string name = columns[52].Trim();
            if (name.Length > 0) result.TryAdd(name, new NativeSpriteDefinition(id, name, alr, apt, palette, layer, layerType));
        }
        return result;
    }

    internal static Dictionary<int, string> ParseNameList(string text)
    {
        var result = new Dictionary<int, string>();
        foreach (string line in text.Split('\n'))
        {
            string[] columns = line.Trim().Split(',');
            if (columns.Length > 1 && Int(columns[0], out int id) && columns[1].Trim().Length > 0) result.TryAdd(id, columns[1].Trim());
        }
        return result;
    }

    private static bool Int(string value, out int result)
        => int.TryParse(value.Trim(), NumberStyles.AllowLeadingSign, CultureInfo.InvariantCulture, out result);

    private static string ReadGameText(string path)
    {
        byte[] bytes = File.ReadAllBytes(path);
        if (bytes.Length >= 4 && bytes.AsSpan(0, 4).SequenceEqual("PFIL"u8)) bytes = GameLZSS.DecompressPfil(bytes);
        return MapTextEncoding.Game.GetString(bytes);
    }

    private static ZipArchive OpenArchive(string path)
        => new(new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete), ZipArchiveMode.Read);

    public void Dispose() { lock (_gate) { _owner?.Dispose(); _alrDocuments.Clear(); _aptDocuments.Clear(); _sprites.Clear(); } }

    private sealed class ArchivePair(ZipArchive alr, ZipArchive apt) : IDisposable
    {
        public ZipArchive Alr { get; } = alr;
        public ZipArchive Apt { get; } = apt;
        public byte[]? Read(ZipArchive archive, string path)
        {
            ZipArchiveEntry? entry = archive.GetEntry(path);
            if (entry is null) return null;
            using Stream stream = entry.Open(); using var buffer = new MemoryStream();
            stream.CopyTo(buffer);
            return buffer.ToArray();
        }
        public void Dispose() { Alr.Dispose(); Apt.Dispose(); }
    }
}
