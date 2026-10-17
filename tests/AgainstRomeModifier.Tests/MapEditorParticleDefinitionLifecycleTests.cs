using System.Reflection;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    private MapEditorForm ParticleDefinitionForm(string map)
    {
        var form = new MapEditorForm(_root, new GameMapInfo(Path.GetFileName(map), map, true, "Particle definitions", "Test"));
        _ = form.Handle;
        var type = new SdlObjectType("BauGerTest", 2, SdlObjectCategory.Building, "Ger", 1,
            new Dictionary<string, string> { ["alias"] = "BauTest" });
        typeof(MapEditorForm).GetField("_objectCatalog", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new[] { type });
        typeof(MapEditorForm).GetField("_objdefNames", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form,
            new Dictionary<int, string> { [555] = "BauGerTest", [556] = "BauGerChild" });
        Invoke(form, "LoadSelectedMap"); return form;
    }
    private void SetupParticleDefinitionSources()
    {
        string folder = Path.Combine(_root, "SelectedDefs"); Directory.CreateDirectory(folder);
        Directory.CreateDirectory(Path.Combine(_root, "SYSTEM"));
        File.WriteAllText(Path.Combine(_root, "SYSTEM", "clengine.ini"), "[DefaultPath]\nSelectedDefs/\n");
        File.WriteAllText(Path.Combine(folder, "partpar.dau"), "[ParticleParameterDefault]\n22,1,Arrow,64,15000\n");
        File.WriteAllText(Path.Combine(folder, "partgeo.dau"), "[ParticleGeometrieDefault]\n23,1,Geometry\n");
        File.WriteAllText(Path.Combine(folder, "partgfx.dau"), "[ParticleGrafikDefault]\n24,1,Grafik,-1,227\n");
        string Row(int id, int divisor, int par, int geo, int gfx)
        {
            string[] columns = Enumerable.Repeat("0", 66).ToArray();
            foreach (var pair in new[] { (0,id), (1,1), (2,divisor), (5,258), (63,par), (64,geo), (65,gfx) })
                columns[pair.Item1] = pair.Item2.ToString(System.Globalization.CultureInfo.InvariantCulture);
            columns[52] = "Object" + id;
            return string.Join(',', columns);
        }
        File.WriteAllText(Path.Combine(folder, "objdef.dau"), "[ObjectDefaults]\n" + Row(830,999,22,23,24) + "\n" + Row(227,1000,-1,-1,-1) + "\n");
    }
    private static object ParticleDefinitionCatalog(MapEditorForm form)
        => typeof(MapEditorForm).GetField("_particleDefinitions", BindingFlags.NonPublic | BindingFlags.Instance)!.GetValue(form)!;
    private static int? ParticleDefinitionDivisor(MapEditorForm form, string property = "TypeDivisor")
    {
        object?[] arguments = [830, null, null];
        bool resolved = (bool)typeof(MapEditorForm).GetMethod("TryResolveParticleDefinition", BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(form, arguments)!;
        return resolved ? (int)arguments[1]!.GetType().GetProperty(property)!.GetValue(arguments[1])! : null;
    }
    private static void AssertParticleCatalogDisposed(object catalog)
    {
        var error = Assert.Throws<TargetInvocationException>(() => catalog.GetType().GetMethod("TryResolve", BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(catalog, [830, null, null]));
        Assert.IsType<ObjectDisposedException>(error.InnerException);
    }

    [Fact]
    public void ParticleDefinitionHost_reload_observes_overlay_and_releases_previous_handle()
    {
        string map = CreateFixture("ENDL_012"); SetupParticleDefinitionSources();
        var disk = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = ParticleDefinitionForm(map);
            Assert.Equal(1000, ParticleDefinitionDivisor(form));
            object old = ParticleDefinitionCatalog(form);
            File.WriteAllText(Path.Combine(_root, "SelectedDefs", "objdef.d09"), "[ObjectDefaults]\n227,1,250\n");
            Invoke(form, "LoadSelectedMap");
            Assert.Equal(250, ParticleDefinitionDivisor(form)); Assert.Null(form.ParticleDefinitionDiagnostic);
            AssertParticleCatalogDisposed(old);
            object current = ParticleDefinitionCatalog(form); form.Dispose(); AssertParticleCatalogDisposed(current);
        });
        var after = SnapshotDirectory(map); Assert.Equal(disk.Keys.Order(), after.Keys.Order());
        foreach (string path in disk.Keys) Assert.Equal(disk[path], after[path]);
    }

    [Fact]
    public void ParticleDefinitionHost_failed_reload_keeps_old_handle_but_blocks_stale_resolution()
    {
        string map = CreateFixture("ENDL_012"); SetupParticleDefinitionSources();
        RunInSta(() =>
        {
            using var form = ParticleDefinitionForm(map); object old = ParticleDefinitionCatalog(form);
            File.WriteAllText(Path.Combine(_root, "SelectedDefs", "partpar.d09"), "[ParticleParameterDefault]\n22,2,N,64,invalid\n");
            Invoke(form, "LoadSelectedMap");
            Assert.Same(old, ParticleDefinitionCatalog(form)); Assert.NotNull(form.ParticleDefinitionDiagnostic);
            object?[] oldQuery = [830, null, null];
            Assert.True((bool)old.GetType().GetMethod("TryResolve", BindingFlags.NonPublic | BindingFlags.Instance)!.Invoke(old, oldQuery)!);
            Assert.Null(ParticleDefinitionDivisor(form));
            File.WriteAllText(Path.Combine(_root, "SelectedDefs", "partpar.d09"), "[ParticleParameterDefault]\n22,2,N,64,5000\n");
            Invoke(form, "LoadSelectedMap"); Assert.Equal(5000, ParticleDefinitionDivisor(form, "Lifetime"));
            Assert.Null(form.ParticleDefinitionDiagnostic); AssertParticleCatalogDisposed(old);
        });
    }

    [Fact]
    public void ParticleDefinitionHost_switch_and_empty_selection_release_catalogs()
    {
        string map = CreateFixture("ENDL_012"), other = CreateFixture("ENDL_006"); SetupParticleDefinitionSources();
        RunInSta(() =>
        {
            using var form = ParticleDefinitionForm(map); object first = ParticleDefinitionCatalog(form);
            var selected = typeof(MapEditorForm).GetField("_selected", BindingFlags.NonPublic | BindingFlags.Instance)!;
            selected.SetValue(form, new AgainstRomeModifier.Maps.GameMapInfo("ENDL_006", other, true, "Other", "無盡模式"));
            Invoke(form, "LoadSelectedMap"); Assert.Equal(1000, ParticleDefinitionDivisor(form));
            AssertParticleCatalogDisposed(first); object second = ParticleDefinitionCatalog(form);
            selected.SetValue(form, null); Invoke(form, "LoadSelectedMap");
            Assert.Null(ParticleDefinitionCatalog(form)); Assert.Null(ParticleDefinitionDivisor(form));
            AssertParticleCatalogDisposed(second);
        });
    }

    [Fact]
    public void ParticleDefinitionHost_lifetime_frame_calls_Rust_and_unverified_direction_returns_no_frame()
    {
        string map = CreateFixture("ENDL_012"); SetupParticleDefinitionSources();
        RunInSta(() =>
        {
            using var form = ParticleDefinitionForm(map);
            var method = typeof(MapEditorForm).GetMethod("TryResolveParticleLifetimeFrame", BindingFlags.NonPublic | BindingFlags.Instance)!;
            object?[] arguments = [830, 14001, 258, 8, 1, 8u, null, null];
            Assert.True((bool)method.Invoke(form, arguments)!); Assert.Equal(7u, arguments[6]); Assert.Null(arguments[7]);
            arguments = [830, 14001, 258, 8, 32, 256u, null, null];
            Assert.False((bool)method.Invoke(form, arguments)!); Assert.Null(arguments[6]); Assert.Contains("direction", (string)arguments[7]!);
        });
    }

    [Fact]
    public void ParticleDefinitionHost_decodes_matching_asset_through_Rust_and_blocks_stale_sources()
    {
        string map = CreateFixture("ENDL_012"); SetupParticleDefinitionSources();
        using var stream = new MemoryStream();
        using (var writer = new BinaryWriter(stream, System.Text.Encoding.ASCII, leaveOpen: true))
        {
            foreach (uint word in new uint[] { 0x41524C41, 6, 0, 4, 8, 0, 4, 1, 1, 0,
                0, 0, 0, 0, 0, 12, 18, 0, 0, 0, 0, 0, 0 }) writer.Write(word);
            writer.Write(-1); writer.Write(12u); writer.Write(7u | (11u << 16));
            writer.Write(2u | (1u << 11) | (2u << 22)); writer.Write(2u);
            writer.Write(0x102030u); writer.Write(0x405060u); writer.Write(new byte[] { 1, 0, 0, 0 });
            writer.Write(0u); writer.Write(2u);
            writer.Write(0); writer.Write(0); writer.Write(0);
        }
        byte[] bytes = stream.ToArray();
        RunInSta(() =>
        {
            using var form = ParticleDefinitionForm(map);
            var catalogField = typeof(MapEditorForm).GetField("_spriteCatalog", BindingFlags.NonPublic | BindingFlags.Instance)!;
            object catalog = catalogField.FieldType.GetMethod("FromText", BindingFlags.Public | BindingFlags.Static)!
                .Invoke(null, new object?[] { "", "258,particle.alr\n", "", (Func<string, byte[]?>)(_ => bytes),
                    (Func<string, byte[]?>)(_ => null), null })!;
            catalogField.SetValue(form, catalog);
            var method = typeof(MapEditorForm).GetMethod("TryDecodeParticleLifetimeFrame", BindingFlags.NonPublic | BindingFlags.Instance)!;
            object?[] arguments = [830, 14001, 0, null, null];
            Assert.True((bool)method.Invoke(form, arguments)!); Assert.Null(arguments[4]);
            object image = arguments[3]!;
            Assert.Equal(3u, image.GetType().GetProperty("FrameIndex")!.GetValue(image));
            object pixels = image.GetType().GetProperty("Image")!.GetValue(image)!;
            Assert.Equal(new uint[] { 0xFF605040, 0 },
                (IReadOnlyList<uint>)pixels.GetType().GetProperty("ArgbPixels")!.GetValue(pixels)!);
            byte[] savedRecord = new byte[2252], savedState = new byte[16];
            savedRecord[0] = 1; savedRecord[2] = 2; savedRecord[12] = 1;
            System.Buffers.Binary.BinaryPrimitives.WriteInt32LittleEndian(savedRecord.AsSpan(13), -1);
            System.Buffers.Binary.BinaryPrimitives.WriteInt32LittleEndian(savedRecord.AsSpan(37), 14001);
            var batch = typeof(MapEditorForm).GetMethod("TryResolveSavedParticleFrameCandidates", BindingFlags.NonPublic | BindingFlags.Instance)!;
            object?[] batchArgs = [830, 0u, savedRecord, savedState, null, null, 0, 0, 0];
            Assert.True((bool)batch.Invoke(form, batchArgs)!); Assert.Null(batchArgs[5]);
            object candidate = Assert.Single(((System.Collections.IEnumerable)batchArgs[4]!).Cast<object>());
            Assert.Equal(3u, candidate.GetType().GetProperty("Frame")!.GetValue(candidate));
            Assert.Equal(-1, candidate.GetType().GetProperty("X")!.GetValue(candidate));
            Assert.Equal(0, candidate.GetType().GetProperty("Status")!.GetValue(candidate));
            File.WriteAllText(Path.Combine(_root, "SelectedDefs", "partpar.d09"), "[ParticleParameterDefault]\n22,1,N,64,invalid\n");
            Invoke(form, "LoadSelectedMap");
            arguments = [830, 14001, 0, null, null];
            Assert.False((bool)method.Invoke(form, arguments)!); Assert.Null(arguments[3]); Assert.NotNull(arguments[4]);
            batchArgs = [830, 0u, savedRecord, savedState, null, null, 0, 0, 0];
            Assert.False((bool)batch.Invoke(form, batchArgs)!);
            Assert.Empty(((System.Collections.IEnumerable)batchArgs[4]!).Cast<object>());
        });
    }
}
