using System.Security.Cryptography;
using System.Text.Json;
using AgainstRomeModifier;

// Run from the repository root: dotnet run --project native/against-rome-modifier/tests/oracle_troop_config -- <out.json>
if (args.Length != 1) throw new ArgumentException("Expected fixture output path");
static Dictionary<string, int> Declared<T>() where T : struct, Enum =>
    typeof(T).GetFields().Where(f => f.IsLiteral).ToDictionary(f => f.Name, f => Convert.ToInt32(f.GetRawConstantValue()));
string[] types = { "ranged_inf", "ranged_cav", "hybrid_inf", "siege", "melee_inf", "cav", "priest", "leader_melee",
    "leader_cav", "", "RANGED_INF", "Siege", "none" };
string lower = "figrominf00_lanze_schild";
var output = new {
    Sources = new Dictionary<string, string> { ["TroopConfig"] =
        Convert.ToHexString(SHA256.HashData(File.ReadAllBytes("src.Core/Core/TroopConfig.cs"))) },
    ObjdefIndex = Declared<ObjdefIndex>(),
    RessIndex = Declared<RessIndex>(),
    VolkresIndex = Declared<VolkresIndex>(),
    UnitOrder = TroopConfig.UnitOrder,
    UnitNames = TroopConfig.UnitNames.Select(p => new[] { p.Key, p.Value }).ToArray(),
    UnitMeta = TroopConfig.UnitMeta.Select(p => new[] { p.Key, p.Value.Faction, p.Value.Tier, p.Value.UnitType, p.Value.Style }).ToArray(),
    BalancedStats = TroopConfig.BalancedUnitStats,
    SupportsRangedRange3x = types.ToDictionary(t => t, TroopConfig.SupportsRangedRange3x),
    LowerKey = lower,
    LowerInNames = TroopConfig.UnitNames.ContainsKey(lower),
    LowerInMeta = TroopConfig.UnitMeta.ContainsKey(lower),
    LowerInBalanced = TroopConfig.BalancedUnitStats.ContainsKey(lower),
};
File.WriteAllText(args[0], JsonSerializer.Serialize(output, new JsonSerializerOptions { WriteIndented = true }));
Console.WriteLine($"C# TroopConfig oracle -> {args[0]}");
