using System.IO.Compression;
using System.Text;
using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Core.Services;

internal sealed record GameSaveInfo(string Folder, string Title, string Level, DateTime LastWriteTime, bool Parsed);
internal sealed record SaveBackupInfo(string FileName, string Title, string Level, string BackupTime,
    string OrigFolder, DateTime LastWriteTime, bool Parsed);
internal sealed record SaveBackupCatalog(IReadOnlyList<GameSaveInfo> Saves, IReadOnlyList<SaveBackupInfo> Backups);
internal sealed record SaveRestoreResult(IReadOnlyList<string> CleanupWarnings);

internal sealed class SaveBackupService
{
    private readonly string _backupDirectory;
    private readonly Dictionary<string, SaveBackupInfo> _cache = new(StringComparer.OrdinalIgnoreCase);
    private readonly object _cacheLock = new();

    internal SaveBackupService(string backupDirectory) =>
        _backupDirectory = Path.GetFullPath(backupDirectory);

    internal SaveBackupCatalog Scan(string gamePath)
    {
        var saves = new List<GameSaveInfo>();
        string saveRoot = GetSaveRoot(gamePath);
        if (Directory.Exists(saveRoot))
        {
            foreach (string directory in Directory.GetDirectories(saveRoot))
            {
                string folder = Path.GetFileName(directory);
                string saveIni = Path.Combine(directory, "save.ini");
                if (!File.Exists(saveIni)) continue;
                bool parsed;
                string title;
                string level;
                try { parsed = TryReadSaveInfo(File.ReadAllBytes(saveIni), out title, out level); }
                catch { parsed = false; title = ""; level = ""; }
                saves.Add(new GameSaveInfo(folder, title, level, Directory.GetLastWriteTime(directory), parsed));
            }
        }

        Directory.CreateDirectory(_backupDirectory);
        var backups = new List<SaveBackupInfo>();
        foreach (string path in Directory.GetFiles(_backupDirectory, "*.zip"))
            backups.Add(GetOrReadBackup(path));
        return new SaveBackupCatalog(saves, backups);
    }

    internal byte[]? ReadSavePreview(string gamePath, string folder)
    {
        string directory = ResolveSaveDirectory(gamePath, folder);
        string preview = Path.Combine(directory, "savepic.tga");
        return File.Exists(preview) ? File.ReadAllBytes(preview) : null;
    }

    internal byte[]? ReadBackupPreview(string fileName)
    {
        string path = ResolveBackupPath(fileName);
        if (!File.Exists(path)) return null;
        using ZipArchive archive = ZipFile.OpenRead(path);
        ZipArchiveEntry? entry = archive.GetEntry("savepic.tga");
        if (entry == null) return null;
        using Stream stream = entry.Open();
        using var memory = new MemoryStream();
        stream.CopyTo(memory);
        return memory.ToArray();
    }

    internal string CreateBackup(string gamePath, string folder, string title, string level, DateTime? now = null)
    {
        string source = ResolveSaveDirectory(gamePath, folder);
        if (!Directory.Exists(source)) throw new DirectoryNotFoundException(source);
        Directory.CreateDirectory(_backupDirectory);

        DateTime backupTime = now ?? DateTime.Now;
        string fileName = $"Backup_{folder}_{backupTime:yyyyMMdd_HHmmss}.zip";
        string destination = ResolveBackupPath(fileName);
        string temporary = destination + ".tmp";
        try
        {
            if (File.Exists(temporary)) File.Delete(temporary);
            using (ZipArchive archive = ZipFile.Open(temporary, ZipArchiveMode.Create))
            {
                foreach (string file in Directory.GetFiles(source, "*", SearchOption.AllDirectories))
                {
                    string entryName = Path.GetRelativePath(source, file).Replace('\\', '/');
                    if (string.Equals(entryName, SaveBackupManifestCodec.EntryName, StringComparison.OrdinalIgnoreCase))
                        continue;
                    archive.CreateEntryFromFile(file, entryName, CompressionLevel.Optimal);
                }
                ZipArchiveEntry manifestEntry = archive.CreateEntry(
                    SaveBackupManifestCodec.EntryName, CompressionLevel.Optimal);
                using var writer = new StreamWriter(manifestEntry.Open(), Encoding.UTF8);
                writer.Write(SaveBackupManifestCodec.Write(folder, title, level, backupTime));
            }
            File.Move(temporary, destination);
            lock (_cacheLock) _cache.Remove(fileName);
            return fileName;
        }
        catch
        {
            try { if (File.Exists(temporary)) File.Delete(temporary); } catch { }
            throw;
        }
    }

