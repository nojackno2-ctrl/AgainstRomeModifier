using System.Buffers.Binary;
using System.Text.Json;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Scripting;

/// <summary>cl_scint.ini [ObjDefName] 的一筆別名：腳本 API（s_createObj 等）以別名指定物件定義。</summary>
public sealed record ScriptObjectAlias(string Alias, string NameDef)
{
    public SdlObjectCategory Category => SdlObjectCatalog.Categorize(NameDef);
    public string Tribe => SdlObjectCatalog.Tribe(NameDef);
}

public static class ScriptObjectAliases
{
    /// <summary>讀取 SYSTEM/CLAK/cl_scint.ini（PFIL）的 [ObjDefName] 區段。</summary>
    public static IReadOnlyList<ScriptObjectAlias> Load(string gamePath)
    {
        string path = Path.Combine(gamePath, "SYSTEM", "CLAK", "cl_scint.ini");
        if (!File.Exists(path)) return Array.Empty<ScriptObjectAlias>();
        string text = MapTextEncoding.Game.GetString(GameLZSS.DecompressPfil(File.ReadAllBytes(path)));
        return Parse(text);
    }

    public static IReadOnlyList<ScriptObjectAlias> Parse(string text)
    {
        var result = new List<ScriptObjectAlias>();
        bool inSection = false;
        foreach (string raw in text.Split('\n'))
        {
            string line = raw.Trim();
            if (line.StartsWith('[')) { inSection = line.Equals("[ObjDefName]", StringComparison.OrdinalIgnoreCase); continue; }
            if (!inSection || line.Length == 0 || line.StartsWith(';')) continue;
            int equals = line.IndexOf('=');
            if (equals <= 0) continue;
            string alias = line[..equals].Trim(), nameDef = line[(equals + 1)..].Trim();
            if (alias.Length > 0 && nameDef.Length > 0) result.Add(new ScriptObjectAlias(alias, nameDef));
        }
        return result;
    }
}

/// <summary>
/// 開局的一個物件（建築、人物）或一支部隊；座標為世界座標 x、z（Y 為地表高度）。
/// <see cref="Prebuilt"/> 為 true 時以官方地圖的完工物件範本寫入 DATA（與官方地圖相同、開局即完工）；
/// 否則由地圖腳本在開局生成（建築以 s_createObj 生成時是 0% 的工地）。
/// </summary>
public sealed record ScenarioSpawn(string Alias, float X, float Z, int Team, int Count = 0, int Angle = 0, float Y = 0, bool Prebuilt = false)
{
    public Guid Id { get; init; }
}

/// <summary>編輯器寫入 DATA 的物件槽位；以 uid 確認仍是同一物件後才在下次儲存時移除。</summary>
public sealed record ScenarioDataSlot(int Slot, uint Uid)
{
    public Guid SpawnId { get; init; }
}

/// <summary>地圖的場景設定（編輯器自有格式，存於地圖目錄；遊戲不讀取，儲存時編譯進 ak_level.bci 與 DATA）。</summary>
public sealed class ScenarioDocument
{
    public const string FileName = "arm_scenario.json";
    public int Version { get; set; } = 6;
    public List<ScenarioSpawn> Spawns { get; set; } = new();
    public List<ScenarioDataSlot> DataSlots { get; set; } = new();
    public List<ScenarioEvent> Events { get; set; } = new();

    /// <summary>需由地圖腳本生成的項目（非預建）。</summary>
    [System.Text.Json.Serialization.JsonIgnore]
    public IReadOnlyList<ScenarioSpawn> ScriptSpawns => Spawns.Where(spawn => !spawn.Prebuilt).ToArray();

    private static readonly JsonSerializerOptions Options = new() { WriteIndented = true };

    public static ScenarioDocument Load(string mapDirectory)
    {
        string path = Path.Combine(mapDirectory, FileName);
        if (!File.Exists(path)) return new ScenarioDocument();
        ScenarioDocument result = JsonSerializer.Deserialize<ScenarioDocument>(File.ReadAllText(path), Options)
            ?? throw new InvalidDataException("場景設定不能是 null。");
        if (result.Version is < 1 or > 6 || result.Spawns is null || result.DataSlots is null || result.Events is null)
            throw new InvalidDataException("不支援或不完整的場景設定。");
        if (result.Version >= 4 && result.Spawns.Any(spawn => spawn is null || spawn.Id == Guid.Empty))
            throw new InvalidDataException("場景物件缺少持久 ID。");
        ScenarioObjectIdentity.Prepare(result, legacy: result.Version < 4);
        ScenarioEventValidator.ValidateConditions(result.Events);
        return result;
    }

    public void Save(string mapDirectory, FileRollbackScope rollback)
    {
        ScenarioObjectIdentity.Prepare(this);
        ScenarioEventValidator.ValidateConditions(Events, this);
        Version = 6;
        Core.Services.SafeFileWriter.WriteAllBytes(Path.Combine(mapDirectory, FileName), JsonSerializer.SerializeToUtf8Bytes(this, Options), rollback);
    }
}

