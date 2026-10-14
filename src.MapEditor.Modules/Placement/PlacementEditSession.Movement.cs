using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Placement;

public sealed partial class PlacementEditSession
{
    /// <summary>Translate a selection without changing identity, direction or family provenance.
    /// Validate the whole batch before adding one undo command.</summary>
    public void MoveMany(IEnumerable<int> indices, float offsetX, float offsetZ,
        Func<float, float, float>? heightAt = null)
    {
        int[] selected = ValidateIndices(indices);
        if (!float.IsFinite(offsetX) || !float.IsFinite(offsetZ))
            throw new ArgumentOutOfRangeException(nameof(offsetX), "Offsets must be finite.");
        if (offsetX == 0 && offsetZ == 0) return;
        var commands = new List<IPlacementCommand>();
        foreach (int index in selected)
        {
            SdlPlacedObject before = _objects[index];
            SdlPlacedObject after = Clone(before with
            {
                WorldX = before.WorldX + offsetX,
                WorldZ = before.WorldZ + offsetZ
            });
            ValidateBounds(after);
            if (heightAt is not null) after = after with { WorldY = heightAt(after.WorldX, after.WorldZ) };
            ValidateBounds(after);
            commands.Add(new EditPlacementCommand(index, before, after));
        }
        if (commands.Count > 0) Execute(new BatchPlacementCommand("Move selected objects", commands));
    }

    /// <summary>Align the first selected object's anchor to a 256-unit tile center;
    /// move all selected objects by the same delta to preserve their arrangement.</summary>
    public void SnapManyToGrid(IEnumerable<int> indices, Func<float, float, float>? heightAt = null)
    {
        int[] selected = ValidateIndices(indices);
        if (selected.Length == 0) return;
        SdlPlacedObject anchor = _objects[selected[0]];
        MoveMany(selected, MathF.Floor(anchor.WorldX / 256) * 256 + 128 - anchor.WorldX,
            MathF.Floor(anchor.WorldZ / 256) * 256 + 128 - anchor.WorldZ, heightAt);
    }
}
