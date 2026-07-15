using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Core.Services;

internal sealed class BackupAutoHealer
{
    private static readonly string[] RecoverableFiles =
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

    private readonly IDictionary<string, byte[]> _files;
    private readonly ILogger _logger;
    private readonly Func<byte[]> _cleanEparaFactory;

    internal BackupAutoHealer(
        IDictionary<string, byte[]> files,
        ILogger logger,
        Func<byte[]> cleanEparaFactory)
    {
        _files = files;
        _logger = logger;
        _cleanEparaFactory = cleanEparaFactory;
    }

    internal bool Heal(string gamePath)
    {
        if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath)) return false;

        bool changed = false;
        foreach (string relativePath in RecoverableFiles)
        {
            if (_files.ContainsKey(relativePath)) continue;
            changed |= relativePath == "SYSTEM/cl_epara.ini"
                ? TryHealEpara(relativePath)
                : TryHealFile(gamePath, relativePath);
        }

        if (!HasTeamDat()) changed |= TryHealTeamFiles(gamePath);
        return changed;
    }

    private bool TryHealEpara(string relativePath)
    {
        try
        {
            _files[relativePath] = _cleanEparaFactory();
            _logger.Log(Loc.Get("SvcLogEparaHealed"));
            return true;
        }
        catch (Exception exception)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogEparaHealFailed"), exception.Message));
            return false;
        }
    }

    private bool TryHealFile(string gamePath, string relativePath)
    {
        string fullPath = Path.Combine(gamePath, relativePath.Replace('/', Path.DirectorySeparatorChar));
        string backupPath = fullPath + ".bak";
        string loadPath = File.Exists(backupPath) ? backupPath : fullPath;
        if (!File.Exists(loadPath)) return false;

        try
        {
            byte[] bytes = File.ReadAllBytes(loadPath);
            if (relativePath == "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau")
            {
                if (!OriginalFileValidator.IsPartgeoOriginal(bytes))
                {
                    _logger.Log(string.Format(Loc.Get("SvcLogAutoHealFailed"), relativePath,
                        "partgeo.dau 已被修改，不能作為備份基準 / partgeo.dau is already modified"));
                    return false;
                }
                if (!File.Exists(backupPath))
                {
                    File.Copy(loadPath, backupPath, overwrite: false);
                    _logger.Log(string.Format("已建立原版檔案實體備份: {0}", backupPath));
                }
            }

            _files[relativePath] = bytes;
            _logger.Log(string.Format(Loc.Get("SvcLogAutoHealed"), relativePath));
            return true;
        }
        catch (Exception exception)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogAutoHealFailed"), relativePath, exception.Message));
            return false;
        }
    }

    private bool HasTeamDat() => _files.Keys.Any(key =>
        key.StartsWith("MAPS/", StringComparison.OrdinalIgnoreCase) &&
        key.EndsWith("team.dat", StringComparison.OrdinalIgnoreCase));

    private bool TryHealTeamFiles(string gamePath)
    {
        string mapsPath = Path.Combine(gamePath, "MAPS");
        if (!Directory.Exists(mapsPath)) return false;

        try
        {
            bool changed = false;
            string normalizedGamePath = Path.GetFullPath(gamePath)
                .TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            foreach (string file in Directory.GetFiles(mapsPath, "team.dat", SearchOption.AllDirectories))
            {
                if (CustomMapManifest.IsCustomMapDirectory(Path.GetDirectoryName(file)!)) continue;
                string relativePath = Path.GetRelativePath(normalizedGamePath, file).Replace('\\', '/');
                string backupPath = file + ".bak";
                string loadPath = File.Exists(backupPath) ? backupPath : file;
                _files[relativePath] = File.ReadAllBytes(loadPath);
                changed = true;
            }
            _logger.Log(Loc.Get("SvcLogTeamDatHealed"));
            return changed;
        }
        catch (Exception exception)
        {
            _logger.Log(string.Format(Loc.Get("SvcLogTeamDatHealFailed"), exception.Message));
            return false;
        }
    }
}