/// <summary>
/// 把場景設定編譯成 IPR 位元碼並注入地圖的 ak_level 腳本。
/// 原版腳本備份為 <c>SCRIPT/ak_level.arm_original</c>（遊戲只載入 .bci），每次都從備份重新注入，可重複儲存。
/// 注入方式：在程式碼段尾端附加進入 shim，mainaddr 指向 shim；shim 建立框架、依序呼叫生成 API、還原框架後跳回原 main。
/// ABI（2026-10-07 Codex 靜態分析，原版 TUTOR／ENDL 呼叫序列交叉驗證）：參數反向入棧（最後入棧＝第一個參數）；
/// 76 N＝推入常數 N 的參照；78 N＝推入框架變數 FP+N 的參照；128 N＝呼叫名為常數 N 的原生函式；73 −k＝清除 k 個字組；
/// 16＝把棧頂整數轉為 double（佔兩字組）；74／94／95／75＝儲存／建立／丟棄／還原框架；112＝無條件跳躍（目標＝指令位址＋8＋運算元）；
/// 131＝取棧頂數值等待（原版 main 迴圈的節拍）。
/// </summary>
public static class LevelScriptInjector
{
    public const string ScriptFile = "ak_level.bci";
    public const string OriginalBackupFile = "ak_level.arm_original";
    private const int OpI2D = 16, OpPushLiteral = 66, OpClear = 73, OpPushFp = 74, OpPopFp = 75, OpConstRef = 76, OpFrameRef = 78,
        OpSetFp = 94, OpDropFrame = 95, OpJump = 112, OpCallNative = 128, OpWait = 131;
    /// <summary>建築生成前的等待量（原版 main 迴圈以同一指令等待 10）。</summary>
    public const int BuildingDelayTicks = 10;

    public static string ScriptDirectory(string mapDirectory)
        => Directory.GetDirectories(mapDirectory).FirstOrDefault(path => Path.GetFileName(path).Equals("SCRIPT", StringComparison.OrdinalIgnoreCase))
           ?? Path.Combine(mapDirectory, "SCRIPT");

    /// <summary>依場景設定寫出注入後的腳本；沒有任何生成項目時還原原版腳本。全部在呼叫端交易內完成。</summary>
    public static void Apply(string mapDirectory, ScenarioDocument scenario, IReadOnlyCollection<string> knownAliases, FileRollbackScope rollback)
    {
        ScenarioEventValidator.Validate(scenario.Events, knownAliases);
        ScenarioEventValidator.ValidateConditions(scenario.Events, scenario);
        bool hasEvents = scenario.Events.Any(item => item.Enabled);
        string scriptDirectory = ScriptDirectory(mapDirectory);
        string script = Path.Combine(scriptDirectory, ScriptFile), backup = Path.Combine(scriptDirectory, OriginalBackupFile);
        if (!File.Exists(backup))
        {
            if (scenario.ScriptSpawns.Count == 0 && !hasEvents) return; // 從未注入且沒有內容：保持原檔
            if (!File.Exists(script)) throw new FileNotFoundException("地圖缺少 ak_level.bci，無法加入場景物件。", script);
            Core.Services.SafeFileWriter.WriteAllBytes(backup, File.ReadAllBytes(script), rollback);
        }
        byte[] original = File.ReadAllBytes(backup);
        IReadOnlyList<ScenarioSpawn> spawns = scenario.ScriptSpawns;
        if (spawns.Count == 0 && !hasEvents) { Core.Services.SafeFileWriter.WriteAllBytes(script, original, rollback); return; }
        foreach (ScenarioSpawn spawn in spawns)
            if (!knownAliases.Contains(spawn.Alias, StringComparer.OrdinalIgnoreCase)) throw new InvalidDataException($"未知的物件別名：{spawn.Alias}");
        bool pfil = original.Length >= 64 && original.AsSpan(0, 4).SequenceEqual("PFIL"u8);
        BciImage image = BciImage.Parse(pfil ? GameLZSS.DecompressPfil(original) : original);
        int originalMain = image.MainAddress;
        if (spawns.Count > 0) Inject(image, spawns);
        if (hasEvents) ScenarioEventCompiler.Inject(image, scenario.Events, originalMain, scenario);
        byte[] serialized = image.Serialize();
        Core.Services.SafeFileWriter.WriteAllBytes(script, pfil ? GameLZSS.CompressPfil(serialized, original.AsSpan(0, 64).ToArray()) : serialized, rollback);
    }

