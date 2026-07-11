using System.Collections.Generic;
using System.IO;
using System.Linq;
using AgainstRomeModifier.Core.Services;
using Xunit;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// Runs only when the local, proprietary Backup.zip is embedded in the product
/// assembly. Public CI intentionally excludes that archive.
/// </summary>
internal sealed class RequiresBackupZipFactAttribute : FactAttribute
{
    public RequiresBackupZipFactAttribute()
    {
        bool backupIsEmbedded = typeof(BackupManager).Assembly
            .GetManifestResourceNames()
            .Any(name => name.EndsWith(".Backup.zip", StringComparison.OrdinalIgnoreCase));

        if (!backupIsEmbedded)
        {
            Skip = "Requires the local Backup.zip archive, which public CI intentionally does not include.";
        }
    }
}

/// <summary>
/// Builds an isolated test game directory from the embedded backup.  Supplemental
/// files are deliberately synthetic and are never copied from an installed game.
/// </summary>
internal sealed class BackupZipGameFixture : IDisposable
{
    private static readonly (string Name, int SymbolIndex)[] FoodHealingScripts =
    {
        ("ak_artillerie", 69), ("ak_geisterreiter", 68), ("ak_kampfverband", 89),
        ("ak_krieger", 73), ("ak_kundschafterwolf", 77), ("ak_landtier", 72),
        ("ak_packpferd", 57), ("ak_priester", 92), ("ak_verbandswolf", 68),
        ("ak_zivilist", 83), ("ak_zivilverband", 87),
    };

    public string RootPath { get; }
    public BackupManager Backup { get; }

    private BackupZipGameFixture(string rootPath, BackupManager backup)
    {
        RootPath = rootPath;
        Backup = backup;
    }

