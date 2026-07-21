using System;
using System.IO;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Core.Services
{
    public class BackupManager
    {
        private readonly Dictionary<string, byte[]> _backupFiles = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
        private readonly IReadOnlyDictionary<string, byte[]> _backupFilesView;
        private readonly BackupAutoHealer _autoHealer;
        private readonly GameDirectoryBackupLoader _gameDirectoryLoader;
        private readonly BackupSourceLoader _sourceLoader;
        private readonly UnitBaselineCatalog _unitBaselines;
        private readonly ILogger _logger;

        public BackupManager(ILogger logger)
        {
            _logger = logger;
            _backupFilesView = new ReadOnlyDictionary<string, byte[]>(_backupFiles);
            _autoHealer = new BackupAutoHealer(_backupFiles, logger, CleanEparaBaseline.CreateBytes);
            _gameDirectoryLoader = new GameDirectoryBackupLoader(logger, CleanEparaBaseline.CreateBytes);
            _sourceLoader = new BackupSourceLoader(typeof(BackupManager).Assembly, AppContext.BaseDirectory);
            _unitBaselines = new UnitBaselineCatalog(_backupFiles);
        }

        public IReadOnlyDictionary<string, byte[]> BackupFiles => _backupFilesView;

        internal void SetBackupFile(string key, byte[] bytes)
        {
            _backupFiles[key] = bytes;
            _unitBaselines.Reset();
        }

        public bool HasFile(string key) => _backupFiles.ContainsKey(key);

        public byte[] GetBackupBytes(string key)
        {
            if (_backupFiles.TryGetValue(key, out byte[]? bytes))
            {
                return bytes;
            }
            throw new InvalidOperationException("記憶體備份中找不到 " + key + "。");
        }

        public void Clear()
        {
            _backupFiles.Clear();
            _unitBaselines.Reset();
        }

        public void LoadBackupZipToMemory(string gamePath)
        {
            BackupZipLoadResult zip = _sourceLoader.TryLoadPreferredZip();
            if (zip.Source != BackupZipSource.None)
            {
                ReplaceBackupFiles(zip.Files);
                TryAutoHealBackupFiles(gamePath);
                ValidateBackupResources();
                _logger.Log(Loc.Get(zip.Source == BackupZipSource.Embedded
                    ? "SvcLogBackupLoadedEmbedded"
                    : "SvcLogBackupLoadedLocal"));
                return;
            }

            if (!TryLoadBackupFromGameDirectory(gamePath, false))
            {
                _logger.Log(Loc.Get("SvcLogBackupMissing"));
            }
        }

        public void TryAutoHealBackupFiles(string gamePath)
        {
            if (_autoHealer.Heal(gamePath))
                _unitBaselines.Reset();
        }

        public List<string> FindMissingBackupResources()
        {
            return BackupValidator.FindMissing(_backupFiles);
        }

        public void ValidateBackupResources()
        {
            BackupValidator.Validate(_backupFiles, _logger);
        }

        public bool EnsureBackupLoadedForGamePath(string gamePath)
        {
            if (_backupFiles.Count > 0 && FindMissingBackupResources().Count == 0)
            {
                return true;
            }
            return TryLoadBackupFromGameDirectory(gamePath, true);
        }

        public bool TryLoadBackupFromGameDirectory(string gamePath, bool showError)
        {
            if (!_gameDirectoryLoader.TryLoad(gamePath, showError, out Dictionary<string, byte[]> loaded))
                return false;

            ReplaceBackupFiles(loaded);

            TryAutoHealBackupFiles(gamePath);
            var missing = FindMissingBackupResources();
            if (missing.Count > 0)
            {
                if (showError)
                {
                    throw new InvalidDataException("遊戲目錄中缺少必要檔案，無法建立完整備份:\r\n" + string.Join("\r\n", missing));
                }
                return false;
            }

            _logger.Log(Loc.Get("SvcLogBackupFromGameDir"));
            return true;
        }

        private void ReplaceBackupFiles(IReadOnlyDictionary<string, byte[]> files)
        {
            _backupFiles.Clear();
            foreach ((string key, byte[] bytes) in files)
                _backupFiles[key] = bytes;
            _unitBaselines.Reset();
        }

        public Dictionary<string, string[]> GetBackupUnitRows()
        {
            return _unitBaselines.GetUnitRows();
        }

        // 兵種基準屬性（依賴 _unitBaselines 實例狀態）。
        // 無狀態的欄位解析請直接使用 UnitStatParser / CleanEparaBaseline。
        public double[] GetOriginalStats(string key)
        {
            return _unitBaselines.GetOriginalStats(key);
        }

        public double[] GetDefaultBalancedStats(string key)
        {
            return _unitBaselines.GetDefaultBalancedStats(key);
        }

        public double[] GetBaseStatsForUnit(string key, PatchProfile options)
        {
            return _unitBaselines.GetBaseStatsForUnit(key, options);
        }
    }
}
