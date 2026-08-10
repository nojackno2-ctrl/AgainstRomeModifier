using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Core.Services;

internal sealed class GameDirectoryBackupLoader
{
    private static readonly string[] BaselineFiles =
    {
        "Against_Rome.exe",
        "SYSTEM/cl_script.ini",
        "SYSTEM/cl_epara.ini",
        "SYSTEM/ress.ini",
        "SYSTEM/DATA_MP/DEFAULTS/objdef.dau",
        "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau",
        "SYSTEM/CLMK/icon.ini",
        "SYSTEM/CLAK/cl_scint.ini",
        "SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci"
    };

    private readonly ILogger _logger;
    private readonly Func<byte[]> _cleanEparaFactory;

    internal GameDirectoryBackupLoader(ILogger logger, Func<byte[]> cleanEparaFactory)
    {
        _logger = logger;
        _cleanEparaFactory = cleanEparaFactory;
    }

    internal bool TryLoad(string gamePath, bool showError, out Dictionary<string, byte[]> files)
    {
        files = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
        if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath))
        {
            if (showError)
                throw new InvalidDataException("找不到 Backup.zip，且遊戲路徑無效，無法建立本機備份。 / Backup.zip not found and game path is invalid.");
            return false;
        }

        try
        {
            LoadBaselineFiles(gamePath, files);
            LoadTeamFiles(gamePath, files);
            return true;
        }
        catch (Exception exception)
        {
            if (showError) throw;
            _logger.Log("建立備份時發生錯誤: " + exception.Message);
            files.Clear();
            return false;
        }
    }

    private void LoadBaselineFiles(string gamePath, Dictionary<string, byte[]> files)
    {
        foreach (string relativePath in BaselineFiles)
        {
            if (relativePath == "SYSTEM/cl_epara.ini")
            {
                try
                {
                    files[relativePath] = _cleanEparaFactory();
                }
                catch (Exception exception)
                {
                    _logger.Log(string.Format(Loc.Get("SvcLogEparaBuildFailed"), exception.Message));
                }
                continue;
            }

            string fullPath = Path.Combine(gamePath, relativePath.Replace('/', Path.DirectorySeparatorChar));
            string backupPath = fullPath + ".bak";
            if (File.Exists(backupPath))
            {
                files[relativePath] = File.ReadAllBytes(backupPath);
                continue;
            }
            if (!File.Exists(fullPath)) continue;

            byte[] bytes = File.ReadAllBytes(fullPath);
            ValidateOriginal(relativePath, bytes);
            File.Copy(fullPath, backupPath, overwrite: false);
            _logger.Log(string.Format("已建立原版檔案實體備份: {0}", backupPath));
            files[relativePath] = bytes;
        }
    }

    private void LoadTeamFiles(string gamePath, Dictionary<string, byte[]> files)
    {
        string mapsPath = Path.Combine(gamePath, "MAPS");
        if (!Directory.Exists(mapsPath)) return;

        string normalizedGamePath = Path.GetFullPath(gamePath)
            .TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        foreach (string file in Directory.GetFiles(mapsPath, "team.dat", SearchOption.AllDirectories))
        {
            if (CustomMapManifest.IsCustomMapDirectory(Path.GetDirectoryName(file)!)) continue;
            string relativePath = Path.GetRelativePath(normalizedGamePath, file).Replace('\\', '/');
            string backupPath = file + ".bak";
            if (File.Exists(backupPath))
            {
                files[relativePath] = File.ReadAllBytes(backupPath);
                continue;
            }

            byte[] bytes = File.ReadAllBytes(file);
            File.Copy(file, backupPath, overwrite: false);
            _logger.Log(string.Format("已建立原版地圖檔案實體備份: {0}", backupPath));
            files[relativePath] = bytes;
        }
    }

    private static void ValidateOriginal(string relativePath, byte[] bytes)
    {
        if (relativePath == "Against_Rome.exe" && !OriginalFileValidator.IsExeOriginal(bytes))
            throw new InvalidDataException("Against_Rome.exe 已經被修改過，或不是支援的原版檔案，無法建立備份。請先還原原版檔案。 / Against_Rome.exe is already modified or not supported. Please restore original file first.");
        if (relativePath == "SYSTEM/DATA_MP/DEFAULTS/objdef.dau" && !OriginalFileValidator.IsObjdefOriginal(bytes))
            throw new InvalidDataException("SYSTEM/DATA_MP/DEFAULTS/objdef.dau 已經被修改過，無法作為備份基準。請先驗證遊戲完整性。 / objdef.dau is already modified. Please verify game integrity first.");
        if (relativePath == "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau" && !OriginalFileValidator.IsPartgeoOriginal(bytes))
            throw new InvalidDataException("SYSTEM/DATA_MP/DEFAULTS/partgeo.dau 已經被修改過，無法作為備份基準。請先驗證遊戲完整性。 / partgeo.dau is already modified. Please verify game integrity first.");
    }
}
