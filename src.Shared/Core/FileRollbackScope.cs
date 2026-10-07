namespace AgainstRomeModifier
{
    public sealed class FileRollbackScope : IDisposable
    {
        private sealed class RollbackEntry
        {
            public string Path { get; set; } = "";
            public string BackupPath { get; set; } = "";
            public bool Existed { get; set; }
        }

        private readonly object _syncRoot = new object();
        private readonly Dictionary<string, RollbackEntry> _entries = new Dictionary<string, RollbackEntry>(StringComparer.OrdinalIgnoreCase);
        private readonly Dictionary<string, string> _directories = new(StringComparer.OrdinalIgnoreCase);
        private bool _directoryRestoreFailed;
        private readonly string _rootPath;
        private bool _committed;
        private bool _restored;

        public FileRollbackScope()
        {
            _rootPath = Path.Combine(Path.GetTempPath(), "AgainstRomeModifierRollback_" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(_rootPath);
        }

        public void TrackFile(string path)
        {
            string fullPath = Path.GetFullPath(path);
            lock (_syncRoot)
            {
                if (_entries.ContainsKey(fullPath)) return;

                var entry = new RollbackEntry
                {
                    Path = fullPath,
                    Existed = File.Exists(fullPath)
                };

                if (entry.Existed)
                {
                    string backupPath = Path.Combine(_rootPath, Guid.NewGuid().ToString("N") + ".bak");
                    File.Copy(fullPath, backupPath, true);
                    entry.BackupPath = backupPath;
                }

                _entries[fullPath] = entry;
            }
        }

        public void Commit()
        {
            _committed = true;
        }

        /// <summary>Snapshots an existing directory, including unknown files and empty subdirectories.</summary>
        public void TrackDirectory(string path)
        {
            string fullPath = Path.TrimEndingDirectorySeparator(Path.GetFullPath(path));
            lock (_syncRoot)
            {
                if (_directories.ContainsKey(fullPath)) return;
                if (!Directory.Exists(fullPath)) throw new DirectoryNotFoundException(fullPath);
                string backup = Path.Combine(_rootPath, Guid.NewGuid().ToString("N"));
                CopyDirectory(fullPath, backup);
                _directories.Add(fullPath, backup);
            }
        }

        /// <summary>Restores a tracked directory without ending the surrounding transaction.</summary>
        public void RestoreDirectory(string path)
        {
            string fullPath = Path.TrimEndingDirectorySeparator(Path.GetFullPath(path));
            if (!_directories.TryGetValue(fullPath, out string? backup)) throw new InvalidOperationException("Directory was not tracked: " + fullPath);
            if (Directory.Exists(fullPath))
            {
                foreach (string file in Directory.EnumerateFiles(fullPath, "*", SearchOption.AllDirectories)) File.SetAttributes(file, FileAttributes.Normal);
                Directory.Delete(fullPath, true);
            }
            CopyDirectory(backup, fullPath);
        }

        private static void CopyDirectory(string source, string destination)
        {
            if ((File.GetAttributes(source) & FileAttributes.ReparsePoint) != 0) throw new IOException("Cannot snapshot a linked directory: " + source);
            Directory.CreateDirectory(destination);
            foreach (string file in Directory.EnumerateFiles(source))
            {
                if ((File.GetAttributes(file) & FileAttributes.ReparsePoint) != 0) throw new IOException("Cannot snapshot a linked file: " + file);
                File.Copy(file, Path.Combine(destination, Path.GetFileName(file)), true);
            }
            foreach (string directory in Directory.EnumerateDirectories(source)) CopyDirectory(directory, Path.Combine(destination, Path.GetFileName(directory)));
        }

        public bool IsCommitted
        {
            get { return _committed; }
        }

        public void RestoreAll(Action<string>? log)
        {
            if (_restored) return;
            foreach (var entry in _entries.Values.Reverse())
            {
                try
                {
                    string? dir = Path.GetDirectoryName(entry.Path);
                    if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
                    {
                        Directory.CreateDirectory(dir);
                    }

                    if (entry.Existed)
                    {
                        if (File.Exists(entry.Path))
                        {
                            File.SetAttributes(entry.Path, FileAttributes.Normal);
                        }
                        File.Copy(entry.BackupPath, entry.Path, true);
                    }
                    else if (File.Exists(entry.Path))
                    {
                        File.SetAttributes(entry.Path, FileAttributes.Normal);
                        File.Delete(entry.Path);
                    }
                }
                catch (Exception ex)
                {
                    log?.Invoke(string.Format("[回復警告] 無法回復 {0}: {1}", entry.Path, ex.Message));
                }
            }
            foreach (var directory in _directories.Reverse())
            {
                try { RestoreDirectory(directory.Key); }
                catch (Exception ex)
                {
                    _directoryRestoreFailed = true;
                    log?.Invoke($"[回復警告] 無法回復 {directory.Key}: {ex.Message}。備份保留於 {_rootPath}");
                }
            }
            _restored = true;
        }

        public void Dispose()
        {
            if (!_committed && !_restored && (_entries.Count > 0 || _directories.Count > 0))
            {
                RestoreAll(null);
            }
            if (!_directoryRestoreFailed && Directory.Exists(_rootPath))
            {
                try
                {
                    Directory.Delete(_rootPath, true);
                }
                catch (Exception ex)
                {
                    System.Diagnostics.Debug.WriteLine("[Rollback] Failed to delete temp directory " + _rootPath + ": " + ex.Message);
                }
            }
        }
    }
}
