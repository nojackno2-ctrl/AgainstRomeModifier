using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Install;

internal sealed class DgVoodooFeature
{
    private const string DgVoodooEmbeddedVersion = "v2.87.3";
    private const string DgVoodooMarkerFileName = ".against-rome-modifier-dgvoodoo.json";
    private static readonly string[] DgVoodooManagedFiles = { "D3D8.dll", "DDraw.dll", "dgVoodooCpl.exe", "dgVoodoo.conf" };
    private static readonly Dictionary<string, string> DgVoodooResourceNames = new(StringComparer.OrdinalIgnoreCase)
    {
        ["D3D8.dll"] = "dgVoodoo2.D3D8.dll", ["DDraw.dll"] = "dgVoodoo2.DDraw.dll",
        ["dgVoodooCpl.exe"] = "dgVoodoo2.dgVoodooCpl.exe", ["dgVoodoo.conf"] = "dgVoodoo2.dgVoodoo.conf",
    };
    private sealed class DgVoodooManifest
    {
        public string Version { get; set; } = "";
        public Dictionary<string, string> Files { get; set; } = new(StringComparer.OrdinalIgnoreCase);
    }
    private readonly ILogger _logger;
    internal DgVoodooFeature(ILogger logger) => _logger = logger;
    private static void SafeDeleteFile(string path, FileRollbackScope? rollback)
    {
        if (!File.Exists(path)) return;
        rollback?.TrackFile(path);
        File.SetAttributes(path, FileAttributes.Normal);
        File.Delete(path);
    }
        internal void Apply(string gamePath, bool enabled, FileRollbackScope? rollback = null)
        {
            if (!enabled)
            {
                RemoveDgVoodoo(gamePath, rollback);
                return;
            }

            DgVoodooManifest? existingManifest = ReadDgVoodooManifest(gamePath);
            Dictionary<string, byte[]> packageFiles = LoadEmbeddedDgVoodooFiles();

            foreach (string fileName in DgVoodooManagedFiles)
            {
                string destination = Path.Combine(gamePath, fileName);
                if (!File.Exists(destination)) continue;

                if (existingManifest == null || !existingManifest.Files.TryGetValue(fileName, out string? expectedHash))
                {
                    throw new IOException(string.Format("目標檔案已被其它程式佔用或已存在非修改器託管之同名衝突檔案：{0}。如果確認要覆蓋，請先手動移除該檔案後重試。", destination));
                }

                string currentHash = ComputeSha256(File.ReadAllBytes(destination));
                if (!string.Equals(currentHash, expectedHash, StringComparison.OrdinalIgnoreCase))
                {
                    if (string.Equals(fileName, "dgVoodoo.conf", StringComparison.OrdinalIgnoreCase))
                    {
                        packageFiles[fileName] = File.ReadAllBytes(destination);
                    }
                    else
                    {
                        throw new IOException(string.Format("目標已存在已修改或非託管之 dgVoodoo2 檔案：{0}。請先手動備份並移除該衝突檔案後重試。", destination));
                    }
                }
            }

            var newManifest = new DgVoodooManifest { Version = DgVoodooEmbeddedVersion };
            foreach (string fileName in DgVoodooManagedFiles)
            {
                byte[] bytes = packageFiles[fileName];
                SafeFileWriter.WriteAllBytes(Path.Combine(gamePath, fileName), bytes, rollback);
                newManifest.Files[fileName] = ComputeSha256(bytes);
            }

            byte[] markerBytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(newManifest, new JsonSerializerOptions { WriteIndented = true }));
            SafeFileWriter.WriteAllBytes(GetDgVoodooMarkerPath(gamePath), markerBytes, rollback);
            _logger.Log(string.Format(Loc.Get("SvcLogDgvInstalled"), DgVoodooEmbeddedVersion));
        }

        private void RemoveDgVoodoo(string gamePath, FileRollbackScope? rollback)
        {
            DgVoodooManifest? manifest = ReadDgVoodooManifest(gamePath);
            if (manifest == null)
            {
                if (DgVoodooManagedFiles.Any(fileName => File.Exists(Path.Combine(gamePath, fileName))))
                {
                    _logger.Log(Loc.Get("SvcLogDgvNotManaged"));
                }
                return;
            }

            var preserved = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (string fileName in DgVoodooManagedFiles)
            {
                if (!manifest.Files.TryGetValue(fileName, out string? expectedHash)) continue;
                string path = Path.Combine(gamePath, fileName);
                if (!File.Exists(path)) continue;

                string currentHash = ComputeSha256(File.ReadAllBytes(path));
                if (!string.Equals(currentHash, expectedHash, StringComparison.OrdinalIgnoreCase))
                {
                    if (string.Equals(fileName, "dgVoodoo.conf", StringComparison.OrdinalIgnoreCase))
                    {
                        _logger.Log(string.Format(Loc.Get("SvcLogDgvPreserved"), fileName));
                        continue;
                    }

                    preserved[fileName] = expectedHash;
                    _logger.Log(string.Format(Loc.Get("SvcLogDgvPreserved"), fileName));
                    continue;
                }

                SafeDeleteFile(path, rollback);
            }

            string markerPath = GetDgVoodooMarkerPath(gamePath);
            if (preserved.Count == 0)
            {
                SafeDeleteFile(markerPath, rollback);
                _logger.Log(Loc.Get("SvcLogDgvRemoved"));
            }
            else
            {
                manifest.Files = preserved;
                byte[] markerBytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }));
                SafeFileWriter.WriteAllBytes(markerPath, markerBytes, rollback);
            }
        }

        internal bool IsInstalled(string gamePath)
        {
            DgVoodooManifest? manifest = ReadDgVoodooManifest(gamePath);
            return manifest != null &&
                   manifest.Files.ContainsKey("D3D8.dll") &&
                   manifest.Files.ContainsKey("DDraw.dll") &&
                   File.Exists(Path.Combine(gamePath, "D3D8.dll")) &&
                   File.Exists(Path.Combine(gamePath, "DDraw.dll"));
        }

        private static string GetDgVoodooMarkerPath(string gamePath)
        {
            return Path.Combine(gamePath, DgVoodooMarkerFileName);
        }

        private static DgVoodooManifest? ReadDgVoodooManifest(string gamePath)
        {
            string markerPath = GetDgVoodooMarkerPath(gamePath);
            if (!File.Exists(markerPath)) return null;
            try
            {
                DgVoodooManifest? manifest = JsonSerializer.Deserialize<DgVoodooManifest>(File.ReadAllText(markerPath, Encoding.UTF8));
                if (manifest == null || manifest.Files == null) return null;
                manifest.Files = new Dictionary<string, string>(manifest.Files, StringComparer.OrdinalIgnoreCase);
                return manifest;
            }
            catch (Exception ex) when (ex is JsonException || ex is IOException || ex is UnauthorizedAccessException)
            {
                return null;
            }
        }

        private static Dictionary<string, byte[]> LoadEmbeddedDgVoodooFiles()
        {
            var result = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            System.Reflection.Assembly assembly = typeof(DgVoodooFeature).Assembly;
            foreach (var resource in DgVoodooResourceNames)
            {
                using Stream source = assembly.GetManifestResourceStream(resource.Value)
                    ?? throw new InvalidDataException("The embedded dgVoodoo2 resource is missing: " + resource.Value);
                using MemoryStream ms = new MemoryStream();
                source.CopyTo(ms);
                result[resource.Key] = ms.ToArray();
            }
            return result;
        }

        private static string ComputeSha256(byte[] bytes)
        {
            byte[] hash = SHA256.HashData(bytes);
            var sb = new StringBuilder(hash.Length * 2);
            foreach (byte b in hash) sb.Append(b.ToString("x2"));
            return sb.ToString();
        }

}

