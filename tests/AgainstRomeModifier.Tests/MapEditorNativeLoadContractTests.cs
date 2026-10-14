using System.Drawing;
using System.Buffers.Binary;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Diagnostics;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Native_data_check_lists_missing_files_and_preview_rechecks_disk_without_writes()
    {
        string map = CreateFixture();
        string data = Path.Combine(map, "DATA");
        Directory.CreateDirectory(data);
        // File presence is the contract here; empty payloads must not be claimed as validated.
        foreach (string file in NativeMapLoadContract.RequiredDataFiles)
            if (!File.Exists(Path.Combine(data, file))) File.WriteAllBytes(Path.Combine(data, file), []);
        File.Delete(Path.Combine(data, "rain.dat"));
        File.Delete(Path.Combine(data, "formatio.dat"));
        // A directory with the right name cannot satisfy an open-for-reading contract.
        Directory.CreateDirectory(Path.Combine(data, "rain.dat"));
        var before = SnapshotDirectory(map);
        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Native check", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents(); Invoke(form, "SetActiveView", false);
            var issue = Assert.Single(form.RefreshMapDiagnostics(), i => i.Code == "native-data-missing");
            Assert.Equal(MapIssueSeverity.Warning, issue.Severity);
            Assert.Contains("rain.dat, formatio.dat", issue.Chinese);
            Assert.Contains("rain.dat, formatio.dat", issue.English);
            var rows = GetField<ListView>(form, "_mapIssues");
            Assert.Contains(rows.Items.Cast<ListViewItem>(), row => Equals(row.Tag, issue));
            Assert.False(form.ValidateNativeMapForGamePreview(out string? error));
            Assert.Contains("rain.dat", error);
            AssertSnapshotUnchanged(map, before);
            Directory.Delete(Path.Combine(data, "rain.dat"));
            File.WriteAllBytes(Path.Combine(data, "rain.dat"), []);
            File.WriteAllBytes(Path.Combine(data, "formatio.dat"), []);
            Assert.True(form.ValidateNativeMapForGamePreview(out error));
            Assert.Null(error);
            Assert.DoesNotContain(form.RefreshMapDiagnostics(), i => i.Code == "native-data-missing");
        });
    }

    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(2)]
    public void Saved_native_third_position_fault_is_visible_in_map_check_without_writes(int definitionMode)
    {
        bool requiredByDefinition = definitionMode == 1;
        string map = CreateFixture();
        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Native check", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents(); Invoke(form, "SetActiveView", false);
            byte[] objects = new byte[16 + 79 + LevelObjectStore.ColumnWidths.Sum()];
            BinaryPrimitives.WriteInt32LittleEndian(objects, 1);
            BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(4), 1);
            BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(8), 30);
            BinaryPrimitives.WriteInt32LittleEndian(objects.AsSpan(12), 30);
            objects[16] = 1;
            int column = 16 + 79 + LevelObjectStore.ColumnWidths.Take(10).Sum();
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(column), requiredByDefinition ? ushort.MaxValue : (ushort)2);
            byte[] positions = new byte[8 + 17];
            BinaryPrimitives.WriteInt32LittleEndian(positions, 1);
            BinaryPrimitives.WriteInt32LittleEndian(positions.AsSpan(4), 1); positions[8] = 1;
            string data = Path.Combine(map, "DATA"); Directory.CreateDirectory(data);
            File.WriteAllBytes(Path.Combine(data, "objects.dat"), objects);
            File.WriteAllBytes(Path.Combine(data, "position.dat"), positions);
            if (definitionMode != 0)
            {
                string path = Path.Combine(_root, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau"); Directory.CreateDirectory(Path.GetDirectoryName(path)!);
                string[] fields = Enumerable.Repeat("0", 61).ToArray(); fields[1] = "1"; fields[60] = definitionMode == 2 ? "broken" : "1";
                File.WriteAllText(path, "[ObjectDefaults]\n" + string.Join(',', fields));
                if (requiredByDefinition)
                {
                    string alternate = Path.Combine(_root, "CUSTOM_DEFINITIONS"); Directory.CreateDirectory(alternate);
                    fields[60] = "0"; File.WriteAllText(Path.Combine(alternate, "objdef.dau"), "[ObjectDefaults]\n" + string.Join(',', fields));
                    fields[60] = "1"; File.WriteAllText(Path.Combine(alternate, "objdef.d09"), "[ObjectDefaults]\n" + string.Join(',', fields));
                    File.WriteAllText(Path.Combine(_root, "SYSTEM", "clengine.ini"), "[DefaultPath]\nCUSTOM_DEFINITIONS\\ ; test\n");
                }
            }
            var before = SnapshotDirectory(map);
            var issue = Assert.Single(form.RefreshMapDiagnostics(), i => i.Code == (requiredByDefinition ? "native-data-position-2-required" : "native-data-position-2-invalid"));
            Assert.Equal(MapIssueSeverity.Warning, issue.Severity);
            if (definitionMode == 2) Assert.Contains(form.RefreshMapDiagnostics(), i => i.Code == "native-object-definitions-invalid");
            Assert.Contains("第三位置", issue.Chinese);
            Assert.Contains("Saved DATA", issue.English);
            Assert.Contains(GetField<ListView>(form, "_mapIssues").Items.Cast<ListViewItem>(), row => Equals(row.Tag, issue));
            AssertSnapshotUnchanged(map, before);
        });
    }
}
