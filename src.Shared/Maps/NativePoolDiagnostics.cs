using System.Buffers.Binary;

namespace AgainstRomeModifier.Maps;

public enum NativePoolIssueSeverity { Error, Warning }

public sealed record NativePoolIssue(
    NativePoolIssueSeverity Severity,
    string Code,
    string Message,
    int ObjectSlot = -1,
    int TargetSlot = -1,
    string? TargetPool = null);

public sealed record NativePoolDiagnosticReport(
    bool IsHealthy,
    IReadOnlyList<NativePoolIssue> Issues,
    int ActiveObjects,
    int ActiveAnimSlots,
    int ActiveGfxtypeSlots,
    int ActiveActionSlots,
    int ActiveGroups,
    int TotalObjectSlots,
    uint? EngineUidCounter);

/// <summary>
/// 針對遊戲原生地圖二進位資料池（DATA/objects.dat、anim.dat、gfxtype.dat、action.dat、hirarchy.dat、
/// objdata.dat、position.dat、engine.dat）的唯讀跨池一致性診斷器。
/// </summary>
public static class NativePoolDiagnostics
{
    private static readonly int[] ColumnWidths = [2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 4, 2, 2, 4];
    private static readonly int[] ObjDataWidths = [9, 8, 4, 4, 2, 16, 4, 4, 2, 2, 2, 2, 4, 2, 2, 2, 4, 4, 4, 4, 2, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 4, 2, 2];

