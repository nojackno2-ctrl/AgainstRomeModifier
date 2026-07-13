using System.Reflection;
using System.Text.Json;

namespace AgainstRomeModifier.Maps;

public sealed class EndlessMapCloner
{
    private readonly EndlessMapCatalog _catalog;
    public EndlessMapCloner(EndlessMapCatalog? catalog = null) => _catalog = catalog ?? new EndlessMapCatalog();

    public EndlessMapInfo Clone(string gamePath, int sourceSlot, int newSlot, string newName)
    {
        string normalizedGamePath = EndlessMapCatalog.ValidateGamePath(gamePath);
        EndlessMapInfo source = _catalog.Require(normalizedGamePath, sourceSlot);
        return CloneCore(normalizedGamePath, source.Id, source.DirectoryPath, sourceSlot, newSlot, newName);
    }

    public EndlessMapInfo Clone(string gamePath, string sourceMapId, int newSlot, string newName)
    {
        string normalizedGamePath = EndlessMapCatalog.ValidateGamePath(gamePath);
        GameMapInfo source = new GameMapCatalog().Require(normalizedGamePath, sourceMapId);
        return CloneCore(normalizedGamePath, source.Id, source.DirectoryPath, source.EndlessSlot ?? 0, newSlot, newName);
    }

    private EndlessMapInfo CloneCore(string normalizedGamePath, string sourceMapId, string sourceDirectory, int sourceSlot, int newSlot, string newName)
    {
        if (newSlot is < 5 or > 999) throw new ArgumentOutOfRangeException(nameof(newSlot), "自製地圖槽位必須在 ENDL_005 至 ENDL_999。");
        if (string.IsNullOrWhiteSpace(newName)) throw new ArgumentException("請輸入地圖名稱。", nameof(newName));
        if (newName.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0) throw new ArgumentException("地圖名稱含不允許的字元。", nameof(newName));
        if (!MapTextDocument.CanEncodeGameText(newName)) throw new ArgumentException("地圖名稱包含遊戲無法儲存的字元。請改用英文、數字或原版地圖使用的字元。", nameof(newName));
        string mapsPath = Path.Combine(normalizedGamePath, "MAPS");
        string mapId = $"ENDL_{newSlot:000}";
        string destination = Path.Combine(mapsPath, mapId);
        string temporary = destination + ".tmp_arm";
        if (Directory.Exists(destination) || Directory.Exists(temporary)) throw new IOException("目標地圖槽位已存在或有待清理暫存資料夾: " + mapId);

        try
        {
            CopyDirectory(sourceDirectory, temporary);
            VerifyCopy(sourceDirectory, temporary);
            RewriteKnownFiles(temporary, sourceMapId, mapId, newName);
            var marker = new CustomMapEntry(newSlot, sourceSlot, DateTimeOffset.UtcNow, ToolVersion());
            Core.Services.SafeFileWriter.WriteAllBytes(Path.Combine(temporary, CustomMapManifest.MarkerFileName), JsonSerializer.SerializeToUtf8Bytes(marker, new JsonSerializerOptions { WriteIndented = true }));
            Directory.Move(temporary, destination);
            CustomMapManifest manifest = CustomMapManifest.Load(normalizedGamePath);
            manifest.Register(marker);
            manifest.Save(normalizedGamePath);
            return _catalog.Require(normalizedGamePath, newSlot);
        }
        catch
        {
            if (Directory.Exists(temporary)) Directory.Delete(temporary, true);
            throw;
        }
    }

    private static void RewriteKnownFiles(string directory, string oldMapId, string newMapId, string newName)
    {
        string briefing = Path.Combine(directory, "TEXT", "US", "briefing.put");
        if (File.Exists(briefing)) { var put = PutTextDocument.Load(briefing); put.SetValue("briefing_titel_1", newName); put.Save(); }
        foreach (string path in Directory.GetFiles(directory, "Endlos_*_Siedlung*.sdl", SearchOption.TopDirectoryOnly)) { var sdl = SdlDocument.Load(path); sdl.RewriteMapPath(oldMapId, newMapId); sdl.Save(); }
    }

    private static void CopyDirectory(string source, string destination)
    {
        foreach (string directory in Directory.GetDirectories(source, "*", SearchOption.AllDirectories)) Directory.CreateDirectory(Path.Combine(destination, Path.GetRelativePath(source, directory)));
        Directory.CreateDirectory(destination);
        foreach (string file in Directory.GetFiles(source, "*", SearchOption.AllDirectories))
        {
            string target = Path.Combine(destination, Path.GetRelativePath(source, file));
            Directory.CreateDirectory(Path.GetDirectoryName(target)!);
            File.Copy(file, target, false);
        }
    }

    private static void VerifyCopy(string source, string destination)
    {
        var sourceFiles = Directory.GetFiles(source, "*", SearchOption.AllDirectories).Select(x => new FileInfo(x)).OrderBy(x => x.FullName).ToArray();
        var destinationFiles = Directory.GetFiles(destination, "*", SearchOption.AllDirectories).Select(x => new FileInfo(x)).OrderBy(x => x.FullName).ToArray();
        if (sourceFiles.Length != destinationFiles.Length || sourceFiles.Sum(x => x.Length) != destinationFiles.Sum(x => x.Length)) throw new InvalidDataException("地圖複製驗證失敗：檔案數或總大小不一致。");
    }

    private static string ToolVersion() => Assembly.GetEntryAssembly()?.GetName().Version?.ToString() ?? "development";
}
