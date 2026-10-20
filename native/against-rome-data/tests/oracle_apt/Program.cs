using System.Globalization;
using AgainstRomeMapEditor.NativeAssets;

if (args.Length < 1)
{
    Console.WriteLine("Usage: AptOracle <cases_directory> | --geom <input_file> <output_file>");
    return 1;
}

if (args[0] == "--geom")
{
    if (args.Length < 3)
    {
        Console.Error.WriteLine("Missing arguments for --geom");
        return 1;
    }
    string[] lines = File.ReadAllLines(args[1]);
    var outLines = new List<string>();
    foreach (string line in lines)
    {
        string trimmed = line.Trim();
        if (string.IsNullOrEmpty(trimmed)) continue;
        float val;
        if (trimmed == "NaN") val = float.NaN;
        else if (trimmed == "Infinity" || trimmed == "+Infinity") val = float.PositiveInfinity;
        else if (trimmed == "-Infinity") val = float.NegativeInfinity;
        else val = float.Parse(trimmed, CultureInfo.InvariantCulture);

        try
        {
            float res = NativeAptAttachmentGeometry.WorldGridOrigin(val);
            outLines.Add($"OK {res.ToString("R", CultureInfo.InvariantCulture)}");
        }
        catch (Exception ex)
        {
            outLines.Add($"REJECT {ex.GetType().Name}");
        }
    }
    File.WriteAllLines(args[2], outLines);
    return 0;
}

string casesDir = Path.GetFullPath(args[0]);
if (!Directory.Exists(casesDir))
{
    Console.Error.WriteLine($"Directory not found: {casesDir}");
    return 1;
}

var inputFiles = Directory.GetFiles(casesDir, "*.in").OrderBy(f => f, StringComparer.Ordinal).ToArray();
Console.WriteLine($"Found {inputFiles.Length} input files in {casesDir}");

