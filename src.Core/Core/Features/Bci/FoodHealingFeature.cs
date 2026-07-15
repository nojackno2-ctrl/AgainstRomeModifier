using System.Security.Cryptography;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Bci;

internal static class FoodHealingFeature
{
    private const int Original = 1;
    private const int Ultimate = 10;
    private const string RetiredLeaderSha = "42B07605D20C81A93BE983252DF1CB34104EB7AE9F8456AB31A842D4CD9232DD";
    private const string VanillaLeaderSha = "778A6E01B99664136AC0420B9F48212C41A5D6297A9952EA9D7D3A5D4851272C";
    private static readonly (string File, int Symbol)[] Sites = {
        ("ak_anfuehrer",87),("ak_artillerie",69),("ak_geisterreiter",68),("ak_kampfverband",89),
        ("ak_krieger",73),("ak_kundschafterwolf",77),("ak_landtier",72),("ak_packpferd",57),
        ("ak_priester",92),("ak_verbandswolf",68),("ak_zivilist",83),("ak_zivilverband",87),
    };

    internal static void Apply(string gamePath, bool enabled, BackupManager backup, EndlessAiOrchestrator orchestrator, ILogger logger)
    {
        string root = Path.Combine(gamePath, @"SYSTEM\CLAK\SCRIPT");
        int target = enabled ? Ultimate : Original;
        foreach (var (file, symbol) in Sites)
        {
            string path = Path.Combine(root, file + ".bci");
            if (!File.Exists(path)) throw new FileNotFoundException("找不到待機回血 AI 腳本。", path);
            BciScriptFile script = orchestrator.GetScriptFile(path);
            byte[] raw = script.RawBytes;
            byte[] data = script.DecompressedBytes;
            if (file.Equals("ak_anfuehrer", StringComparison.OrdinalIgnoreCase))
            {
                byte[] normalized = (byte[])data.Clone();
                var old = BciPattern.FindAllBciWordPatternSites(normalized, Signature(symbol, Original));
                var ultimate = BciPattern.FindAllBciWordPatternSites(normalized, Signature(symbol, Ultimate));
                if (old.Count + ultimate.Count == 1)
                {
                    if (ultimate.Count == 1) BciPattern.WriteBciInt32(normalized, ultimate[0] + 4, Ultimate, Original, "retired leader glory probe");
                    if (Convert.ToHexString(SHA256.HashData(normalized)).Equals(RetiredLeaderSha, StringComparison.Ordinal))
                    {
                        raw = backup.GetBackupBytes("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci");
                        script.ReplaceRawBytes(raw);
                        data = script.DecompressedBytes;
                        if (!Convert.ToHexString(SHA256.HashData(data)).Equals(VanillaLeaderSha, StringComparison.Ordinal))
                            throw new InvalidDataException("乾淨的原版 ak_anfuehrer.bci 備份不存在或版本不符，已取消安全遷移。");
                        logger.Log(Loc.Get("SvcLogLeaderScriptMigrated"));
                    }
                }
            }
            var originals = BciPattern.FindAllBciWordPatternSites(data, Signature(symbol, Original));
            var ultimates = BciPattern.FindAllBciWordPatternSites(data, Signature(symbol, Ultimate));
            if (originals.Count + ultimates.Count != 1)
                throw new InvalidDataException(string.Format("待機回血 AI 腳本特徵數量不符（預期 1、實際 {0}）: {1}", originals.Count + ultimates.Count, path));
            int current = originals.Count == 1 ? Original : Ultimate;
            int offset = (originals.Count == 1 ? originals[0] : ultimates[0]) + 4;
            if (current == target) continue;
            BciPattern.WriteBciInt32(data, offset, current, target, "待機回血量");
            script.UpdateDecompressedBytes(data);
        }
    }

    internal static bool TryDetect(string gamePath, out bool enabled)
    {
        enabled = false;
        bool? state = null;
        foreach (var (file, symbol) in Sites)
        {
            string path = Path.Combine(gamePath, @"SYSTEM\CLAK\SCRIPT", file + ".bci");
            if (!File.Exists(path)) return false;
            byte[] data = GameLZSS.DecompressPfil(File.ReadAllBytes(path));
            int originals = BciPattern.FindAllBciWordPatternSites(data, Signature(symbol, Original)).Count;
            int ultimates = BciPattern.FindAllBciWordPatternSites(data, Signature(symbol, Ultimate)).Count;
            if (originals + ultimates != 1) return false;
            bool current = ultimates == 1;
            if (state.HasValue && state.Value != current) return false;
            state = current;
        }
        enabled = state == true;
        return state.HasValue;
    }

    private static int?[] Signature(int symbol, int amount) => new int?[] { 66, amount, 81, 10, 81, 98, 128, symbol, 73, -3, 86 };
}
