using AgainstRomeModifier;
using AgainstRomeModifier.Scripting;
using AgainstRomeMapEditor.Modules.Cinematics;
if(args.Length>0&&args[0]=="scenario-batch") return ScenarioBatch(args);
var original=File.ReadAllBytes(args[0]);
var raw=original.AsSpan().StartsWith("PFIL"u8)?GameLZSS.DecompressPfil(original):original;
var image=BciImage.Parse(raw);
var scenario=ScenarioDocument.Load(args[1]);
var main=image.MainAddress;
if(scenario.ScriptSpawns.Count>0) LevelScriptInjector.Inject(image,scenario.ScriptSpawns);
ScenarioEventCompiler.Inject(image,scenario.Events,main,scenario);
if(args.Length>3) MapStartCameraInjector.Inject(image,MapStartCamera.Load(args[3]));
File.WriteAllBytes(args[2],image.Serialize());
return 0;

// scenario-batch <casesDir> <outDir> <idle.bci>
// For every <casesDir>/<name>.json runs the shipped C# Load -> Save -> Apply-equivalent compile pipeline and
// records one verdict file per stage (<name>.load/.save/.compile = "ok" or "err ...") plus the produced artifacts.
static int ScenarioBatch(string[] args)
{
    string cases=args[1], outDir=args[2];
    byte[] idle=File.ReadAllBytes(args[3]);
    string[] aliases=["unit","house"];
    Directory.CreateDirectory(outDir);
    foreach(string file in Directory.GetFiles(cases,"*.json").OrderBy(p=>p,StringComparer.Ordinal))
    {
        string name=Path.GetFileNameWithoutExtension(file);
        string input=Path.Combine(outDir,name+".in"), saved=Path.Combine(outDir,name+".saved");
        Directory.CreateDirectory(input);
        File.Copy(file,Path.Combine(input,ScenarioDocument.FileName),true);
        ScenarioDocument? document=null;
        try { document=ScenarioDocument.Load(input); Verdict(outDir,name,"load","ok"); }
        catch(Exception e) { Verdict(outDir,name,"load","err "+e.GetType().Name+": "+e.Message); }
        if(document is null) continue;
        try
        {
            Directory.CreateDirectory(saved);
            using var rollback=new FileRollbackScope();
            document.Save(saved,rollback);
            rollback.Commit();
            Verdict(outDir,name,"save","ok");
        }
        catch(Exception e) { Verdict(outDir,name,"save","err "+e.GetType().Name+": "+e.Message); continue; }
        try
        {
            // Mirrors LevelScriptInjector.Apply on an unpacked idle script, starting from the saved (re-loaded) document.
            ScenarioDocument reloaded=ScenarioDocument.Load(saved);
            ScenarioEventValidator.Validate(reloaded.Events,aliases);
            ScenarioEventValidator.ValidateConditions(reloaded.Events,reloaded);
            bool hasEvents=reloaded.Events.Any(item=>item.Enabled);
            var spawns=reloaded.ScriptSpawns;
            foreach(var spawn in spawns)
                if(!aliases.Contains(spawn.Alias,StringComparer.OrdinalIgnoreCase)) throw new InvalidDataException("unknown alias "+spawn.Alias);
            byte[] output=idle;
            if(spawns.Count>0||hasEvents)
            {
                var bci=BciImage.Parse(idle);
                int originalMain=bci.MainAddress;
                if(spawns.Count>0) LevelScriptInjector.Inject(bci,spawns);
                if(hasEvents) ScenarioEventCompiler.Inject(bci,reloaded.Events,originalMain,reloaded);
                output=bci.Serialize();
            }
            File.WriteAllBytes(Path.Combine(outDir,name+".bci"),output);
            Verdict(outDir,name,"compile","ok");
        }
        catch(Exception e) { Verdict(outDir,name,"compile","err "+e.GetType().Name+": "+e.Message); }
    }
    return 0;
}
static void Verdict(string outDir,string name,string stage,string text)
    => File.WriteAllText(Path.Combine(outDir,name+"."+stage),text.ReplaceLineEndings(" "));
