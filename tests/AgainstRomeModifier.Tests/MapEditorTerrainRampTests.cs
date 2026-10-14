using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Ramp_tool_creates_linear_ramp_with_single_undo_and_redo()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "RampTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            var session = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            Assert.NotNull(session);
            int vSize = session.VertexSize;
            Assert.True(vSize >= 2);

            form.ActivateRampTool();
            Assert.Null(form.CurrentRampStartTile);

            // Execute ramp from tile (10, 10) to tile (20, 10)
            form.RampWidthTiles = 2f;
            form.RampFalloffTiles = 1f;

            var changes = form.ExecuteRamp(10, 10, 20, 10);
            Assert.NotEmpty(changes);
            Assert.True(session.HeightsDirty);
            Assert.True(session.CanUndo);

            // Verify Undo reverts in single stroke
            Invoke(form, "Undo");
            Assert.False(session.HeightsDirty);
            Assert.True(session.CanRedo);

            // Verify Redo restores in single stroke
            Invoke(form, "Redo");
            Assert.True(session.HeightsDirty);
        });
    }

    [Fact]
    public void Ramp_tool_start_point_selection_and_cancellation()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "RampCancelTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            form.ActivateRampTool();
            Assert.Null(form.CurrentRampStartTile);

            // Simulate pointer down on tile (15, 15)
            Invoke(form, "HandleRampPointerDown", 15, 15);
            Assert.Equal((15, 15), form.CurrentRampStartTile);

            // Cancel selection
            form.CancelRampSelection();
            Assert.Null(form.CurrentRampStartTile);

            // Deactivate ramp tool
            form.DeactivateRampTool();
            var rampButton = GetField<ToolStripButton>(form, "_rampTool");
            Assert.False(rampButton.Checked);
        });
    }
}
