using System.Drawing;
using System.Numerics;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Placed_troop_draws_six_soldiers_and_banner_picks_one_spawn_and_saves_no_extra_objects()
    {
        string map = CreateFixture();
        string defaults = Path.Combine(_root, "SYSTEM", "DATA_MP", "DEFAULTS");
        Directory.CreateDirectory(defaults);
        // 合成線段刻意不同於 NativeDefault，驗證宿主確實讀取 PFIL 表格。
        string geometry = "[FormationDefault]\r\n0,1,1,-1,0,1,0" + string.Concat(Enumerable.Repeat(",0", 76));
        byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
        File.WriteAllBytes(Path.Combine(defaults, "formdef.dau"), GameLZSS.CompressPfil(MapTextEncoding.Game.GetBytes(geometry), header));
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile), ScenarioEventsTests.Fixture().Serialize());
        string official = Path.Combine(_root, "MAPS", "HIST_FORMATION");
        Directory.CreateDirectory(official);
        File.WriteAllText(Path.Combine(official, "troop.sdl"),
            "[settlement]\r\nrefpos=0,0,0\r\n[object0000]\r\nnamedef=VerGerKamIco00_Kampf_Icon\r\ndef=426\r\nonload=1\r\nobjdefn0=GER_INF01\r\n");
        var sdlBefore = Directory.GetFiles(map, "*.sdl").ToDictionary(path => Path.GetFileName(path)!, File.ReadAllBytes);
        var dataBefore = SnapshotDirectory(Path.Combine(map, "DATA"));
        const string soldier = "FigGerInf01_Schwert", banner = "VerGerKamIco00_Kampf_Icon";

        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Formation", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }

            string Row(int id, string name) => string.Join(',', Enumerable.Range(0, 60).Select(i => i switch {
                0 => id.ToString(), 5 => "0", 8 => "-1", 14 => "-1", 17 => "0", 52 => name, _ => "0" }));
            using var catalog = NativeSpriteCatalog.FromText(Row(217, soldier) + "\n" + Row(426, banner),
                "0000,soldier.alr", "", _ => SolidAlr(32, 48), _ => null);
            typeof(MapEditorForm).GetField("_spriteCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, catalog);
            view.SpriteCatalog = catalog;
            typeof(MapEditorForm).GetField("_objdefNames", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form,
                new Dictionary<int, string> { [217] = soldier, [426] = banner });
            var type = new SdlObjectType(soldier, 217, SdlObjectCategory.Figure, "Ger", 1, new Dictionary<string, string> { ["alias"] = "GER_INF01" });
            typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { type });
            var spawn = new SdlPlacedObject(type, 8192, 0, 8192, 2, 90, 6) { ScenarioId = Guid.NewGuid() };
            form.PlacementSession.Add(spawn);
            Invoke(form, "RefreshPlacedList");
            _ = form.PlacedList.Handle; // 隱藏分頁的 ListView 也建立 native selection 狀態。
            IReadOnlyList<MapSceneObject> Scene()
            {
                var effective = (IReadOnlyList<MapSceneObject>)Invoke(form, "EffectiveSceneObjects")!;
                Assert.Single(effective, p => p.SourceFile == SdlPlacedObjectsFile.FileName); // 2D 單一圖示
                var scene = (IReadOnlyList<MapSceneObject>)Invoke(form, "SceneObjectsFor3D", effective)!;
                Invoke(form, "PushSceneObjects", effective);
                return scene;
            }
            var scene = Scene();
            var members = scene.Where(p => p.Name == soldier).ToArray();
            Assert.Equal(6, members.Length);
            Assert.Equal(7, view.SpriteObjectCount); // 六名士兵，另加旗幟
            var flag = Assert.Single(scene, p => p.Name == banner);
            Assert.Equal((8192f, 8192f), (flag.WorldX, flag.WorldZ));
            for (int i = 0; i < 6; i++)
            {
                Assert.Equal(8192 + 320 - (i + .5f) * 640 / 6, members[i].WorldX, 2);
                Assert.Equal(8192, members[i].WorldZ, 2);
                Assert.Equal((2, 90f), (members[i].Team, members[i].Angle));
                Assert.Equal((flag.SourceFile, flag.ObjectIndex), (members[i].SourceFile, members[i].ObjectIndex));
                Invoke(form, "SelectPickedSceneObject", members[i]);
                Assert.Equal(0, (int)Assert.Single(form.PlacedList.SelectedItems.Cast<ListViewItem>()).Tag!);
            }
            // 實際 opaque sprite hit 回到同一部隊識別，沒有獨立 SDL 成員。
            var heights = new TerrainHeightField(257, 257, new byte[257 * 257], tileWidth: 64, tileHeight: 64);
            var camera = new EditorCamera { Target = new Vector3(members[0].WorldX / 256, 0, members[0].WorldZ / 256) };
            camera.Zoom(.05f);
            var sprite = catalog.GetSprite(soldier)!;
            var projection = camera.GetProjectionMatrix(1);
            Vector4 eye = Vector4.Transform(new Vector4(SceneObjectRenderer.GroundPoint(members[0], heights), 1), camera.GetViewMatrix());
            Vector4 clip = Vector4.Transform(eye + new Vector4(0, 24 * SceneObjectRenderer.TilesPerSpritePixel, 0, 0), projection);
            var point = new Vector2((clip.X / clip.W * .5f + .5f) * 800, (.5f - clip.Y / clip.W * .5f) * 800);
            Assert.Equal(0, SceneObjectRenderer.PickObject([members[0]], [sprite], _ => true, heights,
                camera.GetViewMatrix(), projection, new Vector2(800), point));
            using Bitmap frame = view.CaptureFrame(640, 480)!;
            Assert.NotEmpty(Red(frame));

            // Ver…Ico 的 objdefn0 別名與數字 ID 兩種結構均可解析。
            foreach (string? member in new string?[] { "GER_INF01", "217", soldier, null })
            {
                var fields = new Dictionary<string, string> { ["alias"] = "GROUP" };
                if (member is not null) fields["objdefn0"] = member;
                var group = new SdlObjectType(banner, 426, SdlObjectCategory.UnitGroup, "Ger", 1,
                    fields);
                form.PlacementSession.Replace(0, spawn with { Type = group });
                Assert.Equal(6, Scene().Count(p => p.Name == soldier));
                Assert.Equal(7, view.SpriteObjectCount);
            }
            form.PlacementSession.Replace(0, spawn);
            Assert.True(form.TrySaveMap(showSuccess: false, out Exception? error), error?.ToString());
            var saved = Assert.Single(ScenarioDocument.Load(map).Spawns);
            Assert.Equal(("GER_INF01", 6, 90), (saved.Alias, saved.Count, saved.Angle));
            Assert.Single(form.PlacementSession.Capture());
            Assert.Equal(7, view.SpriteObjectCount);
            Assert.False(File.Exists(Path.Combine(map, SdlPlacedObjectsFile.FileName)));
        }, TimeSpan.FromMinutes(2));

        Assert.Equal(sdlBefore.Keys.Order(), Directory.GetFiles(map, "*.sdl").Select(Path.GetFileName).Order());
        foreach (var (name, bytes) in sdlBefore) Assert.Equal(bytes, File.ReadAllBytes(Path.Combine(map, name!)));
        var dataAfter = SnapshotDirectory(Path.Combine(map, "DATA"));
        Assert.Equal(dataBefore.Keys.Order(), dataAfter.Keys.Order());
        foreach (var (name, bytes) in dataBefore) Assert.Equal(bytes, dataAfter[name]);
    }
}