foreach (string path in inputFiles)
{
    string name = Path.GetFileNameWithoutExtension(path);
    byte[] source = File.ReadAllBytes(path);
    string statusFile = Path.Combine(casesDir, $"{name}.status");

    NativeAptDocument doc;
    try
    {
        doc = NativeAptDocument.Parse(source);
    }
    catch (Exception ex)
    {
        File.WriteAllText(statusFile, $"REJECT {ex.GetType().Name}\n{ex.Message}");
        continue;
    }

    File.WriteAllText(statusFile, "OK");

    // 1. Meta
    var metaLines = new List<string>
    {
        $"DOC {doc.Version} {doc.Width} {doc.Height} {doc.AnchorX} {doc.AnchorY} {doc.ProjectionOrigin.X} {doc.ProjectionOrigin.Y} {doc.Layout[0]} {doc.Layout[1]} {doc.Layout[2]} {doc.Layout[3]} {doc.PaletteColorCount} {doc.PaletteVariantCount} {doc.Frames.Count} {doc.Tiles.Count} {doc.AttachmentGroups.Count}"
    };

    for (int i = 0; i < doc.Frames.Count; i++)
    {
        var f = doc.Frames[i];
        metaLines.Add($"FRAME {i} {f.FirstTile} {f.TileCount}");
    }

    for (int i = 0; i < doc.Tiles.Count; i++)
    {
        var t = doc.Tiles[i];
        metaLines.Add($"TILE {i} {t.DataOffset} {t.PackedPosition} {t.AuxiliaryBytes} {t.X} {t.Y}");
    }

    for (int i = 0; i < doc.AttachmentGroups.Count; i++)
    {
        var g = doc.AttachmentGroups[i];
        metaLines.Add($"GROUP {i} Marker={g.Marker.X},{g.Marker.Y} LightVariantsCount={g.LightVariants.Count} SmokeCount={g.SmokePoints.Count}");
        for (int v = 0; v < g.LightVariants.Count; v++)
        {
            var lts = g.LightVariants[v];
            string pts = lts.Count == 0 ? "" : " " + string.Join(" ", lts.Select(p => $"{p.X},{p.Y}"));
            metaLines.Add($"GROUP_LIGHTS {i} {v} {lts.Count}{pts}");
        }
        string smk = g.SmokePoints.Count == 0 ? "" : " " + string.Join(" ", g.SmokePoints.Select(p => $"{p.X},{p.Y}"));
        metaLines.Add($"GROUP_SMOKE {i} {g.SmokePoints.Count}{smk}");
    }
    File.WriteAllLines(Path.Combine(casesDir, $"{name}.meta"), metaLines);

    // 2. Attachment Queries
    var queryLines = new List<string>();
    uint[][] queryAxes =
    [
        [0, 0, 0],
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 1, 0],
        [1, 1, 1],
        [2, 0, 0],
        [0, 2, 0],
        [0, 0, 2],
        [uint.MaxValue, 0, uint.MaxValue],
        [100, 100, 100],
    ];

    foreach (var ax in queryAxes)
    {
        uint a0 = ax[0], a1 = ax[1], a2 = ax[2];
        try
        {
            var (lights, smoke, marker) = doc.GetAttachments(a0, a1, a2);
            string markerStr = marker == null ? "NONE" : $"{marker.Value.X},{marker.Value.Y}";
            string lightsStr = string.Join(";", lights.Select(p => $"{p.X},{p.Y}"));
            string smokeStr = string.Join(";", smoke.Select(p => $"{p.X},{p.Y}"));
            queryLines.Add($"QUERY {a0} {a1} {a2} OK Marker={markerStr} Lights={lights.Count}:{lightsStr} Smoke={smoke.Count}:{smokeStr}");
        }
        catch (Exception ex)
        {
            queryLines.Add($"QUERY {a0} {a1} {a2} REJECT {ex.GetType().Name}");
        }
    }
    File.WriteAllLines(Path.Combine(casesDir, $"{name}.queries"), queryLines);

    // 3. Decode Tiles
    for (int t = 0; t < doc.Tiles.Count; t++)
    {
        for (int v = 0; v < doc.PaletteVariantCount; v++)
        {
            try
            {
                var img = doc.DecodeTile(t, v);
                using var stream = File.Create(Path.Combine(casesDir, $"{name}.tile_{t}_{v}.bin"));
                using var bw = new BinaryWriter(stream);
                bw.Write(img.Width);
                bw.Write(img.Height);
                foreach (var p in img.ArgbPixels)
                {
                    bw.Write(p);
                }
            }
            catch (Exception ex)
            {
                File.WriteAllText(Path.Combine(casesDir, $"{name}.tile_{t}_{v}.err"), $"REJECT {ex.GetType().Name}");
            }
        }
    }

    // 4. Decode Frames
    for (int f = 0; f < doc.Frames.Count; f++)
    {
        for (int v = 0; v < doc.PaletteVariantCount; v++)
        {
            try
            {
                var img = doc.DecodeFrame(f, v);
                using var stream = File.Create(Path.Combine(casesDir, $"{name}.frame_{f}_{v}.bin"));
                using var bw = new BinaryWriter(stream);
                bw.Write(img.Width);
                bw.Write(img.Height);
                foreach (var p in img.ArgbPixels)
                {
                    bw.Write(p);
                }
            }
            catch (Exception ex)
            {
                File.WriteAllText(Path.Combine(casesDir, $"{name}.frame_{f}_{v}.err"), $"REJECT {ex.GetType().Name}");
            }
        }
    }

    // 5. Geometry & Lights
    var geomLines = new List<string>();
    NativeAptPoint[] testPoints =
    [
        new(0, 0),
        new(63, 0),
        new(64, 0),
        new(-1, 0),
        new(0, -1),
        new(65, 0),
        new(doc.AnchorX, doc.AnchorY),
        new(doc.ProjectionOrigin.X, doc.ProjectionOrigin.Y),
        new(-100, 250),
        new(1024, -512)
    ];

    foreach (var pt in testPoints)
    {
        var proj = NativeAptAttachmentGeometry.Project(doc, pt);
        geomLines.Add($"PROJECT {pt.X} {pt.Y} -> {proj.X} {proj.Y}");
    }

    (NativeAptPoint Pt, float Px, float Pz, float H)[] placeCases =
    [
        (new(0, 0), 0f, 0f, 80f),
        (new(65, 0), 0f, 0f, 80f),
        (new(doc.AnchorX, doc.AnchorY), 100f, 200f, 50f),
        (new(10, -20), -64.5f, 63.9f, 12.5f)
    ];

    foreach (var (pt, px, pz, h) in placeCases)
    {
        try
        {
            var v = NativeAptAttachmentGeometry.Place(doc, pt, px, pz, h);
            geomLines.Add($"PLACE {pt.X} {pt.Y} {px.ToString("R", CultureInfo.InvariantCulture)} {pz.ToString("R", CultureInfo.InvariantCulture)} {h.ToString("R", CultureInfo.InvariantCulture)} OK {v.X.ToString("R", CultureInfo.InvariantCulture)} {v.Y.ToString("R", CultureInfo.InvariantCulture)} {v.Z.ToString("R", CultureInfo.InvariantCulture)}");
        }
        catch (Exception ex)
        {
            geomLines.Add($"PLACE {pt.X} {pt.Y} {px} {pz} {h} REJECT {ex.GetType().Name}");
        }
    }

    try
    {
        var pts = NativeAptLightPoints.ExtractLightPoints(source);
        string s = pts.Count == 0 ? "" : " " + string.Join(";", pts.Select(p => $"{p.ScreenX},{p.ScreenY},{p.DeltaAnchorX},{p.DeltaAnchorY}"));
        geomLines.Add($"EXTRACT_LIGHTS OK {pts.Count}{s}");
    }
    catch (Exception ex)
    {
        geomLines.Add($"EXTRACT_LIGHTS REJECT {ex.GetType().Name}");
    }

    try
    {
        var vlist = NativeAptLightPoints.GetBuildingWorldLightPositions(source, 100f, 200f, 50f, 10f);
        string s = vlist.Count == 0 ? "" : " " + string.Join(";", vlist.Select(v => $"{v.X.ToString("R", CultureInfo.InvariantCulture)},{v.Y.ToString("R", CultureInfo.InvariantCulture)},{v.Z.ToString("R", CultureInfo.InvariantCulture)}"));
        geomLines.Add($"BUILDING_LIGHTS OK {vlist.Count}{s}");
    }
    catch (Exception ex)
    {
        geomLines.Add($"BUILDING_LIGHTS REJECT {ex.GetType().Name}");
    }

    File.WriteAllLines(Path.Combine(casesDir, $"{name}.geom"), geomLines);
}

return 0;
