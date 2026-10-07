using System.Buffers.Binary;
using System.IO.Compression;
using System.Text.Json;
using AgainstRomeMapEditor.NativeAssets;

if (args.Length != 3)
{
    Console.Error.WriteLine("Usage: anim-probe <alr.dat> <apt.dat> <NEW TEMP output dir>");
    return 2;
}

string alrPath = Path.GetFullPath(args[0]);
string aptPath = Path.GetFullPath(args[1]);
string outDir = Path.GetFullPath(args[2]);

string temp = Path.TrimEndingDirectorySeparator(Path.GetFullPath(Path.GetTempPath())) + Path.DirectorySeparatorChar;
if (!outDir.StartsWith(temp, StringComparison.OrdinalIgnoreCase) || Directory.Exists(outDir) || File.Exists(outDir))
{
    Console.Error.WriteLine("Output must be a NEW child directory of TEMP; existing directories are rejected.");
    return 2;
}

Directory.CreateDirectory(outDir);
Console.WriteLine($"Exporting probe artifacts to {outDir}");

// 1. Process ALR unit animation (gersch01.alr)
using (var alrStream = new FileStream(alrPath, FileMode.Open, FileAccess.Read, FileShare.Read))
using (var alrZip = new ZipArchive(alrStream, ZipArchiveMode.Read))
{
    var entry = alrZip.GetEntry("SYSTEM/DATA/ALR/gersch01.alr") ?? throw new FileNotFoundException("gersch01.alr not found");
    using var mem = new MemoryStream();
    using (var es = entry.Open()) es.CopyTo(mem);
    var doc = NativeAlrDocument.Parse(mem.ToArray());
    Console.WriteLine($"gersch01.alr: {doc.Frames.Count} frames, {doc.LayoutColumns} cols, {doc.LayoutRows} rows, {doc.PaletteVariantCount} variants");

    // Animation 0: Idle/standing (24 frames for direction 14)
    // Animation 1: Walk (24 frames for direction 14)
    int dir = 14;
    int cols = (int)doc.LayoutColumns;
    int rows = (int)doc.LayoutRows;
    int animCount = doc.Frames.Count / (cols * rows);

    // Export contact sheet for Anim 0 (Idle) direction 14, 24 frames
    // Contact sheet: 6 columns x 4 rows
    int cellW = (int)doc.AnchorWidth;
    int cellH = (int)doc.AnchorHeight;
    int sheetCols = 6, sheetRows = 4;
    uint[] sheet0 = new uint[cellW * sheetCols * cellH * sheetRows];
    Array.Fill(sheet0, 0x00000000u); // transparent background

    for (int f = 0; f < cols; f++)
    {
        int frameIdx = ((0 * rows + dir) * cols) + f;
        var decoded = doc.DecodeFrame(frameIdx, 0); // team 0
        var info = doc.Frames[frameIdx];

        int cx = (f % sheetCols) * cellW;
        int cy = (f / sheetCols) * cellH;

        // Blit frame into cell at (cx + info.OffsetX, cy + info.OffsetY)
        for (int py = 0; py < decoded.Height; py++)
        {
            for (int px = 0; px < decoded.Width; px++)
            {
                uint col = decoded.ArgbPixels[py * decoded.Width + px];
                if ((col >> 24) > 0)
                {
                    int dx = cx + info.OffsetX + px;
                    int dy = cy + info.OffsetY + py;
                    if (dx >= 0 && dx < cellW * sheetCols && dy >= 0 && dy < cellH * sheetRows)
                    {
                        sheet0[dy * (cellW * sheetCols) + dx] = col;
                    }
                }
            }
        }
    }
    PngWriter.Write(Path.Combine(outDir, "gersch01_anim0_idle_sheet.png"), cellW * sheetCols, cellH * sheetRows, sheet0);
    Console.WriteLine("Exported gersch01_anim0_idle_sheet.png");

    // Also export a palette contact sheet for direction 14 frame 0 across all 9 variants!
    // 9 cells horizontally: 9 x cellW, 1 x cellH
    uint[] teamSheet = new uint[cellW * 9 * cellH];
    Array.Fill(teamSheet, 0x00000000u);
    for (int v = 0; v < 9; v++)
    {
        int frameIdx = ((0 * rows + dir) * cols) + 0;
        var decoded = doc.DecodeFrame(frameIdx, v);
        var info = doc.Frames[frameIdx];
        int cx = v * cellW;
        int cy = 0;
        for (int py = 0; py < decoded.Height; py++)
        {
            for (int px = 0; px < decoded.Width; px++)
            {
                uint col = decoded.ArgbPixels[py * decoded.Width + px];
                if ((col >> 24) > 0)
                {
                    int dx = cx + info.OffsetX + px;
                    int dy = cy + info.OffsetY + py;
                    if (dx >= 0 && dx < cellW * 9 && dy >= 0 && dy < cellH)
                    {
                        teamSheet[dy * (cellW * 9) + dx] = col;
                    }
                }
            }
        }
    }
    PngWriter.Write(Path.Combine(outDir, "gersch01_team_variants_sheet.png"), cellW * 9, cellH, teamSheet);
    Console.WriteLine("Exported gersch01_team_variants_sheet.png");
}

