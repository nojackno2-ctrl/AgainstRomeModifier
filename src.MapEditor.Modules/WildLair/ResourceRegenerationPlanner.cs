using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.WildLair;

/// <summary>Plans explicit replacements of observed resources. No CA growth, stump decay,
/// seed spread or automatic depletion detection is attributed to the native game.</summary>
public sealed class ResourceRegenerationPlanner
{
    // Identifier-only evidence from TEMP objdef.txt; see docs/reverse-engineering/wild-lairs.md.
    public static IReadOnlyList<NativeResourceDefinition> KnownResources { get; } = Array.AsReadOnly<NativeResourceDefinition>(
    [
        new("LanGerSte00_1Stein", NativeResourceKind.Stone, 0),
        new("LanGerSte01_1Stein", NativeResourceKind.Stone, 1),
        new("LanGerSte02_1Stein", NativeResourceKind.Stone, 2),
        new("BauGerMin00_Mine", NativeResourceKind.MineBuilding, 43, "GER_MIN00"),
        new("LanGerNad00_Tanne_gross", NativeResourceKind.Tree, 46),
        new("LanGerNad01_Tanne_gross", NativeResourceKind.Tree, 80),
        new("LanGerNad02_Tanne_gross", NativeResourceKind.Tree, 81),
        new("LanGerNad03_Tanne_gross", NativeResourceKind.Tree, 82),
        new("LanGerNad04_Tanne_gross", NativeResourceKind.Tree, 83),
        new("LanGerNad05_Tanne_gross", NativeResourceKind.Tree, 84),
        new("LanGerNad06_Tanne_klein", NativeResourceKind.Tree, 85),
        new("LanGerNad07_Tanne_klein", NativeResourceKind.Tree, 86),
        new("LanGerNad08_Tanne_klein", NativeResourceKind.Tree, 87),
        new("LanGerNad09_Tanne_klein", NativeResourceKind.Tree, 88),
        new("LanGerNad10_Tanne_klein", NativeResourceKind.Tree, 89),
        new("LanGerNad11_Tanne_klein", NativeResourceKind.Tree, 90),
        new("LanGerNad12_Tanne_gross", NativeResourceKind.Tree, 126),
        new("LanGerNad13_Tanne_gross", NativeResourceKind.Tree, 127),
        new("LanGerNad14_Tanne_gross", NativeResourceKind.Tree, 128),
        new("LanGerNad15_Tanne_gross", NativeResourceKind.Tree, 129),
        new("LanGerNad16_Tanne_gross", NativeResourceKind.Tree, 130),
        new("LanGerNad17_Tanne_gross", NativeResourceKind.Tree, 131),
        new("LanGerNad18_Tanne_klein", NativeResourceKind.Tree, 132),
        new("LanGerNad19_Tanne_klein", NativeResourceKind.Tree, 133),
        new("LanGerNad20_Tanne_klein", NativeResourceKind.Tree, 134),
        new("LanGerNad21_Tanne_klein", NativeResourceKind.Tree, 135),
        new("LanGerNad22_Tanne_klein", NativeResourceKind.Tree, 136),
        new("LanGerNad23_Tanne_klein", NativeResourceKind.Tree, 137),
        new("BauGerGol00_Goldschmiede", NativeResourceKind.GoldsmithBuilding, 234, "GER_GOL00"),
        new("LanGerNad24_Tanne_mittel", NativeResourceKind.Tree, 320),
        new("LanGerNad25_Tanne_mittel", NativeResourceKind.Tree, 321),
        new("LanGerNad26_Tanne_mittel", NativeResourceKind.Tree, 322),
        new("LanGerNad27_Tanne_mittel", NativeResourceKind.Tree, 323),
        new("LanGerNad28_Tanne_mittel", NativeResourceKind.Tree, 324),
        new("LanGerNad29_Tanne_mittel", NativeResourceKind.Tree, 325),
        new("LanGerNad30_Tanne_mittel", NativeResourceKind.Tree, 326),
        new("LanGerNad31_Tanne_mittel", NativeResourceKind.Tree, 327),
        new("LanGerNad32_Tanne_mittel", NativeResourceKind.Tree, 328),
        new("BauKelMin00_Mine", NativeResourceKind.MineBuilding, 387, "KEL_MIN00"),
        new("BauKelGol01_Goldschmiede", NativeResourceKind.GoldsmithBuilding, 391, "KEL_GOL01"),
        new("BauKelGol00_Goldschmiede", NativeResourceKind.GoldsmithBuilding, 392, "KEL_GOL00"),
        new("LanItaWei00_Weizenfeld", NativeResourceKind.Field, 529),
        new("LanItaWei01_Weizenfeld", NativeResourceKind.Field, 530),
        new("LanItaWei02_Weizenfeld", NativeResourceKind.Field, 531),
        new("LanItaWei03_Weizenfeld", NativeResourceKind.Field, 532),
        new("LanItaWei04_Weizenfeld", NativeResourceKind.Field, 533),
        new("BauRomGol00_Goldschmiede", NativeResourceKind.GoldsmithBuilding, 705, "ROM_GOL00"),
        new("BauRomGol01_Goldschmiede", NativeResourceKind.GoldsmithBuilding, 706, "ROM_GOL01"),
        new("BauRomMin00_Mine", NativeResourceKind.MineBuilding, 715, "ROM_MIN00"),
        new("LanBriLau00_Laubbaum_gross", NativeResourceKind.Tree, 717),
        new("LanBriLau01_Laubbaum_gross", NativeResourceKind.Tree, 718),
        new("LanBriLau02_Laubbaum_gross", NativeResourceKind.Tree, 719),
        new("LanBriLau03_Laubbaum_gross", NativeResourceKind.Tree, 720),
        new("LanBriLau04_Laubbaum_gross", NativeResourceKind.Tree, 721),
        new("LanBriLau05_Laubbaum_gross", NativeResourceKind.Tree, 722),
        new("LanBriLau06_Laubbaum_gross", NativeResourceKind.Tree, 723),
        new("LanBriLau07_Laubbaum_mittel", NativeResourceKind.Tree, 724),
        new("LanBriLau08_Laubbaum_mittel", NativeResourceKind.Tree, 725),
        new("BauHunGol00_Goldschmiede", NativeResourceKind.GoldsmithBuilding, 998, "HUN_GOL00"),
        new("BauHunMin00_Mine", NativeResourceKind.MineBuilding, 1010, "HUN_MIN00"),
        new("LanBriLau09_Laubbaum_mittel", NativeResourceKind.Tree, 1719),
        new("LanBriLau10_Laubbaum_klein", NativeResourceKind.Tree, 1720),
        new("LanBriLau11_Laubbaum_klein", NativeResourceKind.Tree, 1721),
        new("LanBriLau12_Laubbaum_klein", NativeResourceKind.Tree, 1722),
    ]);

