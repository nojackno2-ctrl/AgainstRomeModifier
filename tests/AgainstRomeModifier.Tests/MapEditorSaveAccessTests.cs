using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Theory]
    [InlineData("ENDL_000", false)]
    [InlineData("ENDL_004", false)]
    [InlineData("ENDL_005", true)]
    [InlineData("ENDL_999", true)]
    public void Save_rechecks_original_slot_or_removed_marker_before_opening_map_files(string id, bool removeMarker)
    {
        string map = CreateFixture(id);
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo(id, map, true, "Stale selection", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<TextBox>(form, "_title").Text = "Pending title";
            if (removeMarker) File.Delete(Path.Combine(map, CustomMapManifest.MarkerFileName));
            var before = SnapshotDirectory(map);
            using (var locked = new FileStream(Path.Combine(map, "TEXT", "US", "briefing.put"), FileMode.Open, FileAccess.Read, FileShare.None))
            {
                Assert.False(form.TrySaveMap(false, out Exception? error));
                Assert.IsType<InvalidOperationException>(error); // Not an IO error from opening the locked briefing.
            }
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.Equal("Pending title", GetField<TextBox>(form, "_title").Text);
            var after = SnapshotDirectory(map);
            Assert.Equal(before.Keys.Order(), after.Keys.Order());
            foreach (var (path, bytes) in before) Assert.Equal(bytes, after[path]);
            if (removeMarker)
            {
                File.WriteAllText(Path.Combine(map, CustomMapManifest.MarkerFileName), "{}");
                Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
                Assert.False(GetProperty<bool>(form, "IsDirty"));
                Assert.Equal("Pending title", PutTextDocument.Load(Path.Combine(map, "TEXT", "US", "briefing.put")).GetValue("briefing_titel_1"));
            }
        });
    }

    [Theory]
    [InlineData("ENDL_000", false)]
    [InlineData("ENDL_004", false)]
    [InlineData("ENDL_005", true)]
    [InlineData("ENDL_999", true)]
    public void Both_catalogs_classify_marked_original_maps_as_read_only(string id, bool expectedCustom)
    {
        string map = CreateFixture(id);
        Assert.True(File.Exists(Path.Combine(map, CustomMapManifest.MarkerFileName)));
        Assert.Equal(expectedCustom, new GameMapCatalog().Require(_root, id).IsCustom);
        Assert.Equal(expectedCustom, new EndlessMapCatalog().Require(_root, int.Parse(id.AsSpan(5))).IsCustom);
    }

    [Fact]
    public void Save_refuses_a_marked_map_from_another_root_before_opening_files()
    {
        string originalMap = CreateFixture("ENDL_011");
        string otherMaps = Path.Combine(_root, "OtherGame", "MAPS");
        Directory.CreateDirectory(otherMaps);
        string map = Path.Combine(otherMaps, "ENDL_011");
        Directory.Move(originalMap, map); // Both paths are under this test's synthetic TEMP root.
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_011", map, true, "Wrong root", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<TextBox>(form, "_title").Text = "Must stay pending";
            var before = SnapshotDirectory(map);
            using (var locked = new FileStream(Path.Combine(map, "TEXT", "US", "briefing.put"), FileMode.Open, FileAccess.Read, FileShare.None))
            {
                Assert.False(form.TrySaveMap(false, out Exception? error));
                Assert.IsType<InvalidOperationException>(error);
            }
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            var after = SnapshotDirectory(map);
            Assert.Equal(before.Keys.Order(), after.Keys.Order());
            foreach (var (path, bytes) in before) Assert.Equal(bytes, after[path]);
        });
    }
}