    public static BackupZipGameFixture Create()
    {
        var backup = new BackupManager(new NullLogger());
        backup.LoadBackupZipToMemory(string.Empty);
        if (backup.BackupFiles.Count == 0)
        {
            throw new InvalidOperationException("The embedded Backup.zip is required for characterization tests.");
        }

        string root = Path.Combine(Path.GetTempPath(), "AgainstRomeModifierTests_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        foreach (var (relativePath, bytes) in backup.BackupFiles)
        {
            string destination = Path.Combine(root, relativePath.Replace('/', Path.DirectorySeparatorChar));
            Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
            File.WriteAllBytes(destination, bytes);
        }

        CreateFoodHealingSupplements(root);
        CreateEndlessAiSupplements(root);
        CreateLanguageSupplement(root);
        return new BackupZipGameFixture(root, backup);
    }

    private static void CreateEndlessAiSupplements(string root)
    {
        byte[] levelScript = BuildOriginalLevelScript();
        for (int map = 0; map < 5; map++)
        {
            string mapRoot = Path.Combine(root, "MAPS", $"ENDL_{map:000}");
            WritePfil(Path.Combine(mapRoot, "SCRIPT", "ak_level.bci"), levelScript);
        }

        WritePfil(Path.Combine(root, "SYSTEM", "CLAK", "SCRIPT", "ak_haupthaus.bci"), Words(
            81, 59, 81, 11, 81, 10, 81, 98, 128, 81, 73, -4, 86, Gap,
            66, 1500, 66, 25, 66, -25, 128, 34, 73, -2, 86, 32, 82, 14, 81, 14, 82, 15));

        var villageDefense = new List<int>();
        for (int i = 0; i < 4; i++)
        {
            Append(villageDefense, 66, 0, 66, 1, 66, 6, 66, 6, 66, 0, 66, 0, 66, 0, 66, 1, 90, 8, 128, 157, 73, -9, 86, Gap);
        }
        WritePfil(Path.Combine(root, "SYSTEM", "CLAK", "SCRIPT", "Dorfverteidigung.bci"), Words(villageDefense));

        WritePfil(Path.Combine(root, "SYSTEM", "CLAK", "SCRIPT", "ak_npc.bci"),
            Words(128, 43, 73, -2, 86, 66, 0, 96, 99, 117, 476));
        WritePfil(Path.Combine(root, "SYSTEM", "CLAK", "SCRIPT", "ak_produktion.bci"),
            Words(128, 69, 73, -2, 86, 117, 56, 66, 1, 82, 46));

        int template = 0;
        for (int map = 0; map < 5; map++)
        {
            int count = map < 2 ? 9 : 8;
            for (int i = 0; i < count; i++)
            {
                string path = Path.Combine(root, "MAPS", $"ENDL_{map:000}", $"Endlos_{map}_{template++:00}_Siedlung.sdl");
                WritePfil(path, System.Text.Encoding.Latin1.GetBytes("[building]\r\nnamedef=Test_Haupt\r\nresv=0,0,0,0,0,0\r\n"));
            }
        }
    }

    private const int Gap = unchecked((int)0x6F6F6F6F);

    private static byte[] BuildOriginalLevelScript()
    {
        var words = new List<int>();
        static void Section(List<int> target, params int[] values)
        {
            target.AddRange(values);
            target.Add(Gap);
            target.Add(Gap);
        }

        Section(words, 66, 0, 66, 1, 66, 4, 66, 4, 66, 0, 66, 0, 66, 8, 66, 3, 90, 7, 128, 212, 73, -9, 86); // P1/P2
        Section(words, 128, 83, 86, 66, 180000, 32, 44, 164, 66, 34, 91, 5); // P3
        for (int i = 0; i < 6; i++) Section(words, 81, 61, 90, -3, 128, 83, 86, 66, 600000, 32, 44, 164); // P4
        Section(words, 90, 24, 66, 20, 96, 101, 117, 16, 66, 1, 91, 17); // P5

        (int upper, int lower)[] delays = { (960000, 480000), (960000, 480000), (360000, 240000), (120000, 60000), (120000, 60000), (240000, 120000) };
        foreach (var delay in delays) Section(words, 66, delay.upper, 66, delay.lower, 128, 16); // P6

        int[] spawner = Enumerable.Repeat(Gap, 64).ToArray();
        int[] spawnerHead = { 66, 0, 91, 2, 66, 0, 91, 2, 66, 0, 91, 3, 90, 0, 91, 3 };
        Array.Copy(spawnerHead, spawner, spawnerHead.Length);
        spawner[24] = 80; spawner[37] = 60; spawner[50] = 40; spawner[63] = 20;
        Section(words, spawner); // P7

        Section(words, 90, 0, 66, 4, 96, 98, 91, 11,
            66, 0, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32); // P8

        for (int i = 0; i < 10; i++)
        {
            int opcode = i == 8 || i == 9 ? 90 : 66;
            int value = i == 8 ? 6 : i == 9 ? 15 : 0;
            Section(words, 81, 56, 90, -3, opcode, value, 164);
        }
        Section(words, 128, 214, 73, -2, 86, 66, 1, 96, 102, 117, 92); // P9 filter

        Section(words, 66, 60, 66, 100, 66, 1, 128, 16, 73, -2, 86, 96, 101, 117); // P17
        Section(words, 120, -636, 73, -3, 86, 66, 0, 96, 101, 117, 20, 66, 0, 87); // P18
        Section(words, 66, 2500, 90, 1, 90, 0, 120, -36800, 73, -3, 86); // P19

        Section(words, 71, 66, 0, 117, 16, 66, 256, 91, 7, 112, 0, 66, 256, 90, 14, 96, 118);
        Section(words, 71, 66, 0, 117, 16, 66, 256, 91, 6, 112, 0, 66, 256, 90, 14, 96, 118); // P15
        return Words(words);
    }

    private static void Append(List<int> target, params int[] words) => target.AddRange(words);

    private static void CreateFoodHealingSupplements(string root)
    {
        string scriptRoot = Path.Combine(root, "SYSTEM", "CLAK", "SCRIPT");
        Directory.CreateDirectory(scriptRoot);
        foreach (var (name, symbolIndex) in FoodHealingScripts)
        {
            WritePfil(Path.Combine(scriptRoot, name + ".bci"), Words(66, 1, 81, 10, 81, 98, 128, symbolIndex, 73, -3, 86));
        }
    }

    private static void CreateLanguageSupplement(string root)
    {
        string source = Path.Combine(root, "ToEng", "SYSTEM", "CLMK");
        Directory.CreateDirectory(source);
        File.WriteAllText(Path.Combine(source, "icon.ini"), "synthetic-English-overlay");
    }

    internal static byte[] Words(params int[] words)
    {
        var bytes = new byte[words.Length * sizeof(int)];
        for (int i = 0; i < words.Length; i++)
        {
            BitConverter.GetBytes(words[i]).CopyTo(bytes, i * sizeof(int));
        }
        return bytes;
    }

    internal static byte[] Words(IEnumerable<int> words) => Words(words.ToArray());

    internal static void WritePfil(string path, byte[] decompressed)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        var header = new byte[64];
        header[0] = (byte)'P'; header[1] = (byte)'F'; header[2] = (byte)'I'; header[3] = (byte)'L';
        File.WriteAllBytes(path, GameLZSS.CompressPfil(decompressed, header));
    }

    public void Dispose()
    {
        if (Directory.Exists(RootPath)) Directory.Delete(RootPath, recursive: true);
    }
}