    public TimedResourceReplacement PlanReplacement(string nameDef, float worldX, float worldZ,
        int delaySeconds, int team = 8)
    {
        if (!KnownResources.Any(r => string.Equals(r.NameDef, nameDef, StringComparison.OrdinalIgnoreCase)))
            throw new ArgumentException("Unknown resource definition; require observed objdef evidence.", nameof(nameDef));
        if (!float.IsFinite(worldX) || !float.IsFinite(worldZ) || worldX is < 0 or > 16383 || worldZ is < 0 or > 16383)
            throw new ArgumentOutOfRangeException(nameof(worldX), "Coordinates must be finite world units in 0-16383.");
        if (delaySeconds is < 1 or > 86400) throw new ArgumentOutOfRangeException(nameof(delaySeconds));
        if (team is < 0 or > 8) throw new ArgumentOutOfRangeException(nameof(team));
        return new TimedResourceReplacement(nameDef, worldX, worldZ, delaySeconds, team);
    }

    /// <summary>Explicit editor-time placement of a tree/stone/field; no timed game behavior.
    /// Caller supplies an existing complete native template and commits through NatureEditSession.</summary>
    public NatureAddition CreateEditorAddition(TimedResourceReplacement request,
        Func<string, LevelObjectTemplate?> templateResolver, float worldY = 0)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(templateResolver);
        PlanReplacement(request.NameDef, request.WorldX, request.WorldZ, request.DelaySeconds, request.Team);
        NativeResourceDefinition resource = KnownResources.Single(r => r.NameDef.Equals(request.NameDef, StringComparison.OrdinalIgnoreCase));
        if (resource.Kind is NativeResourceKind.MineBuilding or NativeResourceKind.GoldsmithBuilding || request.Team != 8)
            throw new NotSupportedException("Nature placement requires a neutral landscape template; buildings use the existing placement workflow.");
        if (!float.IsFinite(worldY)) throw new ArgumentOutOfRangeException(nameof(worldY));
        LevelObjectTemplate template = templateResolver(resource.NameDef)
            ?? throw new InvalidOperationException("No native template for this observed resource.");
        if (template.TypeId != resource.DefinitionId)
            throw new InvalidOperationException("The supplied template does not match the observed resource definition ID.");
        return new NatureAddition(template, resource.NameDef, request.WorldX, worldY, request.WorldZ, 0);
    }
}
