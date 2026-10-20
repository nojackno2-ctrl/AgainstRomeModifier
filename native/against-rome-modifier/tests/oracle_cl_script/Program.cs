using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Features.Ini;
using AgainstRomeModifier.Core.Services;

// Runs the real internal ClScriptFeatureDetector.Detect over a temp game dir + in-memory backup.
// Usage (repo root): dotnet run --project native/against-rome-modifier/tests/oracle_cl_script -- <fixture.json>
if (args.Length != 1) throw new ArgumentException("Expected fixture output path");
Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);

string[] flags = { "FastCiviProduction", "InfiniteMorale", "SpellDamage5x", "SpellHealing10x", "SpellResurrection", "SpellRange3x", "GeneralSkills" };
string Morale(string lost, string flee, string over, string idle) =>
    $"MoralsDecLostMem=GER,{lost}\r\nMoralsDecFlee=GER,{flee}\r\nMoralsDecOverPop=GER,{over}\r\nMoralsIncIdle=GER,{idle}\r\n";
const string Orig = "Value=KEL,Spell3,50\r\nValue2=KEL,Spell3,60\r\nValue=GER,Spell3,10\r\n";
string Dmg(int mult) => string.Join("", new[] { "GER_Spell2", "HUN_Spell0", "HUN_Spell1", "HUN_Spell2", "KEL_Spell0", "KEL_Spell2" }
    .Select(k => { var p = k.Split('_'); return $"Value={p[0]},{p[1]},{10 * mult}\r\n"; }));
string DmgBase = Dmg(1);

var cases = new List<(string Name, string Text, string? Original)>
{
    ("missing-everything", "", null),
    ("morale-patched", Morale("0", "0", "99999999", "500"), null),
    ("morale-first-unpatched-then-patched", Morale("5", "0", "99999999", "500") + Morale("0", "0", "99999999", "500"), null),
    ("morale-first-patched-then-unpatched", Morale("0", "0", "99999999", "500") + Morale("5", "5", "1", "1"), null),
    ("morale-negative-zero", Morale("-0", "0", "99999999", "500"), null),
    ("morale-plus-zero", Morale("+0", "0", "99999999", "500"), null),
    ("morale-int32-overflow", Morale("0", "0", "99999999999", "500"), null),
    ("morale-int32-max", Morale("0", "0", "2147483647", "500"), null),
    ("morale-int32-max-plus1", Morale("0", "0", "2147483648", "500"), null),
    ("morale-leading-zeros-fraction", Morale("000", "0", "0099999999", "500.7"), null),
    ("morale-threshold-minus1", Morale("0", "0", "99999998", "500"), null),
    ("morale-idle-501", Morale("0", "0", "99999999", "501"), null),
    ("morale-missing-line", "MoralsDecLostMem=GER,0\r\n", null),
    ("morale-wrong-faction", Morale("0", "0", "99999999", "500").Replace("MoralsDecFlee=GER", "MoralsDecFlee=ROM"), null),
    ("morale-spaced", "MoralsDecLostMem = GER , 0\r\nMoralsDecFlee\t=\tGER,0\r\nMoralsDecOverPop=GER ,  99999999\r\nMoralsIncIdle=GER,500\r\n", null),
    ("morale-indented-still-matches-unanchored", "  " + Morale("0", "0", "99999999", "500").Replace("\r\n", "\r\n  "), null),
    ("morale-lowercase-faction", Morale("0", "0", "99999999", "500").Replace("GER", "ger"), null),
    ("civi-ok", "CiviDelay=GER,100\r\nCiviDelay=ROM,500\r\n", null),
    ("civi-501", "CiviDelay=GER,100\r\nCiviDelay=ROM,501\r\n", null),
    ("civi-zero", "CiviDelay=GER,0\r\n", null),
    ("civi-negative", "CiviDelay=GER,-5\r\n", null),
    ("civi-lowercase", "CiviDelay=ger,5\r\n", null),
    ("civi-spaced", "CiviDelay = GER , 250\r\n", null),
    ("civi-fraction", "CiviDelay=GER,500.9\r\n", null),
    ("civi-huge-digits", "CiviDelay=GER,99999999999999999999\r\n", null),
    ("civi-one-bad-among-good", "CiviDelay=GER,1\r\nCiviDelay=ROM,600\r\nCiviDelay=KEL,2\r\n", null),
    ("res-both", "Value=KEL,Spell3,100\r\nValue2=KEL,Spell3,100\r\n", Orig),
    ("res-value2-missing", "Value=KEL,Spell3,100\r\n", Orig),
    ("res-value-missing", "Value2=KEL,Spell3,100\r\n", Orig),
    ("res-value2-original", "Value=KEL,Spell3,100\r\nValue2=KEL,Spell3,60\r\n", Orig),
    ("res-already-100-original", "Value=KEL,Spell3,100\r\n", "Value=KEL,Spell3,100\r\n"),
    ("res-no-backup", "Value=KEL,Spell3,100\r\nValue2=KEL,Spell3,100\r\n", null),
    ("res-fraction-truncates", "Value=KEL,Spell3,100.9\r\nValue2=KEL,Spell3,100.2\r\n", Orig),
    ("res-lowercase-key-case-insensitive-dict", "Value=KEL,Spell3,100\r\nValue2=KEL,Spell3,100\r\n", Orig.Replace("Value=KEL", "Value=KEL")),
    ("dmg-indented", "  Value=GER,Spell3,50\r\n", "Value=GER,Spell3,10\r\n"),
    ("dmg-trailing-comment", Dmg(5).Replace("\r\n", " ; x5\r\n"), DmgBase),
    ("dmg-5x-all-listed-keys", Dmg(5), DmgBase),
    ("dmg-5x-spell3-only", "Value=GER,Spell3,50\r\n", "Value=GER,Spell3,10\r\n"),
    ("dmg-5x-one-key-missing", Dmg(5).Replace("Value=HUN,Spell2,50\r\n", ""), DmgBase),
    ("dmg-lowercase-faction", "Value=ger,Spell3,50\r\n", "Value=GER,Spell3,10\r\n"),
    ("dmg-unchanged", DmgBase, DmgBase),
    ("heal-10x", "Value=KEL,Spell1,100\r\n", "Value=KEL,Spell1,10\r\n"),
    ("heal-9x", "Value=KEL,Spell1,90\r\n", "Value=KEL,Spell1,10\r\n"),
    ("heal-original-zero", "Value=KEL,Spell1,100\r\n", "Value=KEL,Spell1,0\r\n"),
    ("radius-3x", "Radius=GER,Spell1,30\r\n", "Radius=GER,Spell1,10\r\n"),
    ("radius-7.5x", "Radius=GER,Spell1,75\r\n", "Radius=GER,Spell1,10\r\n"),
    ("radius-31", "Radius=GER,Spell1,31\r\n", "Radius=GER,Spell1,10\r\n"),
    ("radius-tolerance", "Radius=GER,Spell1,30.4\r\n", "Radius=GER,Spell1,10\r\n"),
    ("radius-missing-current", "", "Radius=GER,Spell1,10\r\n"),
    ("radius-two-keys-one-bad", "Radius=GER,Spell1,30\r\nRadius=ROM,Spell2,20\r\n", "Radius=GER,Spell1,10\r\nRadius=ROM,Spell2,10\r\n"),
    ("radius-zero-baseline", "Radius=GER,Spell1,30\r\n", "Radius=GER,Spell1,0\r\n"),
    ("ability-5x", "Value=GER,SAbility1,20\r\n", "Value=GER,SAbility1,4\r\n"),
    ("ability-unchanged", "Value=GER,SAbility1,4\r\n", "Value=GER,SAbility1,4\r\n"),
    ("ability-5x-two-keys-one-bad", "Value=GER,SAbility1,20\r\nValue2=GER,SAbility1,5\r\n", "Value=GER,SAbility1,4\r\nValue2=GER,SAbility1,4\r\n"),
    ("ability-missing-current", "", "Value=GER,SAbility1,4\r\n"),
    ("ability-fraction", "Value=GER,SAbility1,20.9\r\n", "Value=GER,SAbility1,4.1\r\n"),
    ("ability-non-numeric", "Value=GER,SAbility1,abc\r\n", "Value=GER,SAbility1,4\r\n"),
    ("combined-everything", Morale("0", "0", "99999999", "500") + "CiviDelay=GER,100\r\n" + Dmg(5) + "Value=KEL,Spell1,100\r\nValue=KEL,Spell3,100\r\nValue2=KEL,Spell3,100\r\nRadius=GER,Spell1,30\r\nValue=GER,SAbility1,20\r\n",
        DmgBase + "Value=KEL,Spell1,10\r\n" + Orig + "Radius=GER,Spell1,10\r\nValue=GER,SAbility1,4\r\n"),
};