    internal SaveRestoreResult RestoreBackup(string gamePath, string fileName, string folder,
        Action<string>? onRollbackFailure = null)
    {
        string archivePath = ResolveBackupPath(fileName);
        if (!File.Exists(archivePath)) throw new FileNotFoundException("找不到備份檔案。", archivePath);
        string saveRoot = GetSaveRoot(gamePath);
        Directory.CreateDirectory(saveRoot);
        string destination = ResolveSaveDirectory(gamePath, folder);
        string temporary = Path.Combine(saveRoot, ".restore_tmp_" + Guid.NewGuid().ToString("N"));
        string oldDirectory = Path.Combine(saveRoot, ".restore_old_" + Guid.NewGuid().ToString("N"));
        bool movedOld = false;
        bool movedNew = false;
        var warnings = new List<string>();

        try
        {
            ZipFile.ExtractToDirectory(archivePath, temporary);
            if (!File.Exists(Path.Combine(temporary, "save.ini")))
                throw new InvalidDataException("備份壓縮檔缺少 save.ini，已停止還原。");
            if (Directory.Exists(destination))
            {
                Directory.Move(destination, oldDirectory);
                movedOld = true;
            }
            Directory.Move(temporary, destination);
            movedNew = true;
        }
        catch (Exception)
        {
            try
            {
                if (movedNew && Directory.Exists(destination)) Directory.Delete(destination, true);
                if (movedOld && Directory.Exists(oldDirectory) && !Directory.Exists(destination))
                    Directory.Move(oldDirectory, destination);
            }
            catch (Exception rollbackError)
            {
                // 回復原存檔失敗只回報給呼叫端記錄；對外仍拋出原始的還原例外。
                onRollbackFailure?.Invoke(rollbackError.Message);
            }
            throw;
        }
        finally
        {
            try { if (Directory.Exists(temporary)) Directory.Delete(temporary, true); }
            catch (Exception error) { warnings.Add(error.Message); }
        }

        if (movedOld && Directory.Exists(oldDirectory))
        {
            try { Directory.Delete(oldDirectory, true); }
            catch (Exception error) { warnings.Add(error.Message); }
        }
        return new SaveRestoreResult(warnings);
    }

    internal void DeleteSave(string gamePath, string folder)
    {
        string directory = ResolveSaveDirectory(gamePath, folder);
        if (Directory.Exists(directory)) Directory.Delete(directory, true);
    }

    internal void DeleteBackup(string fileName)
    {
        string path = ResolveBackupPath(fileName);
        if (File.Exists(path)) File.Delete(path);
        lock (_cacheLock) _cache.Remove(fileName);
    }

    internal static bool IsSimpleName(string value) =>
        !string.IsNullOrWhiteSpace(value) &&
        string.Equals(value, Path.GetFileName(value), StringComparison.Ordinal) &&
        value.IndexOfAny(Path.GetInvalidFileNameChars()) < 0;

