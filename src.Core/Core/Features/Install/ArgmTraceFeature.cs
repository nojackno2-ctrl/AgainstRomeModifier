using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Install;

/// <summary>
/// 部署 <c>native/argm-trace</c> 執行期飛行紀錄器（winmm.dll 代理）到遊戲目錄，
/// 產生一份合適的 argm_trace.ini，並以 manifest 標記檔追蹤託管檔案以便安全移除。
/// 與 <see cref="DgVoodooFeature"/> 同屬「把檔案放進遊戲目錄」型的 Compat 功能，
/// 但這是純除錯記錄工具：只安裝 log-only inline hook，不改遊戲檔案。
///
/// 代理目標為 winmm.dll：遊戲的 import table 直接含 WINMM.dll、且 winmm 非
/// KnownDLL，所以遊戲啟動時會從遊戲目錄載入本代理。（先前用的 version.dll 是
/// 錯誤目標——遊戲不 import 它，單獨放不會被載入，且會與 dgVoodoo 的 version API
/// 呼叫衝突。）預設以 log-only 模式部署（enableHooks=0），先確認代理載入與遊戲
/// 穩定，再手動開啟 hook。
/// </summary>
internal sealed class ArgmTraceFeature
{
    // 已逆向並逐位元組驗證過的 Against_Rome.exe 組建指紋（見 docs/reverse-engineering/
    // runtime-trace-hooks.md）。只有目標 EXE 指紋相符時 ini 才寫入解鎖值，否則寫 0。
    private const uint KnownAnalyzedTimeDateStamp = 0x404D1710;

    private const string WinmmDllName = "winmm.dll";
    private const string LegacyVersionDllName = "version.dll";
    private const string IniName = "argm_trace.ini";
    private const string MarkerFileName = ".against-rome-modifier-argmtrace.json";
    private const string WinmmDllResource = "argm-trace.winmm.dll";
    private const string GameExeName = "Against_Rome.exe";

    private sealed class ArgmTraceManifest
    {
        public string DllName { get; set; } = WinmmDllName;
        public string DllSha256 { get; set; } = "";
        public string IniSha256 { get; set; } = "";
        public uint ExpectedTimeDateStamp { get; set; }
    }

    private readonly ILogger _logger;

    internal ArgmTraceFeature(ILogger logger) => _logger = logger;

