using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Text.Json;
using AgainstRomeMapEditor;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;

if (args.Length == 0)
{
    Console.WriteLine("usage: PlacementOracle [probe ... | batch-workflow <json_output> | batch-data <map> <output> <slot uid ...> | <map> <output> <slot> <uid>]");
    return;
}

if (args[0] == "batch-workflow")
{
    string outputJsonPath = args[1];
    RunBatchWorkflowOracle(outputJsonPath);
    return;
}

if (args[0] == "batch-data")
{
    string mapPath = args[1];
    string outputPath = args[2];
    RunBatchDataOracle(mapPath, outputPath, args.Skip(3).ToArray());
    return;
}

// probe SLOT UID MAP...: one line per map, "accept" iff the managed FamilyAt closure is valid (rejection parity).
if(args[0]=="probe")
{
    int probeSlot=int.Parse(args[1]);uint probeUid=uint.Parse(args[2]);
    foreach(string probeMap in args.Skip(3))
    {
        try
        {
            var family=LevelObjectStore.Load(probeMap).FamilyAt(probeSlot,probeUid);
            Console.WriteLine(family is null?"reject":$"accept objects={string.Join(",",family.ObjectSlots)} particles={string.Join(",",family.ParticleSlots)} lagers={string.Join(",",family.LagerSlots)} biglagers={string.Join(",",family.BigLagerSlots)} lights={string.Join(",",family.LightSlots.Order())}");
        }
        catch(Exception e) when(e is InvalidDataException||e is IOException||e is ArgumentException){Console.WriteLine("reject-load");}
    }
    return;
}
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

static void RunBatchWorkflowOracle(string outputJsonPath)
{
    Exception? error = null;
    var thread = new Thread(() =>
    {
        try
        {
            string tempDir = Path.Combine(Path.GetTempPath(), "ArmPlacementOracle_" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(tempDir);
            try
            {
                using var form = new MapEditorForm(tempDir, new GameMapInfo("TEST", tempDir, true, "Test", "Test"));
                _ = form.Handle;

                // Configure PlaceObject mode
                var setEditMode = typeof(MapEditorForm).GetMethod("SetEditMode", BindingFlags.Instance | BindingFlags.NonPublic)!;
                var modeType = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
                setEditMode.Invoke(form, new object[] { Enum.Parse(modeType, "PlaceObject") });

                // Seed synthetic placed objects
                var t1 = new SdlObjectType("BauGerHau00_Haupthaus", 1, SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string> { ["alias"] = "GER_HAU00" });
                var t2 = new SdlObjectType("FigGerWar00_Krieger", 2, SdlObjectCategory.Figure, "Ger", 1, new Dictionary<string, string> { ["alias"] = "GER_WAR00" });
                var t3 = new SdlObjectType("BauGerTow00_Turm", 3, SdlObjectCategory.Building, "Ger", 1, new Dictionary<string, string> { ["alias"] = "GER_TOW00" });

                var item0 = new SdlPlacedObject(t1, 400f, 20f, 600f, 1, 90f, 0) { ScenarioId = Guid.NewGuid() };
                var item1 = new SdlPlacedObject(t2, 850f, 10f, 1200f, 1, 180f, 5) { ScenarioId = Guid.NewGuid() };
                var item2 = new SdlPlacedObject(t3, 1500f, 15f, 2000f, 2, 270f, 0) { ScenarioId = Guid.NewGuid() };

                form.PlacementSession.Load(new[] { item0, item1, item2 });
                typeof(MapEditorForm).GetMethod("RefreshPlacedList", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(form, null);

                var stages = new Dictionary<string, object>();

                object SnapshotPlacements()
                {
                    return form.PlacementSession.Capture().Select((p, idx) => new
                    {
                        index = idx,
                        name = p.Type.NameDef,
                        category = p.Type.Category.ToString(),
                        x = (double)p.WorldX,
                        y = (double)p.WorldY,
                        z = (double)p.WorldZ,
                        team = p.Team,
                        angle = (double)p.Angle,
                        unitCount = p.UnitCount
                    }).ToList();
                }

                stages["initial"] = SnapshotPlacements();

                void SelectIndices(params int[] indices)
                {
                    _ = form.PlacedList.Handle;
                    form.PlacedList.SelectedItems.Clear();
                    foreach (int idx in indices) form.PlacedList.Items[idx].Selected = true;
                }

                // 1. PlacementWorkflow: MoveSelectedPlacements(256, -64, snap: false) on [0, 1]
                SelectIndices(0, 1);
                form.MoveSelectedPlacements(256f, -64f, snap: false);
                stages["translate"] = SnapshotPlacements();

                // 2. PlacementWorkflow: MoveSelectedPlacements(0, 0, snap: true) on [0, 1]
                float anchorXBefore = form.PlacementSession[0].WorldX;
                float anchorZBefore = form.PlacementSession[0].WorldZ;
                form.MoveSelectedPlacements(0, 0, snap: true);
                float snapDx = form.PlacementSession[0].WorldX - anchorXBefore;
                float snapDz = form.PlacementSession[0].WorldZ - anchorZBefore;
                stages["snap"] = new
                {
                    delta = new double[] { snapDx, 0.0, snapDz },
                    objects = SnapshotPlacements()
                };

                // 3. PlacementWorkflow: HandlePlacementRotationKey(Keys.R) on [0, 1, 2]
                SelectIndices(0, 1, 2);
                form.HandlePlacementRotationKey(new KeyEventArgs(Keys.R), selectedObjects: true);
                stages["rotate"] = SnapshotPlacements();

                // 4. PlacementBatchEditDialog on [0, 1]: apply Team=4 and Angle=315 via BatchEditDialogRunner
                SelectIndices(0, 1);
                form.BatchEditDialogRunner = dialog =>
                {
                    dialog.Team = 4;
                    dialog.Angle = 315f;
                    return DialogResult.OK;
                };
                form.EditSelectedPlacedObjectsBatch();
                stages["batch_edit"] = SnapshotPlacements();

                // 5. DuplicateSelectedPlacedObjects on [0, 1]
                SelectIndices(0, 1);
                form.DuplicateSelectedPlacedObjects();
                stages["clone"] = SnapshotPlacements();

                // 6. DeleteSelectedPlacedObjects on duplicates (which are now selected)
                form.DeleteSelectedPlacedObjects();
                stages["remove"] = SnapshotPlacements();

                string dir = Path.GetDirectoryName(Path.GetFullPath(outputJsonPath))!;
                if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir)) Directory.CreateDirectory(dir);
                string json = JsonSerializer.Serialize(stages, new JsonSerializerOptions { WriteIndented = true });
                File.WriteAllText(outputJsonPath, json);

                string tsvPath = Path.ChangeExtension(outputJsonPath, ".tsv");
                using (var sw = new StreamWriter(tsvPath))
                {
                    sw.WriteLine($"SNAP_DELTA\t{snapDx}\t0\t{snapDz}");
                    void WriteStageItems(string stageName, object itemsObj)
                    {
                        var jsonEl = JsonSerializer.SerializeToElement(itemsObj);
                        foreach (var el in jsonEl.EnumerateArray())
                        {
                            sw.WriteLine($"{stageName}\t{el.GetProperty("index").GetInt32()}\t{el.GetProperty("name").GetString()}\t{el.GetProperty("x").GetDouble()}\t{el.GetProperty("y").GetDouble()}\t{el.GetProperty("z").GetDouble()}\t{el.GetProperty("team").GetInt32()}\t{el.GetProperty("angle").GetDouble()}\t{el.GetProperty("unitCount").GetInt32()}");
                        }
                    }
                    WriteStageItems("initial", stages["initial"]);
                    WriteStageItems("translate", stages["translate"]);
                    WriteStageItems("snap", ((dynamic)stages["snap"]).objects);
                    WriteStageItems("rotate", stages["rotate"]);
                    WriteStageItems("batch_edit", stages["batch_edit"]);
                    WriteStageItems("clone", stages["clone"]);
                    WriteStageItems("remove", stages["remove"]);
                }

                Console.WriteLine($"batch-workflow oracle executed successfully -> {outputJsonPath} and {tsvPath}");
            }
            finally
            {
                if (Directory.Exists(tempDir)) Directory.Delete(tempDir, recursive: true);
            }
        }
        catch (Exception ex)
        {
            error = ex;
        }
    });
    thread.SetApartmentState(ApartmentState.STA);
    thread.Start();
    thread.Join();
    if (error != null) ExceptionDispatchInfo.Capture(error).Throw();
}