    private SaveBackupInfo GetOrReadBackup(string path)
    {
        string fileName = Path.GetFileName(path);
        DateTime lastWrite = File.GetLastWriteTime(path);
        lock (_cacheLock)
            if (_cache.TryGetValue(fileName, out SaveBackupInfo? cached) && cached.LastWriteTime == lastWrite)
                return cached;

        string title = "", level = "", folder = "", backupTime = "";
        bool parsed = true;
        try
        {
            using ZipArchive archive = ZipFile.OpenRead(path);
            SaveBackupManifest? manifest = SaveBackupManifestCodec.Read(archive);
            if (manifest != null)
            {
                title = manifest.Title ?? "";
                level = manifest.Level ?? "";
                folder = manifest.OrigFolder ?? "";
                backupTime = SaveBackupManifestCodec.FormatTime(manifest.BackupTime ?? "");
            }
            ZipArchiveEntry? saveIni = archive.GetEntry("save.ini");
            if (saveIni != null && (string.IsNullOrEmpty(title) || string.IsNullOrEmpty(level)))
            {
                using Stream stream = saveIni.Open();
                using var memory = new MemoryStream();
                stream.CopyTo(memory);
                bool saveParsed = TryReadSaveInfo(memory.ToArray(), out string parsedTitle, out string parsedLevel);
                parsed &= saveParsed;
                if (string.IsNullOrEmpty(title)) title = parsedTitle;
                if (string.IsNullOrEmpty(level)) level = parsedLevel;
            }
        }
        catch
        {
            parsed = false;
        }

        if ((string.IsNullOrEmpty(folder) || string.IsNullOrEmpty(backupTime)) &&
            TryParseLegacyFileName(fileName, out string legacyFolder, out string legacyTime))
        {
            if (string.IsNullOrEmpty(folder)) folder = legacyFolder;
            if (string.IsNullOrEmpty(backupTime)) backupTime = legacyTime;
        }
        if (string.IsNullOrEmpty(backupTime)) backupTime = File.GetCreationTime(path).ToString("yyyy-MM-dd HH:mm:ss");

        var result = new SaveBackupInfo(fileName, title, level, backupTime, folder, lastWrite, parsed);
        lock (_cacheLock) _cache[fileName] = result;
        return result;
    }

    private static bool TryReadSaveInfo(byte[] bytes, out string title, out string level)
    {
        title = "";
        level = "";
        try
        {
            byte[] decompressed = GameLZSS.DecompressPfil(bytes);
            string text = PatchText.GameEncoding.GetString(decompressed);
            string[] lines = text.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries);
            for (int index = 0; index < lines.Length; index++)
            {
                string line = lines[index].Trim();
                if (line.Contains("[orglevelname]") && index + 1 < lines.Length) level = lines[index + 1].Trim();
                if (line.Contains("[titel]") && index + 1 < lines.Length) title = lines[index + 1].Trim();
            }
            return true;
        }
        catch { return false; }
    }

    private static bool TryParseLegacyFileName(string fileName, out string folder, out string backupTime)
    {
        folder = "";
        backupTime = "";
        string name = Path.GetFileNameWithoutExtension(fileName);
        if (!name.StartsWith("Backup_", StringComparison.OrdinalIgnoreCase)) return false;
        int timeSeparator = name.LastIndexOf('_');
        int dateSeparator = timeSeparator < 0 ? -1 : name.LastIndexOf('_', timeSeparator - 1);
        if (dateSeparator < "Backup_".Length) return false;
        string date = name.Substring(dateSeparator + 1, timeSeparator - dateSeparator - 1);
        string time = name.Substring(timeSeparator + 1);
        if (date.Length != 8 || time.Length != 6) return false;
        folder = name.Substring("Backup_".Length, dateSeparator - "Backup_".Length);
        backupTime = $"{date[..4]}-{date.Substring(4, 2)}-{date.Substring(6, 2)} {time[..2]}:{time.Substring(2, 2)}:{time.Substring(4, 2)}";
        return true;
    }

    private static string GetSaveRoot(string gamePath) => Path.Combine(Path.GetFullPath(gamePath), "SAVE");

    private static string ResolveSaveDirectory(string gamePath, string folder)
    {
        if (!IsSimpleName(folder)) throw new InvalidDataException("存檔資料夾名稱無效。");
        return Path.Combine(GetSaveRoot(gamePath), folder);
    }

    private string ResolveBackupPath(string fileName)
    {
        if (!IsSimpleName(fileName)) throw new InvalidDataException("備份檔名無效。");
        return Path.Combine(_backupDirectory, fileName);
    }
}
