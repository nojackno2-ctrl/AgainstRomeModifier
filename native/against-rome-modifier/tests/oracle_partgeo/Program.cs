using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using AgainstRomeModifier;
using AgainstRomeModifier.Core.Patches;

namespace PartgeoOracle;

public static class Program
{
    public static void Main(string[] args)
    {
        if (args.Length != 1)
        {
            Console.Error.WriteLine("Expected output fixture path argument");
            Environment.Exit(1);
        }

        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        var cp1251 = Encoding.GetEncoding(1251);

        var sources = new Dictionary<string, string>
        {
            ["PartgeoPatcher"] = Convert.ToHexString(SHA256.HashData(File.ReadAllBytes("src.Core/Core/Patches/PartgeoPatcher.cs"))),
            ["PatchText"] = Convert.ToHexString(SHA256.HashData(File.ReadAllBytes("src.Core/Core/Patches/PatchText.cs"))),
            ["GameLZSS"] = Convert.ToHexString(SHA256.HashData(File.ReadAllBytes("src.Shared/Core/GameLZSS.cs")))
        };

        byte[] MakePfil(string text, string? headerTemplateStr = null)
        {
            byte[] header = new byte[64];
            header[0] = (byte)'P'; header[1] = (byte)'F'; header[2] = (byte)'I'; header[3] = (byte)'L';
            if (headerTemplateStr != null)
            {
                byte[] custom = cp1251.GetBytes(headerTemplateStr);
                Array.Copy(custom, 0, header, 4, Math.Min(custom.Length, 60));
            }
            return GameLZSS.CompressPfil(cp1251.GetBytes(text), header);
        }

        string Row(int idx, string name, string ysub, int totalCols = 16)
        {
            var cols = new List<string>();
            for (int i = 0; i < totalCols; i++) cols.Add("        0");
            cols[0] = idx.ToString().PadLeft(5);
            cols[1] = "    1";
            cols[2] = name.PadLeft(30);
            if (cols.Count > 12) cols[12] = ysub;
            return string.Join(",", cols);
        }

        string Standard7Rows() =>
            Row(0, "Rauch", "  -983040") + "\r\n" +
            Row(20, "Wurfspeer00", "  5832704") + "\r\n" +
            Row(21, "Wurfaxt00", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n" +
            Row(75, "Fackel", "  8126464") + "\r\n";

        var testCases = new List<(string Name, byte[] Input, bool ProjectileArcHeight)>();

        // 1. Retail baseline
        string retailPath = "re_workspace/cd-modifier-baseline-20261010/files/SYSTEM/DATA_MP/DEFAULTS/partgeo.dau";
        if (File.Exists(retailPath))
        {
            byte[] retailBytes = File.ReadAllBytes(retailPath);
            testCases.Add(("retail-partgeo-arc-true", retailBytes, true));
            testCases.Add(("retail-partgeo-arc-false", retailBytes, false));
        }

        // 2. Representative synthetic cases
        string standardText = "[ParticleGeometrieDefault]\r\n;header comment\r\n" + Standard7Rows();
        byte[] standardPfil = MakePfil(standardText);
        testCases.Add(("synthetic-standard-arc-true", standardPfil, true));
        testCases.Add(("synthetic-standard-arc-false", standardPfil, false));

        // Line ending: LF only
        string lfText = standardText.Replace("\r\n", "\n");
        testCases.Add(("synthetic-lf-arc-true", MakePfil(lfText), true));

        // No trailing newline
        string noTrailingNewline = standardText.TrimEnd('\r', '\n');
        testCases.Add(("synthetic-no-trailing-newline-arc-true", MakePfil(noTrailingNewline), true));

        // Scrambled order with extra non-projectile rows
        string scrambledText = "[ParticleGeometrieDefault]\r\n" +
            Row(75, "Fackel", "  8126464") + "\r\n" +
            Row(1, "Funken00", "        0") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n" +
            Row(2, "Blut00", "        0") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(21, "Wurfaxt00", "  5832704") + "\r\n" +
            Row(20, "Wurfspeer00", "  5832704") + "\r\n";
        testCases.Add(("synthetic-scrambled-with-other-rows-arc-true", MakePfil(scrambledText), true));

        // CP1251 Cyrillic comments and names in non-projectile rows
        string cyrillicText = "; Комментарий к файлу геометрии частиц\r\n[ParticleGeometrieDefault]\r\n" +
            Row(0, "Огонь00", "        0") + "\r\n" +
            Standard7Rows();
        testCases.Add(("synthetic-cyrillic-arc-true", MakePfil(cyrillicText), true));

        // Exact width fit (e.g. width 10, ysub 4000000000 -> 8000000000, exactly 10 digits)
        string exactFitText = "[ParticleGeometrieDefault]\r\n" +
            Row(0, "Rauch", "  -983040") + "\r\n" +
            Row(20, "Wurfspeer00", "4000000000") + "\r\n" +
            Row(21, "Wurfaxt00", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n" +
            Row(75, "Fackel", "  8126464") + "\r\n";
        testCases.Add(("synthetic-exact-width-fit-arc-true", MakePfil(exactFitText), true));

        // Leading plus sign on ysub
        string leadingPlusText = "[ParticleGeometrieDefault]\r\n" +
            Row(0, "Rauch", "  -983040") + "\r\n" +
            Row(20, "Wurfspeer00", " +5832704") + "\r\n" +
            Row(21, "Wurfaxt00", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n" +
            Row(75, "Fackel", "  8126464") + "\r\n";
        testCases.Add(("synthetic-leading-plus-arc-true", MakePfil(leadingPlusText), true));

        // Whitespace around projectile names
        string spacedNameText = "[ParticleGeometrieDefault]\r\n" +
            Row(0, "Rauch", "  -983040") + "\r\n" +
            Row(20, "  Wurfspeer00  ", "  5832704") + "\r\n" +
            Row(21, "\tWurfaxt00 ", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n" +
            Row(75, "Fackel", "  8126464") + "\r\n";
        testCases.Add(("synthetic-spaced-names-arc-true", MakePfil(spacedNameText), true));

        // Many columns (e.g. 25 columns)
        string manyColsText = "[ParticleGeometrieDefault]\r\n" +
            Row(20, "Wurfspeer00", "  5832704", 25) + "\r\n" +
            Row(21, "Wurfaxt00", "  5832704", 25) + "\r\n" +
            Row(22, "Katapultstein00", "  8978432", 25) + "\r\n" +
            Row(23, "Pfeil00", "  5832704", 25) + "\r\n" +
            Row(45, "Spiess00", "  5832704", 25) + "\r\n" +
            Row(70, "Katapultstein01", "  8978432", 25) + "\r\n" +
            Row(75, "Fackel", "  8126464", 25) + "\r\n";
        testCases.Add(("synthetic-many-columns-arc-true", MakePfil(manyColsText), true));

        // 3. Malformed / error cases
        // Option false on empty byte array
        testCases.Add(("empty-input-arc-false", Array.Empty<byte>(), false));

        // Option false on random invalid bytes
        testCases.Add(("random-bytes-arc-false", new byte[] { 1, 2, 3, 4, 5 }, false));

        // Option true on empty byte array
        testCases.Add(("empty-input-arc-true", Array.Empty<byte>(), true));

        // Option true on random bytes < 64
        testCases.Add(("random-bytes-arc-true", new byte[] { 1, 2, 3, 4, 5 }, true));

        // Missing projectiles: 0 found (header only)
        testCases.Add(("missing-all-projectiles-arc-true", MakePfil("[ParticleGeometrieDefault]\r\n;header only\r\n"), true));

        // Missing 1 projectile (only 6 projectiles: missing Fackel)
        string missingFackel = "[ParticleGeometrieDefault]\r\n" +
            Row(20, "Wurfspeer00", "  5832704") + "\r\n" +
            Row(21, "Wurfaxt00", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n";
        testCases.Add(("missing-fackel-arc-true", MakePfil(missingFackel), true));

        // Missing Wurfspeer00
        string missingWurfspeer = "[ParticleGeometrieDefault]\r\n" +
            Row(21, "Wurfaxt00", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n" +
            Row(75, "Fackel", "  8126464") + "\r\n";
        testCases.Add(("missing-wurfspeer-arc-true", MakePfil(missingWurfspeer), true));

        // Duplicate projectile (8 projectiles: Pfeil00 duplicated)
        string duplicatePfeil = standardText + Row(24, "Pfeil00", "  5832704") + "\r\n";
        testCases.Add(("duplicate-pfeil-arc-true", MakePfil(duplicatePfeil), true));

        // Duplicate Fackel
        string duplicateFackel = standardText + Row(76, "Fackel", "  8126464") + "\r\n";
        testCases.Add(("duplicate-fackel-arc-true", MakePfil(duplicateFackel), true));

        // Invalid ysub: 0
        string zeroYsub = standardText.Replace("  5832704", "        0");
        testCases.Add(("invalid-ysub-zero-arc-true", MakePfil(zeroYsub), true));

        // Invalid ysub: negative
        string negYsub = standardText.Replace("  5832704", " -5832704");
        testCases.Add(("invalid-ysub-negative-arc-true", MakePfil(negYsub), true));

        // Invalid ysub: alphabetic
        string alphaYsub = standardText.Replace("  5832704", "      abc");
        testCases.Add(("invalid-ysub-alpha-arc-true", MakePfil(alphaYsub), true));

        // Invalid ysub: float decimal
        string floatYsub = standardText.Replace("  5832704", " 5832.704");
        testCases.Add(("invalid-ysub-float-arc-true", MakePfil(floatYsub), true));

        // Invalid ysub: trailing text
        string trailingTextYsub = standardText.Replace("  5832704", "   5832px");
        testCases.Add(("invalid-ysub-trailing-text-arc-true", MakePfil(trailingTextYsub), true));

        // Invalid ysub: empty string
        string emptyYsub = standardText.Replace("  5832704", "");
        testCases.Add(("invalid-ysub-empty-arc-true", MakePfil(emptyYsub), true));

        // Width overflow: ysub is "50" (width 2), new value "100" (length 3 > 2)
        string overflowWidth2 = "[ParticleGeometrieDefault]\r\n" +
            Row(20, "Wurfspeer00", "50") + "\r\n" +
            Row(21, "Wurfaxt00", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n" +
            Row(75, "Fackel", "  8126464") + "\r\n";
        testCases.Add(("width-overflow-width2-arc-true", MakePfil(overflowWidth2), true));

        // Width overflow: ysub is "  5000000" (width 9), new value "10000000" (length 8 <= 9, ok) vs "5000000" (width 7, new value 10000000 len 8 > 7, overflow)
        string overflowWidth7 = "[ParticleGeometrieDefault]\r\n" +
            Row(20, "Wurfspeer00", "5000000") + "\r\n" +
            Row(21, "Wurfaxt00", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n" +
            Row(75, "Fackel", "  8126464") + "\r\n";
        testCases.Add(("width-overflow-width7-arc-true", MakePfil(overflowWidth7), true));

        // Truncated columns: projectile row has only 12 columns (column 12 missing)
        string truncatedCols = "[ParticleGeometrieDefault]\r\n" +
            Row(20, "Wurfspeer00", "  5832704", 12) + "\r\n" +
            Row(21, "Wurfaxt00", "  5832704") + "\r\n" +
            Row(22, "Katapultstein00", "  8978432") + "\r\n" +
            Row(23, "Pfeil00", "  5832704") + "\r\n" +
            Row(45, "Spiess00", "  5832704") + "\r\n" +
            Row(70, "Katapultstein01", "  8978432") + "\r\n" +
            Row(75, "Fackel", "  8126464") + "\r\n";
        testCases.Add(("truncated-cols-arc-true", MakePfil(truncatedCols), true));

        // Wrong casing: "wurfspeer00" instead of "Wurfspeer00" (ordinal comparison fails)
        string wrongCase = standardText.Replace("Wurfspeer00", "wurfspeer00");
        testCases.Add(("wrong-case-arc-true", MakePfil(wrongCase), true));

        // Malformed PFIL: truncated PFIL header (20 bytes starting with PFIL)
        byte[] truncatedPfil = new byte[20];
        truncatedPfil[0] = (byte)'P'; truncatedPfil[1] = (byte)'F'; truncatedPfil[2] = (byte)'I'; truncatedPfil[3] = (byte)'L';
        testCases.Add(("truncated-pfil-header-arc-true", truncatedPfil, true));

        // Malformed PFIL: uncompressed size > 50 MB
        byte[] bombPfil = new byte[64];
        bombPfil[0] = (byte)'P'; bombPfil[1] = (byte)'F'; bombPfil[2] = (byte)'I'; bombPfil[3] = (byte)'L';
        BitConverter.GetBytes(60 * 1024 * 1024).CopyTo(bombPfil, 16);
        testCases.Add(("bomb-pfil-uncompressed-size-arc-true", bombPfil, true));

        var caseResults = new List<object>();

        foreach (var (name, input, arcHeight) in testCases)
        {
            var options = new PartgeoOptions(arcHeight);
            try
            {
                byte[] patched = PartgeoPatcher.GetPatchedBytes(input, options);
                caseResults.Add(new
                {
                    Name = name,
                    ProjectileArcHeight = arcHeight,
                    InputBase64 = Convert.ToBase64String(input),
                    IsSuccess = true,
                    OutputBase64 = Convert.ToBase64String(patched),
                    OutputSha256 = Convert.ToHexString(SHA256.HashData(patched)),
                    OutputLength = patched.Length,
                    ErrorType = (string?)null,
                    ErrorMessage = (string?)null
                });
            }
            catch (Exception ex)
            {
                caseResults.Add(new
                {
                    Name = name,
                    ProjectileArcHeight = arcHeight,
                    InputBase64 = Convert.ToBase64String(input),
                    IsSuccess = false,
                    OutputBase64 = (string?)null,
                    OutputSha256 = (string?)null,
                    OutputLength = 0,
                    ErrorType = ex.GetType().Name,
                    ErrorMessage = ex.Message
                });
            }
        }

        var outputObj = new
        {
            Sources = sources,
            TotalCases = caseResults.Count,
            Cases = caseResults
        };

        string json = JsonSerializer.Serialize(outputObj, new JsonSerializerOptions { WriteIndented = true });
        string dir = Path.GetDirectoryName(args[0])!;
        if (!string.IsNullOrEmpty(dir)) Directory.CreateDirectory(dir);
        File.WriteAllText(args[0], json);
        Console.WriteLine($"Partgeo C# oracle generated {caseResults.Count} cases -> {args[0]}");
    }
}