    public static NativePoolDiagnosticReport DiagnoseDirectory(string dataDirectory)
    {
        ArgumentNullException.ThrowIfNull(dataDirectory);
        var issues = new List<NativePoolIssue>();
        void AddIssue(NativePoolIssueSeverity severity, string code, string message, int slot = -1, int target = -1, string? pool = null)
            => issues.Add(new(severity, code, message, slot, target, pool));

        string PathOf(string file) => Path.Combine(dataDirectory, file);

        // 讀取 objects.dat
        byte[]? objects = ReadPayload(PathOf("objects.dat"), out string? objErr);
        if (objects is null)
        {
            AddIssue(NativePoolIssueSeverity.Error, "missing-objects", $"無法讀取 objects.dat: {objErr}");
            return EmptyReport(issues);
        }
        if (objects.Length < 16 || BinaryPrimitives.ReadInt32LittleEndian(objects) != 1)
        {
            AddIssue(NativePoolIssueSeverity.Error, "invalid-objects-header", "objects.dat 版本或標頭無效。");
            return EmptyReport(issues);
        }
        int objectCount = BinaryPrimitives.ReadInt32LittleEndian(objects.AsSpan(4));
        int expectedObjLen = 16 + objectCount * (79 + ColumnWidths.Sum());
        if (objectCount < 1 || objectCount > 14000 || objects.Length != expectedObjLen)
        {
            AddIssue(NativePoolIssueSeverity.Error, "objects-length-mismatch", $"objects.dat 長度或槽位數不符（槽位={objectCount}，實際長度={objects.Length}，預期={expectedObjLen}）。");
            return EmptyReport(issues);
        }

        // 讀取 anim.dat
        byte[]? anim = ReadPayload(PathOf("anim.dat"), out string? animErr);
        int animCount = 0;
        HashSet<int> activeAnim = [];
        if (anim is not null)
        {
            if (anim.Length >= 8 && BinaryPrimitives.ReadInt32LittleEndian(anim) == 1)
            {
                animCount = BinaryPrimitives.ReadInt32LittleEndian(anim.AsSpan(4));
                int expectedAnimLen = 8 + animCount * (21 + 4);
                if (anim.Length == expectedAnimLen)
                {
                    for (int i = 0; i < animCount; i++)
                        if (anim[8 + i * 21] != 0) activeAnim.Add(i);
                }
                else AddIssue(NativePoolIssueSeverity.Error, "anim-length-mismatch", $"anim.dat 長度不符（預期 {expectedAnimLen}，實際 {anim.Length}）。");
            }
            else AddIssue(NativePoolIssueSeverity.Error, "invalid-anim-header", "anim.dat 標頭或版本無效。");
        }
        else AddIssue(NativePoolIssueSeverity.Warning, "missing-anim", $"缺少 anim.dat: {animErr}");

        // 讀取 gfxtype.dat
        byte[]? gfxtype = ReadPayload(PathOf("gfxtype.dat"), out string? gfxErr);
        int gfxCount = 0;
        HashSet<int> activeGfx = [];
        if (gfxtype is not null)
        {
            if (gfxtype.Length >= 8 && BinaryPrimitives.ReadInt32LittleEndian(gfxtype) == 1)
            {
                gfxCount = BinaryPrimitives.ReadInt32LittleEndian(gfxtype.AsSpan(4));
                int expectedGfxLen = 8 + gfxCount * (15 + 2);
                if (gfxtype.Length == expectedGfxLen)
                {
                    for (int i = 0; i < gfxCount; i++)
                        if (gfxtype[8 + i * 15] != 0) activeGfx.Add(i);
                }
                else AddIssue(NativePoolIssueSeverity.Error, "gfxtype-length-mismatch", $"gfxtype.dat 長度不符（預期 {expectedGfxLen}，實際 {gfxtype.Length}）。");
            }
            else AddIssue(NativePoolIssueSeverity.Error, "invalid-gfxtype-header", "gfxtype.dat 標頭或版本無效。");
        }
        else AddIssue(NativePoolIssueSeverity.Warning, "missing-gfxtype", $"缺少 gfxtype.dat: {gfxErr}");

        // 讀取 action.dat
        byte[]? action = ReadPayload(PathOf("action.dat"), out string? actErr);
        int actCount = 0;
        HashSet<int> activeAction = [];
        if (action is not null)
        {
            if (action.Length >= 12 && BinaryPrimitives.ReadInt32LittleEndian(action) == 1)
            {
                actCount = BinaryPrimitives.ReadInt32LittleEndian(action.AsSpan(4));
                int words = BinaryPrimitives.ReadInt32LittleEndian(action.AsSpan(8));
                int expectedActLen = 12 + actCount * 25;
                if (words == 6 && action.Length == expectedActLen)
                {
                    for (int i = 0; i < actCount; i++)
                        if (action[12 + i * 25] != 0) activeAction.Add(i);
                }
                else AddIssue(NativePoolIssueSeverity.Error, "action-length-mismatch", $"action.dat 長度或欄位數不符（預期長度 {expectedActLen}，實際 {action.Length}，words={words}）。");
            }
            else AddIssue(NativePoolIssueSeverity.Error, "invalid-action-header", "action.dat 標頭或版本無效。");
        }
        else AddIssue(NativePoolIssueSeverity.Warning, "missing-action", $"缺少 action.dat: {actErr}");

        // 讀取 hirarchy.dat
        byte[]? hirarchy = ReadPayload(PathOf("hirarchy.dat"), out string? hirErr);
        int groupCount = 0;
        HashSet<int> activeGroups = [];
        Dictionary<int, List<int>> groupMembers = [];
        if (hirarchy is not null)
        {
            if (hirarchy.Length >= 12 && BinaryPrimitives.ReadInt32LittleEndian(hirarchy) == 1)
            {
                groupCount = BinaryPrimitives.ReadInt32LittleEndian(hirarchy.AsSpan(4));
                int memberCapacity = BinaryPrimitives.ReadInt32LittleEndian(hirarchy.AsSpan(8));
                int expectedHirLen = 12 + groupCount * 105;
                if (memberCapacity == 50 && hirarchy.Length == expectedHirLen)
                {
                    for (int g = 0; g < groupCount; g++)
                    {
                        int gOff = 12 + g * 103;
                        if (hirarchy[gOff] != 0)
                        {
                            activeGroups.Add(g);
                            int mCount = BinaryPrimitives.ReadInt16LittleEndian(hirarchy.AsSpan(gOff + 1));
                            if (mCount is < 0 or > 50)
                            {
                                AddIssue(NativePoolIssueSeverity.Error, "hirarchy-member-count-invalid", $"hirarchy 群組 {g} 人數無效: {mCount}", target: g, pool: "hirarchy");
                                continue;
                            }
                            var members = new List<int>(mCount);
                            for (int m = 0; m < mCount; m++)
                            {
                                int mSlot = BinaryPrimitives.ReadInt16LittleEndian(hirarchy.AsSpan(gOff + 3 + m * 2));
                                members.Add(mSlot);
                            }
                            groupMembers[g] = members;
                        }
                    }
                }
                else AddIssue(NativePoolIssueSeverity.Error, "hirarchy-length-mismatch", $"hirarchy.dat 長度不符（預期 {expectedHirLen}，實際 {hirarchy.Length}，capacity={memberCapacity}）。");
            }
            else AddIssue(NativePoolIssueSeverity.Error, "invalid-hirarchy-header", "hirarchy.dat 標頭或版本無效。");
        }
        else AddIssue(NativePoolIssueSeverity.Warning, "missing-hirarchy", $"缺少 hirarchy.dat: {hirErr}");

        // 讀取 objdata.dat
        byte[]? objdata = ReadPayload(PathOf("objdata.dat"), out string? dataErr);
        HashSet<int> activeObjData = [];
        if (objdata is not null)
        {
            if (objdata.Length >= 8 && BinaryPrimitives.ReadInt32LittleEndian(objdata) == 1)
            {
                int dataCount = BinaryPrimitives.ReadInt32LittleEndian(objdata.AsSpan(4));
                int expectedDataLen = 8 + dataCount * ObjDataWidths.Sum();
                if (dataCount == objectCount && objdata.Length == expectedDataLen)
                {
                    int seg0Offset = 8;
                    for (int i = 0; i < dataCount; i++)
                        if (objdata[seg0Offset + i * ObjDataWidths[0]] != 0) activeObjData.Add(i);
                }
                else AddIssue(NativePoolIssueSeverity.Error, "objdata-length-mismatch", $"objdata.dat 長度或物件數不符（物件數={dataCount}，預期={objectCount}）。");
            }
            else AddIssue(NativePoolIssueSeverity.Error, "invalid-objdata-header", "objdata.dat 標頭或版本無效。");
        }
        else AddIssue(NativePoolIssueSeverity.Warning, "missing-objdata", $"缺少 objdata.dat: {dataErr}");

        // 讀取 position.dat
        byte[]? positions = ReadPayload(PathOf("position.dat"), out string? posErr);
        int posCount = 0;
        if (positions is not null)
        {
            if (positions.Length >= 8 && BinaryPrimitives.ReadInt32LittleEndian(positions) == 1)
            {
                posCount = BinaryPrimitives.ReadInt32LittleEndian(positions.AsSpan(4));
                int expectedPosLen = 8 + posCount * 17;
                if (positions.Length != expectedPosLen)
                    AddIssue(NativePoolIssueSeverity.Error, "position-length-mismatch", $"position.dat 長度不符（預期 {expectedPosLen}，實際 {positions.Length}）。");
            }
            else AddIssue(NativePoolIssueSeverity.Error, "invalid-position-header", "position.dat 標頭或版本無效。");
        }
        else AddIssue(NativePoolIssueSeverity.Warning, "missing-position", $"缺少 position.dat: {posErr}");

        // 讀取 engine.dat
        uint? engineCounter = null;
        byte[]? engine = ReadPayload(PathOf("engine.dat"), out _);
        if (engine is not null && engine.Length >= 38 && BinaryPrimitives.ReadInt32LittleEndian(engine) == 1)
        {
            engineCounter = BinaryPrimitives.ReadUInt32LittleEndian(engine.AsSpan(34));
        }

        // 掃描活躍物件並進行跨池約束檢查
        int activeObjectCount = 0;
        var uidsSeen = new HashSet<uint>();
        int col1Offset = 16 + objectCount * 79 + objectCount * ColumnWidths[0];
        int col4Offset = 16 + objectCount * 79 + objectCount * ColumnWidths.Take(4).Sum();

        for (int slot = 0; slot < objectCount; slot++)
        {
            int r = 16 + slot * 79;
            if (objects[r] == 0) continue;
            activeObjectCount++;

            uint uid = BinaryPrimitives.ReadUInt32LittleEndian(objects.AsSpan(r + 3));
            if (!uidsSeen.Add(uid))
                AddIssue(NativePoolIssueSeverity.Error, "duplicate-uid", $"物件槽位 {slot} 的 UID 重複：{uid}", slot);

            if (engineCounter.HasValue && (uid & 0x00FFFFFF) > engineCounter.Value)
                AddIssue(NativePoolIssueSeverity.Warning, "uid-exceeds-counter", $"物件槽位 {slot} 的 UID 低 24 位元 ({uid & 0x00FFFFFF}) 超過 engine.dat 計數器 ({engineCounter.Value})", slot);

            // 檢查 anim link
            if (anim is not null)
            {
                short animTarget = BinaryPrimitives.ReadInt16LittleEndian(objects.AsSpan(r + 71));
                if (animTarget != -1)
                {
                    if (animTarget < 0 || animTarget >= animCount)
                        AddIssue(NativePoolIssueSeverity.Error, "anim-out-of-range", $"物件槽位 {slot} 的 anim link 越界: {animTarget}", slot, animTarget, "anim");
                    else if (!activeAnim.Contains(animTarget))
                        AddIssue(NativePoolIssueSeverity.Error, "anim-target-inactive", $"物件槽位 {slot} 的 anim link 指向未啟用的 anim 槽位: {animTarget}", slot, animTarget, "anim");
                }
            }

            // 檢查 gfxtype link
            if (gfxtype is not null)
            {
                short gfxTarget = BinaryPrimitives.ReadInt16LittleEndian(objects.AsSpan(r + 73));
                if (gfxTarget != -1)
                {
                    if (gfxTarget < 0 || gfxTarget >= gfxCount)
                        AddIssue(NativePoolIssueSeverity.Error, "gfxtype-out-of-range", $"物件槽位 {slot} 的 gfxtype link 越界: {gfxTarget}", slot, gfxTarget, "gfxtype");
                    else if (!activeGfx.Contains(gfxTarget))
                        AddIssue(NativePoolIssueSeverity.Error, "gfxtype-target-inactive", $"物件槽位 {slot} 的 gfxtype link 指向未啟用的 gfxtype 槽位: {gfxTarget}", slot, gfxTarget, "gfxtype");
                }
            }

            // 檢查 action link
            if (action is not null)
            {
                short actTarget = BinaryPrimitives.ReadInt16LittleEndian(objects.AsSpan(r + 77));
                if (actTarget != -1)
                {
                    if (actTarget < 0 || actTarget >= actCount)
                        AddIssue(NativePoolIssueSeverity.Error, "action-out-of-range", $"物件槽位 {slot} 的 action link 越界: {actTarget}", slot, actTarget, "action");
                    else if (!activeAction.Contains(actTarget))
                        AddIssue(NativePoolIssueSeverity.Error, "action-target-inactive", $"物件槽位 {slot} 的 action link 指向未啟用的 action 槽位: {actTarget}", slot, actTarget, "action");
                }
            }

            // 檢查 objdata link
            if (objdata is not null)
            {
                ushort dataTarget = BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(col1Offset + slot * 2));
                if (dataTarget != 0xFFFF)
                {
                    if (dataTarget >= objectCount)
                        AddIssue(NativePoolIssueSeverity.Error, "objdata-out-of-range", $"物件槽位 {slot} 的 objdata link 越界: {dataTarget}", slot, dataTarget, "objdata");
                    else if (!activeObjData.Contains(dataTarget))
                        AddIssue(NativePoolIssueSeverity.Error, "objdata-target-inactive", $"物件槽位 {slot} 的 objdata link 指向未啟用的 objdata 槽位: {dataTarget}", slot, dataTarget, "objdata");
                }
            }

            // 檢查 position links
            if (positions is not null)
            {
                int p0 = BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(r + 67));
                int p1 = BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(r + 69));
                if (p0 >= posCount || positions[8 + p0 * 17] == 0)
                    AddIssue(NativePoolIssueSeverity.Error, "position-0-invalid", $"物件槽位 {slot} 的主要位置索引 {p0} 越界或無效", slot, p0, "position");
                if (p1 >= posCount || positions[8 + p1 * 17] == 0)
                    AddIssue(NativePoolIssueSeverity.Error, "position-1-invalid", $"物件槽位 {slot} 的次要位置索引 {p1} 越界或無效", slot, p1, "position");
            }

            // 檢查 hirarchy backlink
            if (hirarchy is not null)
            {
                short gLink = BinaryPrimitives.ReadInt16LittleEndian(objects.AsSpan(col4Offset + slot * 2));
                if (gLink != -1)
                {
                    if (gLink < 0 || gLink >= groupCount)
                        AddIssue(NativePoolIssueSeverity.Error, "hirarchy-backlink-out-of-range", $"物件槽位 {slot} 的群組反向連結越界: {gLink}", slot, gLink, "hirarchy");
                    else if (!activeGroups.Contains(gLink))
                        AddIssue(NativePoolIssueSeverity.Error, "hirarchy-group-inactive", $"物件槽位 {slot} 指向未啟用的 hirarchy 群組: {gLink}", slot, gLink, "hirarchy");
                    else if (!groupMembers.TryGetValue(gLink, out var members) || !members.Contains(slot))
                        AddIssue(NativePoolIssueSeverity.Error, "hirarchy-member-mismatch", $"物件槽位 {slot} 指向群組 {gLink}，但該群組成員清單不包含該物件", slot, gLink, "hirarchy");
                }
            }
        }

        // 檢查群組中的成員反向連結
        if (hirarchy is not null)
        {
            foreach (var (g, members) in groupMembers)
            {
                if (members.Distinct().Count() != members.Count)
                    AddIssue(NativePoolIssueSeverity.Warning, "duplicate-hirarchy-members", $"hirarchy 群組 {g} 包含重複成員槽位", target: g, pool: "hirarchy");

                foreach (int mSlot in members)
                {
                    if (mSlot < 0 || mSlot >= objectCount)
                        AddIssue(NativePoolIssueSeverity.Error, "member-out-of-range", $"hirarchy 群組 {g} 成員索引越界: {mSlot}", target: g, slot: mSlot, pool: "hirarchy");
                    else if (objects[16 + mSlot * 79] == 0)
                        AddIssue(NativePoolIssueSeverity.Error, "member-inactive", $"hirarchy 群組 {g} 成員指向未啟用的物件: {mSlot}", target: g, slot: mSlot, pool: "hirarchy");
                    else
                    {
                        short actualGroup = BinaryPrimitives.ReadInt16LittleEndian(objects.AsSpan(col4Offset + mSlot * 2));
                        if (actualGroup != g)
                            AddIssue(NativePoolIssueSeverity.Error, "member-backlink-mismatch", $"hirarchy 群組 {g} 成員 {mSlot} 的反向連結為 {actualGroup}，不相符", slot: mSlot, target: g, pool: "hirarchy");
                    }
                }
            }
        }

        bool isHealthy = !issues.Any(i => i.Severity == NativePoolIssueSeverity.Error);
        return new NativePoolDiagnosticReport(
            isHealthy,
            issues,
            activeObjectCount,
            activeAnim.Count,
            activeGfx.Count,
            activeAction.Count,
            activeGroups.Count,
            objectCount,
            engineCounter);
    }

    private static NativePoolDiagnosticReport EmptyReport(IReadOnlyList<NativePoolIssue> issues)
        => new(!issues.Any(i => i.Severity == NativePoolIssueSeverity.Error), issues, 0, 0, 0, 0, 0, 0, null);

    private static byte[]? ReadPayload(string path, out string? error)
    {
        error = null;
        if (!File.Exists(path)) { error = "檔案不存在"; return null; }
        try
        {
            byte[] bytes = File.ReadAllBytes(path);
            if (bytes.Length >= 64 && bytes.AsSpan(0, 4).SequenceEqual("PFIL"u8))
                return GameLZSS.DecompressPfil(bytes);
            return bytes;
        }
        catch (Exception ex)
        {
            error = ex.Message;
            return null;
        }
    }
}
