using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Core.Features.Bci;

/// <summary>
/// 「住宅帳篷一次生產 20」的實作已改為 EXE 補丁（玩家專屬按鈕，見
/// <see cref="AgainstRomeModifier.Core.Patches.ExePatchModel"/> 的 CiviProduce20），
/// 因為玩家 ♂/♀ 生產鈕不經過腳本，而是 EXE 處理器 0x44FBED 的 <c>push 1</c>。
///
/// 這個類別只保留一件事：把早期版本誤改到 <c>ak_npc.bci</c>（那是 AI 村莊自動生產
/// 的路徑，非玩家按鈕）的 s_addUnbornCivis 數量字面值還原回原版 1。屬盡力而為的
/// 清理，找不到特徵就跳過，不拋例外。
/// </summary>
internal static class CiviProduce20Feature
{
    private const int Original = 1;
    private const int LegacyBoosted = 20;
    private const string RelPath = @"SYSTEM\CLAK\SCRIPT\ak_npc.bci";
    private const int AddUnbornCivisSymbol = 46;

    /// <summary>還原早期誤寫入 ak_npc.bci 的 AI 生產數量（20 → 1）。找不到就靜默略過。</summary>
    internal static void RestoreAkNpcOriginal(string gamePath, EndlessAiOrchestrator orchestrator)
    {
        string path = Path.Combine(gamePath, RelPath);
        if (!File.Exists(path)) return;

        BciScriptFile script = orchestrator.GetScriptFile(path);
        byte[] data = script.DecompressedBytes;

        var boosted = BciPattern.FindAllBciWordPatternSites(data, Signature(LegacyBoosted));
        if (boosted.Count != 1) return; // 已是原版或特徵不符：不動它

        BciPattern.WriteBciInt32(data, boosted[0] + 4, LegacyBoosted, Original, "還原誤寫的 AI 生產數量");
        script.UpdateDecompressedBytes(data);
    }

    private static int?[] Signature(int amount) => new int?[]
    {
        66, amount,
        90, 3, 90, 2, 90, 8,
        128, AddUnbornCivisSymbol,
        73, -4,
        86
    };
}
