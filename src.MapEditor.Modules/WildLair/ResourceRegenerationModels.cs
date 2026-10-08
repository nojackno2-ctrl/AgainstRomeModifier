namespace AgainstRomeMapEditor.Modules.WildLair;

/// <summary>Names identify real objects; category does not prove harvestability or regeneration.</summary>
public enum NativeResourceKind { Tree, Stone, Field, MineBuilding, GoldsmithBuilding }
public sealed record NativeResourceDefinition(string NameDef, NativeResourceKind Kind, int DefinitionId, string? ScriptAlias = null);

/// <summary>An explicit author request, not an inferred native growth or depletion rule.</summary>
public sealed record TimedResourceReplacement(string NameDef, float WorldX, float WorldZ,
    int DelaySeconds, int Team = 8);