    internal void Apply(string gamePath, bool enabled, FileRollbackScope? rollback = null)
    {
        if (!enabled)
        {
            Remove(gamePath, rollback);
            return;
        }

        ArgmTraceManifest? existing = ReadManifest(gamePath);

        // Migration: an earlier build deployed a version.dll proxy. It is the
        // wrong target (never loaded on its own, conflicts with dgVoodoo), so
        // remove any leftover we recognize as ours before deploying winmm.
        RemoveLegacyVersionDll(gamePath, existing, rollback);

        string dllPath = Path.Combine(gamePath, WinmmDllName);
        string iniPath = Path.Combine(gamePath, IniName);
        byte[] dllBytes = LoadEmbeddedWinmmDll();
        string dllHash = ComputeSha256(dllBytes);

        // winmm.dll should not normally exist in the game folder (the game uses
        // the system copy). If one is present that we don't manage, never
        // overwrite it -- it could be another wrapper.
        if (File.Exists(dllPath))
        {
            string currentHash = ComputeSha256(File.ReadAllBytes(dllPath));
            bool managed = existing != null &&
                           string.Equals(existing.DllName, WinmmDllName, StringComparison.OrdinalIgnoreCase) &&
                           (string.Equals(currentHash, existing.DllSha256, StringComparison.OrdinalIgnoreCase) ||
                            string.Equals(currentHash, dllHash, StringComparison.OrdinalIgnoreCase));
            if (!managed)
                throw new IOException(string.Format(Loc.Get("SvcErrArgmDllConflict"), dllPath));
        }

        uint timeDateStamp = TryReadExeTimeDateStamp(gamePath);
        uint expected = timeDateStamp == KnownAnalyzedTimeDateStamp ? KnownAnalyzedTimeDateStamp : 0u;

        // argm_trace.ini：若已存在且非本工具託管（使用者自訂），保留不動。
        bool writeIni = true;
        if (File.Exists(iniPath))
        {
            string currentIniHash = ComputeSha256(File.ReadAllBytes(iniPath));
            bool iniManaged = existing != null &&
                              string.Equals(currentIniHash, existing.IniSha256, StringComparison.OrdinalIgnoreCase);
            if (!iniManaged)
            {
                writeIni = false;
                _logger.Log(Loc.Get("SvcLogArgmIniPreserved"));
            }
        }

        SafeFileWriter.WriteAllBytes(dllPath, dllBytes, rollback);

        byte[] iniBytes = Encoding.UTF8.GetBytes(BuildIni(expected));
        string iniHash;
        if (writeIni)
        {
            SafeFileWriter.WriteAllBytes(iniPath, iniBytes, rollback);
            iniHash = ComputeSha256(iniBytes);
        }
        else
        {
            iniHash = ComputeSha256(File.ReadAllBytes(iniPath));
        }

        var manifest = new ArgmTraceManifest
        {
            DllName = WinmmDllName,
            DllSha256 = dllHash,
            IniSha256 = iniHash,
            ExpectedTimeDateStamp = expected,
        };
        byte[] markerBytes = Encoding.UTF8.GetBytes(
            JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }));
        SafeFileWriter.WriteAllBytes(GetMarkerPath(gamePath), markerBytes, rollback);

        _logger.Log(Loc.Get("SvcLogArgmInstalled"));
        // Deployed in log-only mode (see BuildIni). The build banner still helps
        // the user confirm the fingerprint; hook unlock happens later via ini.
        if (expected != 0)
            _logger.Log(string.Format(Loc.Get("SvcLogArgmHookUnlocked"), expected.ToString("X8")));
        else
            _logger.Log(string.Format(Loc.Get("SvcLogArgmHookLocked"),
                timeDateStamp.ToString("X8"), KnownAnalyzedTimeDateStamp.ToString("X8")));
    }

    private void Remove(string gamePath, FileRollbackScope? rollback)
    {
        ArgmTraceManifest? manifest = ReadManifest(gamePath);

        // Always clean up a legacy version.dll we may have deployed before.
        RemoveLegacyVersionDll(gamePath, manifest, rollback);

        if (manifest == null)
        {
            if (File.Exists(Path.Combine(gamePath, WinmmDllName)))
                _logger.Log(Loc.Get("SvcLogArgmNotManaged"));
            return;
        }

        // 只刪本工具託管且未被使用者竄改的檔案。argm_trace.log 是使用者的擷取資料，永不刪除。
        string dllPath = Path.Combine(gamePath, WinmmDllName);
        if (string.Equals(manifest.DllName, WinmmDllName, StringComparison.OrdinalIgnoreCase) &&
            File.Exists(dllPath))
        {
            string currentHash = ComputeSha256(File.ReadAllBytes(dllPath));
            if (string.Equals(currentHash, manifest.DllSha256, StringComparison.OrdinalIgnoreCase))
                SafeDeleteFile(dllPath, rollback);
            else
                _logger.Log(string.Format(Loc.Get("SvcLogArgmPreserved"), WinmmDllName));
        }

        string iniPath = Path.Combine(gamePath, IniName);
        if (File.Exists(iniPath))
        {
            string currentHash = ComputeSha256(File.ReadAllBytes(iniPath));
            if (string.Equals(currentHash, manifest.IniSha256, StringComparison.OrdinalIgnoreCase))
                SafeDeleteFile(iniPath, rollback);
            else
                _logger.Log(string.Format(Loc.Get("SvcLogArgmPreserved"), IniName));
        }

        SafeDeleteFile(GetMarkerPath(gamePath), rollback);
        _logger.Log(Loc.Get("SvcLogArgmRemoved"));
    }

    // Removes a leftover version.dll from the superseded proxy design, but only
    // if the marker recorded it as ours (hash match) -- never touches a
    // version.dll we don't recognize.
    private void RemoveLegacyVersionDll(string gamePath, ArgmTraceManifest? manifest,
                                        FileRollbackScope? rollback)
    {
        if (manifest == null ||
            !string.Equals(manifest.DllName, LegacyVersionDllName, StringComparison.OrdinalIgnoreCase))
            return;

        string legacyPath = Path.Combine(gamePath, LegacyVersionDllName);
        if (!File.Exists(legacyPath)) return;

        string currentHash = ComputeSha256(File.ReadAllBytes(legacyPath));
        if (string.Equals(currentHash, manifest.DllSha256, StringComparison.OrdinalIgnoreCase))
        {
            SafeDeleteFile(legacyPath, rollback);
            _logger.Log(Loc.Get("SvcLogArgmLegacyRemoved"));
        }
    }

    internal bool IsInstalled(string gamePath)
    {
        ArgmTraceManifest? manifest = ReadManifest(gamePath);
        return manifest != null &&
               string.Equals(manifest.DllName, WinmmDllName, StringComparison.OrdinalIgnoreCase) &&
               File.Exists(Path.Combine(gamePath, WinmmDllName));
    }

    private static string BuildIni(uint expectedTimeDateStamp)
    {
        // 內容對應 native/argm-trace/argm_trace.ini.sample。首次部署以 log-only
        // 模式（enableHooks=0）：DLL 載入、轉發、開 log,但完全不裝 hook,先確認
        // 遊戲穩定;之後可手動改 enableHooks=1 開啟 AI 事件 hook。
        var sb = new StringBuilder();
        sb.Append("; 由 Against Rome Modifier 自動產生（argm-trace 執行期飛行紀錄器,winmm 代理）。\r\n");
        sb.Append("; 逐鍵說明見 native/argm-trace/argm_trace.ini.sample。\r\n\r\n");
        sb.Append("[general]\r\n");
        sb.Append("enabled=1\r\n");
        sb.Append("verifySignatures=1\r\n");
        sb.Append("; 首次部署為 log-only（不裝任何 hook）。確認遊戲能正常啟動且產生\r\n");
        sb.Append("; argm_trace.log 後,把下一行改成 enableHooks=1 即可開啟 AI 事件記錄。\r\n");
        sb.Append("enableHooks=0\r\n");
        sb.Append("argDumpCount=9\r\n");
        sb.Append("maxLogMegabytes=256\r\n\r\n");
        sb.Append("[build]\r\n");
        if (expectedTimeDateStamp != 0)
        {
            sb.Append("; 目標組建已辨識,開啟 hook 後位址型 AI hook 會一併解鎖。\r\n");
            sb.Append("expectedTimeDateStamp=").Append(expectedTimeDateStamp.ToString("X8")).Append("\r\n\r\n");
        }
        else
        {
            sb.Append("; 目標組建未辨識;開啟 hook 後僅安裝有簽章驗證的 hook。\r\n");
            sb.Append("expectedTimeDateStamp=0\r\n\r\n");
        }
        sb.Append("[hooks]\r\n");
        sb.Append("traceNpcJobs=1\r\n");
        sb.Append("traceNpcActive=1\r\n");
        sb.Append("traceVillage=1\r\n");
        sb.Append("traceCreateUnit=1\r\n");
        sb.Append("; 無盡 AI 決策脈絡：復活資格快照、新局邊界、單位數量上限。\r\n");
        sb.Append("traceNpcQuery=1\r\n");
        sb.Append("traceLevelInit=1\r\n");
        sb.Append("traceUnitMax=1\r\n");
        sb.Append("traceOpcodes=0\r\n");
        return sb.ToString();
    }

    // 讀取 gamePath 下 Against_Rome.exe 的 PE FileHeader.TimeDateStamp。讀不到回傳 0。
    private static uint TryReadExeTimeDateStamp(string gamePath)
    {
        try
        {
            string exePath = Path.Combine(gamePath, GameExeName);
            if (!File.Exists(exePath)) return 0;
            using var fs = new FileStream(exePath, FileMode.Open, FileAccess.Read, FileShare.Read);
            using var br = new BinaryReader(fs);
            fs.Seek(0x3C, SeekOrigin.Begin);
            uint peOffset = br.ReadUInt32();
            fs.Seek(peOffset, SeekOrigin.Begin);
            if (br.ReadUInt32() != 0x00004550) return 0; // "PE\0\0"
            fs.Seek(peOffset + 8, SeekOrigin.Begin);     // COFF FileHeader.TimeDateStamp
            return br.ReadUInt32();
        }
        catch (Exception ex) when (ex is IOException || ex is UnauthorizedAccessException)
        {
            return 0;
        }
    }

    private static void SafeDeleteFile(string path, FileRollbackScope? rollback)
    {
        if (!File.Exists(path)) return;
        rollback?.TrackFile(path);
        File.SetAttributes(path, FileAttributes.Normal);
        File.Delete(path);
    }

    private static string GetMarkerPath(string gamePath) => Path.Combine(gamePath, MarkerFileName);

    private static ArgmTraceManifest? ReadManifest(string gamePath)
    {
        string markerPath = GetMarkerPath(gamePath);
        if (!File.Exists(markerPath)) return null;
        try
        {
            ArgmTraceManifest? m = JsonSerializer.Deserialize<ArgmTraceManifest>(
                File.ReadAllText(markerPath, Encoding.UTF8));
            if (m == null) return null;
            // Older markers had no DllName; they described a version.dll deploy.
            if (string.IsNullOrEmpty(m.DllName)) m.DllName = LegacyVersionDllName;
            return m;
        }
        catch (Exception ex) when (ex is JsonException || ex is IOException || ex is UnauthorizedAccessException)
        {
            return null;
        }
    }

    private static byte[] LoadEmbeddedWinmmDll()
    {
        System.Reflection.Assembly assembly = typeof(ArgmTraceFeature).Assembly;
        using Stream source = assembly.GetManifestResourceStream(WinmmDllResource)
            ?? throw new InvalidDataException("The embedded argm-trace resource is missing: " + WinmmDllResource);
        using var ms = new MemoryStream();
        source.CopyTo(ms);
        return ms.ToArray();
    }

    private static string ComputeSha256(byte[] bytes)
    {
        byte[] hash = SHA256.HashData(bytes);
        var sb = new StringBuilder(hash.Length * 2);
        foreach (byte b in hash) sb.Append(b.ToString("x2"));
        return sb.ToString();
    }
}
