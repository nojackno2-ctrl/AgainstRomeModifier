using System.Text;
using System.Text.Json;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Install;

internal sealed class LanguagePackFeature
{
    private const string LanguageBackupDirectoryName = ".against-rome-modifier-language-backup";
    private const string LanguageBackupManifestName = "manifest.json";
    private sealed class LanguageBackupManifest
    {
        public List<string> ExistingFiles { get; set; } = new();
        public List<string> MissingFiles { get; set; } = new();
    }
    private readonly ILogger _logger;
    internal LanguagePackFeature(ILogger logger) => _logger = logger;
    private static void SafeCopyFile(string src, string dest, bool overwrite, FileRollbackScope? rollback)
    {
        if (!overwrite && File.Exists(dest)) throw new IOException("目標檔案已存在: " + dest);
        SafeFileWriter.WriteAllBytes(dest, File.ReadAllBytes(src), rollback);
    }
    private static void SafeDeleteFile(string path, FileRollbackScope? rollback)
    {
        if (!File.Exists(path)) return;
        rollback?.TrackFile(path);
        File.SetAttributes(path, FileAttributes.Normal);
        File.Delete(path);
    }
        internal void Apply(string gamePath, bool toEnglish, FileRollbackScope? rollback = null)
        {
            string localToEngDir = Path.Combine(gamePath, "ToEng");

            if (toEnglish)
            {
                if (!Directory.Exists(localToEngDir))
                {
                    throw new DirectoryNotFoundException("找不到 ToEng 英文包資源目錄。");
                }

                string[] files = Directory.GetFiles(localToEngDir, "*", SearchOption.AllDirectories);
                if (files.Length == 0)
                {
                    throw new InvalidDataException("ToEng 英文包資源目錄內無任何檔案。");
                }
                EnsureLanguageBackup(gamePath, localToEngDir, files);
                foreach (string file in files)
                {
                    string relPath = Path.GetRelativePath(localToEngDir, file);
                    string destPath = GetSafeLanguagePath(gamePath, relPath);
                    SafeCopyFile(file, destPath, true, rollback);
                }
                _logger.Log(Loc.Get("SvcLogLangApplied"));
                return;
            }

            string languageManifestPath = Path.Combine(GetLanguageBackupDirectory(gamePath), LanguageBackupManifestName);
            if (!File.Exists(languageManifestPath))
            {
                if (TryGetLanguageOverlayState(gamePath, out bool overlayEnabled) && overlayEnabled)
                {
                    throw new InvalidOperationException("找不到語言還原資訊 manifest.json 備份檔。");
                }
                return;
            }
            RestoreLanguageBackup(gamePath, rollback);
            _logger.Log(Loc.Get("SvcLogLangRestored"));
        }

        private static string GetLanguageBackupDirectory(string gamePath)
        {
            return Path.Combine(gamePath, LanguageBackupDirectoryName);
        }