    /// <summary>在映像中加入生成 shim 並把 mainaddr 指向它（就地修改）。</summary>
    public static void Inject(BciImage image, IReadOnlyList<ScenarioSpawn> spawns)
    {
        ArgumentNullException.ThrowIfNull(image);
        if (spawns.Where(spawn => spawn.Id != Guid.Empty).Select(spawn => spawn.Id).Distinct().Count()
            != spawns.Count(spawn => spawn.Id != Guid.Empty))
            throw new InvalidDataException("生成物件 ID 重複，無法安全保存 runtime 配對。");
        var code = new List<int>();
        void Op(int opcode) => code.Add(opcode);
        void Op1(int opcode, int operand) { code.Add(opcode); code.Add(operand); }

        int defaultScript = image.AddConstant("DEFSCRIPT");
        int createObj = image.AddConstant("s_createObj");
        int createUnit = image.AddConstant("s_createUnitAndMems");
        int setBinding = spawns.Any(spawn => spawn.Id != Guid.Empty) ? image.AddConstant("s_setScriptVarL") : -1;
        var aliasConstants = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        int AliasConstant(string alias) => aliasConstants.TryGetValue(alias, out int index) ? index : aliasConstants[alias] = image.AddConstant(alias);

        Op(OpPushFp); Op(OpSetFp); Op1(OpClear, 2); // locals[0]=obj, locals[1]=odesc（輸出參數）
        // 部隊先生成，建築等待一次節拍後再建立（2026-10-07 實機：此順序下主屋以 0% 工地生成；
        // 先前未生成是因放置點被樹擋住，等待本身是否必要未單獨驗證）。
        bool waited = false;
        foreach (ScenarioSpawn spawn in spawns.OrderBy(spawn => spawn.Count > 0 ? 0 : 1))
        {
            if (spawn.Count <= 0 && !waited) { Op1(OpPushLiteral, BuildingDelayTicks); Op(OpWait); waited = true; }
            if (!float.IsFinite(spawn.X) || !float.IsFinite(spawn.Z)) throw new InvalidDataException("生成座標必須是有限數值。");
            int indexKey = -1, uidKey = -1;
            void StoreBinding(int key) { Op1(OpConstRef, key); Op1(OpCallNative, setBinding); Op1(OpClear, -2); }
            if (spawn.Id != Guid.Empty)
            {
                indexKey = image.AddConstant(ScenarioObjectIdentity.RuntimeIndexKey(spawn.Id));
                uidKey = image.AddConstant(ScenarioObjectIdentity.RuntimeUidKey(spawn.Id));
                Op1(OpPushLiteral, 0); StoreBinding(indexKey);
                Op1(OpPushLiteral, -1); StoreBinding(uidKey);
                // 91 才是寫入 frame local；建立失敗時不得沿用上個 native 的輸出。
                Op1(OpPushLiteral, 0); Op1(91, 0); Op1(OpPushLiteral, -1); Op1(91, 1);
            }
            int x = (int)MathF.Round(Math.Clamp(spawn.X, 0, 16383)), z = (int)MathF.Round(Math.Clamp(spawn.Z, 0, 16383));
            if (spawn.Count > 0)
            {
                // s_createUnitAndMems(&obj,&odesc,team,unitType,formDef,unused,angle(double),x,z,memberAlias,count,packHorses,life%,morale%)
                if (spawn.Team is < 0 or > 7) throw new InvalidDataException("部隊隊伍必須介於 0 與 7。");
                Op1(OpPushLiteral, 100); Op1(OpPushLiteral, 100); Op1(OpPushLiteral, 0);
                Op1(OpPushLiteral, Math.Clamp(spawn.Count, 1, 20)); Op1(OpConstRef, AliasConstant(spawn.Alias));
                Op1(OpPushLiteral, z); Op1(OpPushLiteral, x);
                Op1(OpPushLiteral, ((spawn.Angle % 360) + 360) % 360); Op(OpI2D);
                Op1(OpPushLiteral, 1); Op1(OpPushLiteral, 0); Op1(OpPushLiteral, 1); Op1(OpPushLiteral, spawn.Team);
                Op1(OpFrameRef, 1); Op1(OpFrameRef, 0);
                Op1(OpCallNative, createUnit); Op1(OpClear, -15);
            }
            else
            {
                // s_createObj(&obj,&odesc,alias,x,z,team,script)
                if (spawn.Team is < -1 or > 15) throw new InvalidDataException("物件隊伍必須介於 -1 與 15。");
                Op1(OpConstRef, defaultScript); Op1(OpPushLiteral, spawn.Team); Op1(OpPushLiteral, z); Op1(OpPushLiteral, x);
                Op1(OpConstRef, AliasConstant(spawn.Alias)); Op1(OpFrameRef, 1); Op1(OpFrameRef, 0);
                Op1(OpCallNative, createObj); Op1(OpClear, -7);
            }
            if (spawn.Id != Guid.Empty)
            {
                // 兩個建立 API 成功回傳 1；只有成功才公布原生輸出的一開始索引/UID。
                Op(86); Op1(OpPushLiteral, 1); Op(96);
                int failed = code.Count; Op1(118, 0);
                Op1(90, 1); StoreBinding(uidKey); Op1(90, 0); StoreBinding(indexKey);
                code[failed + 1] = (code.Count - failed - 2) * 4;
            }
        }
        Op(OpDropFrame); Op(OpPopFp);
        int start = image.Code.Length;
        int jumpAt = start + code.Count * 4;
        Op1(OpJump, image.MainAddress - (jumpAt + 8)); // 跳回原 main（原 initializer 與相對跳躍皆不變）

        var bytes = new byte[code.Count * 4];
        for (int index = 0; index < code.Count; index++) BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(index * 4), code[index]);
        image.AppendCode(bytes);
        image.MainAddress = start;
    }
}