static void RunBatchDataOracle(string map, string output, string[] familyArgs)
{
    Directory.CreateDirectory(output);
    var store = LevelObjectStore.Load(map);
    var families = new List<(int Slot, uint Uid)>();
    for (int i = 0; i < familyArgs.Length; i += 2)
    {
        families.Add((int.Parse(familyArgs[i]), uint.Parse(familyArgs[i + 1])));
    }

    void SnapshotStage(string stage)
    {
        using (var rollback = new FileRollbackScope()) { store.Save(map, rollback); rollback.Commit(); }
        string dir = Path.Combine(output, stage);
        Directory.CreateDirectory(dir);
        foreach (string path in Directory.GetFiles(Path.Combine(map, "DATA"), "*.dat"))
        {
            byte[] b = File.ReadAllBytes(path);
            if (b.AsSpan().StartsWith("PFIL"u8)) b = GameLZSS.DecompressPfil(b);
            File.WriteAllBytes(Path.Combine(dir, Path.GetFileNameWithoutExtension(path) + ".bin"), b);
        }
    }

    // 1. Batch Translate: families by [65536, 0, -32768]
    foreach (var (slot, uid) in families)
    {
        if (!store.TranslateFamilyIfUidFixed(slot, uid, 65536, 0, -32768))
            throw new InvalidOperationException($"managed translate refused for slot {slot}");
    }
    SnapshotStage("translate");

    // 2. Batch Clone: families by [131072, 0, 65536]
    var clonedRoots = new List<(int Slot, uint Uid)>();
    foreach (var (slot, uid) in families)
    {
        var copy = store.CreateFamilyCopyCandidate(slot, uid, 131072, 0, 65536)
            ?? throw new InvalidOperationException($"managed clone refused for slot {slot}");
        store = copy.Store;
        clonedRoots.Add((copy.RootSlot, copy.RootUid));
    }
    File.WriteAllLines(Path.Combine(output, "cloned.txt"), clonedRoots.Select(c => $"{c.Slot} {c.Uid}"));
    SnapshotStage("clone");

    // 3. Batch Remove: remove cloned families
    foreach (var (slot, uid) in clonedRoots)
    {
        if (!store.RemoveFamilyIfUid(slot, uid))
            throw new InvalidOperationException($"managed delete refused for slot {slot}");
    }
    SnapshotStage("remove");

    Console.WriteLine($"batch-data differential completed across {families.Count} families");
}