        private static string GetSafeLanguagePath(string rootPath, string relativePath)
        {
            if (string.IsNullOrWhiteSpace(relativePath) || Path.IsPathRooted(relativePath))
            {
                throw new InvalidDataException("不安全的語言還原目標路徑。");
            }

            string root = Path.GetFullPath(rootPath).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
            string fullPath = Path.GetFullPath(Path.Combine(root, relativePath.Replace('/', Path.DirectorySeparatorChar)));
            if (!fullPath.StartsWith(root, StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidDataException("不安全的語言還原目標路徑。");
            }
            return fullPath;
        }

        private void EnsureLanguageBackup(string gamePath, string sourceRoot, string[] sourceFiles)
        {
            string backupRoot = GetLanguageBackupDirectory(gamePath);
            string manifestPath = Path.Combine(backupRoot, LanguageBackupManifestName);
            if (File.Exists(manifestPath))
            {
                LanguageBackupManifest? existingManifest = JsonSerializer.Deserialize<LanguageBackupManifest>(File.ReadAllText(manifestPath, Encoding.UTF8));
                if (existingManifest == null || existingManifest.ExistingFiles == null || existingManifest.MissingFiles == null)
                {
                    throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
                }
                var coveredPaths = new HashSet<string>(existingManifest.ExistingFiles.Concat(existingManifest.MissingFiles), StringComparer.OrdinalIgnoreCase);
                foreach (string sourcePath in sourceFiles)
                {
                    string relativePath = Path.GetRelativePath(sourceRoot, sourcePath).Replace('\\', '/');
                    if (!coveredPaths.Contains(relativePath)) throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
                }
                foreach (string relativePath in existingManifest.ExistingFiles)
                {
                    if (!File.Exists(GetSafeLanguagePath(Path.Combine(backupRoot, "files"), relativePath)))
                    {
                        throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
                    }
                }
                return;
            }
            if (Directory.Exists(backupRoot))
            {
                throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
            }
            if (TryGetLanguageOverlayState(gamePath, out bool alreadyEnabled) && alreadyEnabled)
            {
                throw new InvalidOperationException("檢測到已套用英文套件，但找不到語言還原資訊備份檔。");
            }

            string tempRoot = backupRoot + ".tmp-" + Guid.NewGuid().ToString("N");
            var manifest = new LanguageBackupManifest();
            try
            {
                foreach (string sourcePath in sourceFiles)
                {
                    string relativePath = Path.GetRelativePath(sourceRoot, sourcePath).Replace('\\', '/');
                    string destinationPath = GetSafeLanguagePath(gamePath, relativePath);
                    if (File.Exists(destinationPath))
                    {
                        string backupPath = GetSafeLanguagePath(Path.Combine(tempRoot, "files"), relativePath);
                        Directory.CreateDirectory(Path.GetDirectoryName(backupPath)!);
                        File.Copy(destinationPath, backupPath, true);
                        manifest.ExistingFiles.Add(relativePath);
                    }
                    else
                    {
                        manifest.MissingFiles.Add(relativePath);
                    }
                }

                Directory.CreateDirectory(tempRoot);
                File.WriteAllText(
                    Path.Combine(tempRoot, LanguageBackupManifestName),
                    JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }),
                    Encoding.UTF8);
                Directory.Move(tempRoot, backupRoot);
            }
            finally
            {
                if (Directory.Exists(tempRoot)) Directory.Delete(tempRoot, true);
            }
        }

        private void RestoreLanguageBackup(string gamePath, FileRollbackScope? rollback)
        {
            string backupRoot = GetLanguageBackupDirectory(gamePath);
            string manifestPath = Path.Combine(backupRoot, LanguageBackupManifestName);
            if (!File.Exists(manifestPath))
            {
                throw new InvalidOperationException("找不到語言還原資訊 manifest.json 備份檔。");
            }

            LanguageBackupManifest? manifest = JsonSerializer.Deserialize<LanguageBackupManifest>(File.ReadAllText(manifestPath, Encoding.UTF8));
            if (manifest == null || manifest.ExistingFiles == null || manifest.MissingFiles == null)
            {
                throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
            }

            foreach (string relativePath in manifest.ExistingFiles)
            {
                string backupPath = GetSafeLanguagePath(Path.Combine(backupRoot, "files"), relativePath);
                if (!File.Exists(backupPath)) throw new InvalidDataException("語言還原資訊 manifest.json 格式損毀。");
                SafeCopyFile(backupPath, GetSafeLanguagePath(gamePath, relativePath), true, rollback);
            }
            foreach (string relativePath in manifest.MissingFiles)
            {
                string destinationPath = GetSafeLanguagePath(gamePath, relativePath);
                if (!File.Exists(destinationPath)) continue;
                SafeDeleteFile(destinationPath, rollback);
            }
        }

        internal bool TryGetLanguageOverlayState(string gamePath, out bool enabled)
        {
            enabled = false;
            string sourceRoot = Path.Combine(gamePath, "ToEng");
            if (!Directory.Exists(sourceRoot)) return false;
            string[] sourceFiles = Directory.GetFiles(sourceRoot, "*", SearchOption.AllDirectories);
            if (sourceFiles.Length == 0) return false;

            foreach (string sourcePath in sourceFiles)
            {
                string relativePath = Path.GetRelativePath(sourceRoot, sourcePath);
                string destinationPath = GetSafeLanguagePath(gamePath, relativePath);
                if (!FilesAreEqual(sourcePath, destinationPath)) return true;
            }
            enabled = true;
            return true;
        }

        private static bool FilesAreEqual(string firstPath, string secondPath)
        {
            var firstInfo = new FileInfo(firstPath);
            var secondInfo = new FileInfo(secondPath);
            if (!firstInfo.Exists || !secondInfo.Exists || firstInfo.Length != secondInfo.Length) return false;

            const int bufferSize = 81920;
            byte[] firstBuffer = new byte[bufferSize];
            byte[] secondBuffer = new byte[bufferSize];
            using var first = new FileStream(firstPath, FileMode.Open, FileAccess.Read, FileShare.Read, bufferSize, FileOptions.SequentialScan);
            using var second = new FileStream(secondPath, FileMode.Open, FileAccess.Read, FileShare.Read, bufferSize, FileOptions.SequentialScan);
            while (true)
            {
                int firstRead = first.Read(firstBuffer, 0, firstBuffer.Length);
                int secondRead = second.Read(secondBuffer, 0, secondBuffer.Length);
                if (firstRead != secondRead) return false;
                if (firstRead == 0) return true;
                if (!firstBuffer.AsSpan(0, firstRead).SequenceEqual(secondBuffer.AsSpan(0, secondRead))) return false;
            }
        }

}

