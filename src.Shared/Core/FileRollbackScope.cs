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
            _restored = true;
        }

        public void Dispose()
        {
            if (!_committed && !_restored && _entries.Count > 0)
            {
                RestoreAll(null);
            }
            if (Directory.Exists(_rootPath))
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
