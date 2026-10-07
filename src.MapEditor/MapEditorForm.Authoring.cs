using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    internal void EditSelectedPlacedObjectsBatch()
    {
        if (_selected?.IsCustom != true) return;
        int[] indices = _placedList.SelectedItems.Cast<ListViewItem>().Select(row => (int)row.Tag!).Order().ToArray();
        if (indices.Length == 0) return;
        using var dialog = new PlacementBatchEditDialog(indices.Length, indices.Any(index => _placementSession[index].Type.Category == SdlObjectCategory.Figure));
        if (BatchEditDialogRunner(dialog) != DialogResult.OK) return;
        try { _placementSession.EditMany(indices, dialog.Team, dialog.Angle); }
        catch (ArgumentException ex)
        {
            _status.Text = (AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "Cannot change selected objects: " : "無法修改選取物件：") + ex.Message;
            return;
        }
        RefreshPlacedList(); RefreshSceneMarkers(); UpdateEditorState();
        foreach (int index in indices) _placedList.Items[index].Selected = true;
    }
}