Dictionary<string, bool> Run(string text, string? original, bool initial)
{
    string root = Path.Combine(Path.GetTempPath(), "argm-clscript-oracle-" + Guid.NewGuid().ToString("N"));
    Directory.CreateDirectory(Path.Combine(root, "SYSTEM"));
    try
    {
        File.WriteAllBytes(Path.Combine(root, "SYSTEM", "cl_script.ini"), Encoding.GetEncoding(1251).GetBytes(text));
        var logger = new NullLogger();
        var backup = new BackupManager(logger);
        if (original != null) backup.SetBackupFile("SYSTEM/cl_script.ini", Encoding.GetEncoding(1251).GetBytes(original));
        var profile = new PatchProfile();
        foreach (string flag in flags) profile.Set(flag, FeatureValue.Of(initial));
        new ClScriptFeatureDetector(logger).Detect(root, backup, profile);
        return flags.ToDictionary(f => f, f => profile.GetBool(f));
    }
    finally { Directory.Delete(root, true); }
}

string Hash(string path) => Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path)));
var output = new
{
    Sources = new Dictionary<string, string>
    {
        ["ClScriptFeatureDetector"] = Hash("src.Core/Core/Features/Ini/ClScriptFeatureDetector.cs"),
        ["ClScriptPatcher"] = Hash("src.Core/Core/Patches/ClScriptPatcher.cs"),
    },
    Flags = flags,
    Cases = cases.Select(c => new
    {
        c.Name, c.Text, c.Original,
        FromFalse = Run(c.Text, c.Original, false),
        FromTrue = Run(c.Text, c.Original, true),
    }).ToArray(),
};
File.WriteAllText(args[0], JsonSerializer.Serialize(output, new JsonSerializerOptions { WriteIndented = true }));
Console.WriteLine($"C# cl_script oracle: {cases.Count} cases x2 profiles -> {args[0]}");

sealed class NullLogger : ILogger { public void Log(string message) { } }

