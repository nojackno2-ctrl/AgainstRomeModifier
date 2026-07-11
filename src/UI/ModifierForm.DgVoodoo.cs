using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        private const string DgVoodooEmbeddedVersion = "v2.87.3";
        private const string DgVoodooMarkerFileName = ".against-rome-modifier-dgvoodoo.json";
        private static readonly string[] DgVoodooManagedFiles = {
            "D3D8.dll",
            "DDraw.dll",
            "dgVoodooCpl.exe",
            "dgVoodoo.conf"
        };
        private static readonly Dictionary<string, string> DgVoodooResourceNames = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase) {
            { "D3D8.dll", "dgVoodoo2.D3D8.dll" },
            { "DDraw.dll", "dgVoodoo2.DDraw.dll" },
            { "dgVoodooCpl.exe", "dgVoodoo2.dgVoodooCpl.exe" },
            { "dgVoodoo.conf", "dgVoodoo2.dgVoodoo.conf" }
        };

        private sealed class DgVoodooManifest {
            public string Version { get; set; } = "";
            public Dictionary<string, string> Files { get; set; } = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        }

        private static string GetDgVoodooMarkerPath(string gamePath) {
            return Path.Combine(gamePath, DgVoodooMarkerFileName);
        }

        private bool IsDgVoodooInstalled(string gamePath) {
            DgVoodooManifest? manifest = ReadDgVoodooManifest(gamePath);
            return manifest != null &&
                   manifest.Files.ContainsKey("D3D8.dll") &&
                   manifest.Files.ContainsKey("DDraw.dll") &&
                   File.Exists(Path.Combine(gamePath, "D3D8.dll")) &&
                   File.Exists(Path.Combine(gamePath, "DDraw.dll"));
        }

        private void ApplyDgVoodooPatch(string gamePath, bool enabled, FileRollbackScope? rollback = null) {
            if (!enabled) {
                RemoveDgVoodoo(gamePath, rollback);
                return;
            }

            DgVoodooManifest? existingManifest = ReadDgVoodooManifest(gamePath);
            Dictionary<string, byte[]> packageFiles = LoadEmbeddedDgVoodooFiles();

            foreach (string fileName in DgVoodooManagedFiles) {
                string destination = Path.Combine(gamePath, fileName);
                if (!File.Exists(destination)) continue;

                if (existingManifest == null || !existingManifest.Files.TryGetValue(fileName, out string? expectedHash)) {
                    throw new IOException(string.Format(Loc.Get("DgVoodooUnmanagedConflict"), destination));
                }

                string currentHash = ComputeSha256(File.ReadAllBytes(destination));
                if (!string.Equals(currentHash, expectedHash, StringComparison.OrdinalIgnoreCase)) {
                    if (string.Equals(fileName, "dgVoodoo.conf", StringComparison.OrdinalIgnoreCase)) {
                        packageFiles[fileName] = File.ReadAllBytes(destination);
                    } else {
                        throw new IOException(string.Format(Loc.Get("DgVoodooModifiedConflict"), destination));
                    }
                }
            }

            var newManifest = new DgVoodooManifest { Version = DgVoodooEmbeddedVersion };
            foreach (string fileName in DgVoodooManagedFiles) {
                byte[] bytes = packageFiles[fileName];
                SafeWriteAllBytes(Path.Combine(gamePath, fileName), bytes, rollback);
                newManifest.Files[fileName] = ComputeSha256(bytes);
            }

            byte[] markerBytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(newManifest, new JsonSerializerOptions { WriteIndented = true }));
            SafeWriteAllBytes(GetDgVoodooMarkerPath(gamePath), markerBytes, rollback);
            Log(string.Format(Loc.Get("LogDgVoodooInstalled"), DgVoodooEmbeddedVersion));
        }

        private void RemoveDgVoodoo(string gamePath, FileRollbackScope? rollback) {
            DgVoodooManifest? manifest = ReadDgVoodooManifest(gamePath);
            if (manifest == null) {
                if (DgVoodooManagedFiles.Any(fileName => File.Exists(Path.Combine(gamePath, fileName)))) {
                    Log(Loc.Get("LogDgVoodooNotManaged"));
                }
                return;
            }

            var preserved = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (string fileName in DgVoodooManagedFiles) {
                if (!manifest.Files.TryGetValue(fileName, out string? expectedHash)) continue;
                string path = Path.Combine(gamePath, fileName);
                if (!File.Exists(path)) continue;

                string currentHash = ComputeSha256(File.ReadAllBytes(path));
                if (!string.Equals(currentHash, expectedHash, StringComparison.OrdinalIgnoreCase)) {
                    if (string.Equals(fileName, "dgVoodoo.conf", StringComparison.OrdinalIgnoreCase)) {
                        Log(string.Format(Loc.Get("LogDgVoodooPreserved"), fileName));
                        continue;
                    }

                    preserved[fileName] = expectedHash;
                    Log(string.Format(Loc.Get("LogDgVoodooPreserved"), fileName));
                    continue;
                }

                SafeDeleteDgVoodooFile(path, rollback);
            }

            string markerPath = GetDgVoodooMarkerPath(gamePath);
            if (preserved.Count == 0) {
                SafeDeleteDgVoodooFile(markerPath, rollback);
                Log(Loc.Get("LogDgVoodooRemoved"));
            } else {
                manifest.Files = preserved;
                byte[] markerBytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }));
                SafeWriteAllBytes(markerPath, markerBytes, rollback);
            }
        }

        private static void SafeDeleteDgVoodooFile(string path, FileRollbackScope? rollback) {
            if (!File.Exists(path)) return;
            rollback?.TrackFile(path);
            File.SetAttributes(path, FileAttributes.Normal);
            File.Delete(path);
        }

        private static DgVoodooManifest? ReadDgVoodooManifest(string gamePath) {
            string markerPath = GetDgVoodooMarkerPath(gamePath);
            if (!File.Exists(markerPath)) return null;
            try {
                DgVoodooManifest? manifest = JsonSerializer.Deserialize<DgVoodooManifest>(File.ReadAllText(markerPath, Encoding.UTF8));
                if (manifest == null || manifest.Files == null) return null;
                manifest.Files = new Dictionary<string, string>(manifest.Files, StringComparer.OrdinalIgnoreCase);
                return manifest;
            } catch (Exception ex) when (ex is JsonException || ex is IOException || ex is UnauthorizedAccessException) {
                return null;
            }
        }

        private static Dictionary<string, byte[]> LoadEmbeddedDgVoodooFiles() {
            var result = new Dictionary<string, byte[]>(StringComparer.OrdinalIgnoreCase);
            Assembly assembly = typeof(ModifierForm).Assembly;
            foreach (var resource in DgVoodooResourceNames) {
                using Stream source = assembly.GetManifestResourceStream(resource.Value)
                    ?? throw new InvalidDataException("The embedded dgVoodoo2 resource is missing: " + resource.Value);
                using var output = new MemoryStream();
                source.CopyTo(output);
                if (output.Length == 0 || output.Length > 16 * 1024 * 1024) {
                    throw new InvalidDataException("The embedded dgVoodoo2 resource has an invalid size: " + resource.Value);
                }
                result[resource.Key] = output.ToArray();
            }
            return result;
        }

        private static string ComputeSha256(byte[] bytes) {
            return Convert.ToHexString(SHA256.HashData(bytes));
        }
    }
}