// 2. Process APT building animation (gerhau02.apt)
using (var aptStream = new FileStream(aptPath, FileMode.Open, FileAccess.Read, FileShare.Read))
using (var aptZip = new ZipArchive(aptStream, ZipArchiveMode.Read))
{
    var entry = aptZip.GetEntry("SYSTEM/DATA/APT/gerhau02.apt") ?? throw new FileNotFoundException("gerhau02.apt not found");
    using var mem = new MemoryStream();
    using (var es = entry.Open()) es.CopyTo(mem);
    var doc = NativeAptDocument.Parse(mem.ToArray());
    Console.WriteLine($"gerhau02.apt: {doc.Frames.Count} frames, layout {doc.Layout[0]}x{doc.Layout[1]}x{doc.Layout[2]}x{doc.Layout[3]}");

    // Let's compare Axis 1 = 0 vs Axis 1 = 1:
    // a = 4 (constructed), c = 0 (undamaged), d = 0 (frame 0)
    // idx_b0 = (((4 * 2) + 0) * 5 + 0) * 25 + 0 = 1000
    // idx_b1 = (((4 * 2) + 1) * 5 + 0) * 25 + 0 = 1125
    var frameB0 = doc.DecodeFrame(1000, 0);
    var frameB1 = doc.DecodeFrame(1125, 0);

    PngWriter.Write(Path.Combine(outDir, "gerhau02_axis1_val0.png"), frameB0.Width, frameB0.Height, frameB0.ArgbPixels.ToArray());
    PngWriter.Write(Path.Combine(outDir, "gerhau02_axis1_val1.png"), frameB1.Width, frameB1.Height, frameB1.ArgbPixels.ToArray());

    // Compare pixel diff between axis1=0 and axis1=1
    int diffCount = 0;
    for (int i = 0; i < frameB0.ArgbPixels.Count; i++)
    {
        if (frameB0.ArgbPixels[i] != frameB1.ArgbPixels[i]) diffCount++;
    }
    Console.WriteLine($"gerhau02 frame 1000 vs 1125 pixel diffs: {diffCount} / {frameB0.ArgbPixels.Count}");

    // Export building torch/flag animation contact sheet (25 frames, a=4, b=0, c=0, d=0..24)
    // 5 cols x 5 rows
    int cropW = 200, cropH = 200; // crop around torch area or center
    int cx0 = 280, cy0 = 200; // torch area near house roof
    int sheetW = cropW * 5;
    int sheetH = cropH * 5;
    uint[] animSheet = new uint[sheetW * sheetH];
    Array.Fill(animSheet, 0xFF202020u); // dark gray bg

    for (int d = 0; d < 25; d++)
    {
        int fIdx = (((4 * 2) + 0) * 5 + 0) * 25 + d;
        var f = doc.DecodeFrame(fIdx, 0);
        int sx = (d % 5) * cropW;
        int sy = (d / 5) * cropH;
        for (int py = 0; py < cropH; py++)
        {
            for (int px = 0; px < cropW; px++)
            {
                int srcX = cx0 + px;
                int srcY = cy0 + py;
                if (srcX >= 0 && srcX < f.Width && srcY >= 0 && srcY < f.Height)
                {
                    uint col = f.ArgbPixels[srcY * f.Width + srcX];
                    if ((col >> 24) > 0)
                    {
                        animSheet[(sy + py) * sheetW + (sx + px)] = col;
                    }
                }
            }
        }
    }
    PngWriter.Write(Path.Combine(outDir, "gerhau02_anim_torch_sheet.png"), sheetW, sheetH, animSheet);
    Console.WriteLine("Exported gerhau02_anim_torch_sheet.png");
}

File.WriteAllText(Path.Combine(outDir, "probe_summary.json"), JsonSerializer.Serialize(new
{
    status = "SUCCESS",
    unit_anim = "gersch01_anim0_idle_sheet.png",
    team_variants = "gersch01_team_variants_sheet.png",
    apt_axis1_val0 = "gerhau02_axis1_val0.png",
    apt_axis1_val1 = "gerhau02_axis1_val1.png",
    apt_anim = "gerhau02_anim_torch_sheet.png"
}, new JsonSerializerOptions { WriteIndented = true }));

Console.WriteLine("Probe complete successfully.");
return 0;
