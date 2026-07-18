using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Install;

/// <summary>
/// 部署 <c>native/argm-trace</c> 執行期飛行紀錄器（version.dll 代理）到遊戲目錄，
/// 產生一份合適的 argm_trace.ini，並以 manifest 標記檔追蹤託管檔案以便安全移除。
/// 與 <see cref="DgVoodooFeature"/> 同屬「把檔案放進遊戲目錄」型的 Compat 功能，
/// 但這是純除錯記錄工具：只安裝 log-only inline hook，不改遊戲檔案。
/// </summary>
internal sealed class ArgmTraceFeature
{
    // 已逆向並逐位元組驗證過的 Against_Rome.exe 組建指紋（見 docs/reverse-engineering/
    // runtime-trace-hooks.md）。只有目標 EXE 指紋相符時才自動解鎖位址型 AI hook，
    // 否則 ini 維持 expectedTimeDateStamp=0，DLL 端只會安裝有簽章驗證的 faction hook。
    private const uint KnownAnalyzedTimeDateStamp = 0x404D1710;

    private const string VersionDllName = "version.dll";
    private const string IniName = "argm_trace.ini";
    private const string MarkerFileName = ".against-rome-modifier-argmtrace.json";
    private const string VersionDllResource = "argm-trace.version.dll";
    private const string GameExeName = "Against_Rome.exe";

    private sealed class ArgmTraceManifest
    {
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

        string dllPath = Path.Combine(gamePath, VersionDllName);
        string iniPath = Path.Combine(gamePath, IniName);
        ArgmTraceManifest? existing = ReadManifest(gamePath);
        byte[] dllBytes = LoadEmbeddedVersionDll();
        string dllHash = ComputeSha256(dllBytes);

        // version.dll 是通用檔名，可能被其它包裝器占用。若目標已有一份而不是本工具
        // 託管的（沒有 marker，或 hash 與過往託管值不符），絕不覆蓋，要求使用者處理。
        if (File.Exists(dllPath))
        {
            string currentHash = ComputeSha256(File.ReadAllBytes(dllPath));
            bool managed = existing != null &&
                           (string.Equals(currentHash, existing.DllSha256, StringComparison.OrdinalIgnoreCase) ||
                            string.Equals(currentHash, dllHash, StringComparison.OrdinalIgnoreCase));
            if (!managed)
            {
                throw new IOException(string.Format(Loc.Get("SvcErrArgmDllConflict"), dllPath));
            }
        }

        // 讀取目標遊戲組建指紋：相符才自動解鎖位址型 AI hook，避免在不明組建上以錯位址
        // 安裝 hook。使用者仍可事後自行編輯 ini 覆寫。
        uint timeDateStamp = TryReadExeTimeDateStamp(gamePath);
        uint expected = timeDateStamp == KnownAnalyzedTimeDateStamp ? KnownAnalyzedTimeDateStamp : 0u;

        // argm_trace.ini：若已存在且非本工具託管（使用者自訂），保留不動；否則寫入
        // 依指紋決定的預設內容。
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
            DllSha256 = dllHash,
            IniSha256 = iniHash,
            ExpectedTimeDateStamp = expected,
        };
        byte[] markerBytes = Encoding.UTF8.GetBytes(
            JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }));
        SafeFileWriter.WriteAllBytes(GetMarkerPath(gamePath), markerBytes, rollback);

        _logger.Log(Loc.Get("SvcLogArgmInstalled"));
        if (expected != 0)
            _logger.Log(string.Format(Loc.Get("SvcLogArgmHookUnlocked"), expected.ToString("X8")));
        else
            _logger.Log(string.Format(Loc.Get("SvcLogArgmHookLocked"),
                timeDateStamp.ToString("X8"), KnownAnalyzedTimeDateStamp.ToString("X8")));
    }

    private void Remove(string gamePath, FileRollbackScope? rollback)
    {
        ArgmTraceManifest? manifest = ReadManifest(gamePath);
        if (manifest == null)
        {
            if (File.Exists(Path.Combine(gamePath, VersionDllName)))
                _logger.Log(Loc.Get("SvcLogArgmNotManaged"));
            return;
        }

        // 只刪本工具託管且未被使用者竄改的檔案。argm_trace.log 是使用者的擷取資料，永不刪除。
        string dllPath = Path.Combine(gamePath, VersionDllName);
        if (File.Exists(dllPath))
        {
            string currentHash = ComputeSha256(File.ReadAllBytes(dllPath));
            if (string.Equals(currentHash, manifest.DllSha256, StringComparison.OrdinalIgnoreCase))
                SafeDeleteFile(dllPath, rollback);
            else
                _logger.Log(string.Format(Loc.Get("SvcLogArgmPreserved"), VersionDllName));
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

    internal bool IsInstalled(string gamePath)
    {
        ArgmTraceManifest? manifest = ReadManifest(gamePath);
        return manifest != null && File.Exists(Path.Combine(gamePath, VersionDllName));
    }

    private static string BuildIni(uint expectedTimeDateStamp)
    {
        // 內容對應 native/argm-trace/argm_trace.ini.sample，但 expectedTimeDateStamp 依
        // 目標組建自動決定。以 \r\n 換行符輸出，符合 Windows INI 慣例。
        var sb = new StringBuilder();
        sb.Append("; 由 Against Rome Modifier 自動產生（argm-trace 執行期飛行紀錄器）。\r\n");
        sb.Append("; 逐鍵說明見 native/argm-trace/argm_trace.ini.sample。\r\n\r\n");
        sb.Append("[general]\r\n");
        sb.Append("enabled=1\r\n");
        sb.Append("verifySignatures=1\r\n");
        sb.Append("argDumpCount=9\r\n");
        sb.Append("maxLogMegabytes=256\r\n\r\n");
        sb.Append("[build]\r\n");
        if (expectedTimeDateStamp != 0)
        {
            sb.Append("; 目標組建已辨識，位址型 AI hook 已解鎖。\r\n");
            sb.Append("expectedTimeDateStamp=").Append(expectedTimeDateStamp.ToString("X8")).Append("\r\n\r\n");
        }
        else
        {
            sb.Append("; 目標組建未辨識，僅安裝有簽章驗證的 faction hook。若確認遊戲與被逆向的\r\n");
            sb.Append("; 組建相同，可把 argm_trace.log 中 [build] banner 顯示的 TimeDateStamp 填在下方。\r\n");
            sb.Append("expectedTimeDateStamp=0\r\n\r\n");
        }
        sb.Append("[hooks]\r\n");
        sb.Append("traceNpcJobs=1\r\n");
        sb.Append("traceNpcActive=1\r\n");
        sb.Append("traceVillage=1\r\n");
        sb.Append("traceCreateUnit=1\r\n");
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
            return JsonSerializer.Deserialize<ArgmTraceManifest>(File.ReadAllText(markerPath, Encoding.UTF8));
        }
        catch (Exception ex) when (ex is JsonException || ex is IOException || ex is UnauthorizedAccessException)
        {
            return null;
        }
    }

    private static byte[] LoadEmbeddedVersionDll()
    {
        System.Reflection.Assembly assembly = typeof(ArgmTraceFeature).Assembly;
        using Stream source = assembly.GetManifestResourceStream(VersionDllResource)
            ?? throw new InvalidDataException("The embedded argm-trace resource is missing: " + VersionDllResource);
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
