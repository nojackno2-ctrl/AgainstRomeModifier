using AgainstRomeModifier;
using AgainstRomeModifier.Maps;

// Executes the actual existing managed implementation against a supplied TEMP map.
string map=args[0], output=args[1]; int slot=int.Parse(args[2]);uint uid=uint.Parse(args[3]);
Directory.CreateDirectory(output);
var store=LevelObjectStore.Load(map);
void Snapshot(string stage)
{
    using(var rollback=new FileRollbackScope()){store.Save(map,rollback);rollback.Commit();}
    string dir=Path.Combine(output,stage);Directory.CreateDirectory(dir);
    foreach(string path in Directory.GetFiles(Path.Combine(map,"DATA"),"*.dat"))
    {
        byte[] b=File.ReadAllBytes(path);
        if(b.AsSpan().StartsWith("PFIL"u8))b=GameLZSS.DecompressPfil(b);
        File.WriteAllBytes(Path.Combine(dir,Path.GetFileNameWithoutExtension(path)+".bin"),b);
    }
}
if(!store.TranslateFamilyIfUidFixed(slot,uid,65536,32768,-123))throw new InvalidOperationException("managed move refused");Snapshot("move");
var copy=store.CreateFamilyCopyCandidate(slot,uid,131072,-65536,4321)??throw new InvalidOperationException("managed clone refused");
store=copy.Store;File.WriteAllText(Path.Combine(output,"copy.txt"),$"{copy.RootSlot} {copy.RootUid}");Snapshot("clone");
if(!store.RemoveFamilyIfUid(copy.RootSlot,copy.RootUid))throw new InvalidOperationException("managed delete refused");Snapshot("delete");
Console.WriteLine($"oracle move/clone/delete root={slot}/{uid} copy={copy.RootSlot}/{copy.RootUid}");

