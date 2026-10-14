using System.Buffers.Binary;
using System.Collections.ObjectModel;
using System.Globalization;

namespace AgainstRomeModifier.Maps;

public sealed record NativeMarkerTypeSelection(int Team, int Tribe, int BannerVersion,
    string? DefinitionName, int CandidateType, int SavedType, int EffectiveType);

/// <summary>
/// The team marker types after the verified native banner-selection setters.
/// Reads loose files only; does not create attachments or emulate runtime VFS.
/// </summary>
public sealed class NativeMarkerTypeTable
{
    public const int SerializedOffset = 54;
    public const int TeamCount = 9;
    private readonly ReadOnlyCollection<NativeMarkerTypeSelection> _selections;

    private NativeMarkerTypeTable(NativeMarkerTypeSelection[] selections)
        => _selections = Array.AsReadOnly(selections);

    public IReadOnlyList<NativeMarkerTypeSelection> Selections => _selections;

    public int GetEffectiveType(int team)
        => team is >= 0 and < TeamCount ? _selections[team].EffectiveType : -1;

    public static NativeMarkerTypeTable Load(string gamePath, string dataDirectory,
        IReadOnlyDictionary<int, NativeObjectDefinition>? definitions = null)
    {
        definitions ??= NativeObjectDefinitions.Load(gamePath);
        return Parse(ReadText(Path.Combine(gamePath, "SYSTEM", "banner.ini")),
            ReadText(Path.Combine(dataDirectory, "team.dat")),
            GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(dataDirectory, "engine.dat"))), definitions);
    }

    internal static NativeMarkerTypeTable Parse(string bannerText, string teamText,
        ReadOnlySpan<byte> engine, IReadOnlyDictionary<int, NativeObjectDefinition> definitions)
    {
        if (engine.Length != 94 || BinaryPrimitives.ReadInt32LittleEndian(engine) != 1)
            throw new InvalidDataException("engine.dat marker type table 的版本或長度無效。");
        var banners = Sections(bannerText);
        var teams = new (int Tribe, int Version)[8]; // 468a10 initializes both fields to zero.
        foreach (string row in Sections(teamText).GetValueOrDefault("[teamdata]") ?? [])
        {
            string[] columns = row.Split(',', StringSplitOptions.RemoveEmptyEntries);
            if (!int.TryParse(columns[0].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int team)
                || team is < 0 or >= 8)
                throw new InvalidDataException("team.dat 隊伍編號必須介於 0 和 7。");
            var previous = teams[team];
            if (columns.Length > 1)
                previous.Tribe = columns[1].Trim().ToUpperInvariant() switch
                { "KEL" => 1, "HUN" => 2, "ROM" => 3, _ => 0 };
            if (columns.Length > 5)
            {
                if (!int.TryParse(columns[5].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int version)
                    || version is < 0 or >= 10)
                    throw new InvalidDataException("team.dat 旗幟版本必須介於 0 和 9。");
                previous.Version = version;
            }
            teams[team] = previous;
        }
        var names = definitions.Values.Where(d => !string.IsNullOrEmpty(d.Name))
            .GroupBy(d => d.Name!, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(g => g.Key, g => g.ToArray(), StringComparer.OrdinalIgnoreCase);
        var selections = new NativeMarkerTypeSelection[TeamCount];
        for (int team = 0; team < TeamCount; team++)
        {
            int saved = BinaryPrimitives.ReadInt32LittleEndian(engine.Slice(SerializedOffset + team * 4, 4));
            int candidate = -1, tribe = -1, version = -1;
            string? alias = null;
            if (team < 8)
            {
                (tribe, version) = teams[team];
                string section = FormattableString.Invariant($"[volk{tribe:00}_obdef_bver{version:00}]");
                alias = banners.GetValueOrDefault(section)?.FirstOrDefault();
                if (alias is not null && !alias.Equals("none", StringComparison.OrdinalIgnoreCase)
                    && names.TryGetValue(alias, out var matches))
                {
                    if (matches.Length != 1)
                        throw new InvalidDataException($"旗幟定義名稱 {alias} 對應多個物件，無法確定類型。");
                    candidate = matches[0].TypeId;
                }
            }
            // 4c9d40 validates an active type before writing. In particular, -1
            // (including the team8 request) leaves the loaded engine value intact.
            int effective = candidate is >= 0 and < 2500
                && definitions.TryGetValue(candidate, out var definition) && definition.Active ? candidate : saved;
            selections[team] = new(team, tribe, version, alias, candidate, saved, effective);
        }
        return new(selections);
    }

    private static string ReadText(string path)
        => MapTextEncoding.Game.GetString(GameLZSS.DecompressPfil(File.ReadAllBytes(path)));

    private static Dictionary<string, List<string>> Sections(string text)
    {
        var result = new Dictionary<string, List<string>>(StringComparer.OrdinalIgnoreCase);
        List<string>? section = null;
        foreach (string line in text.Split('\n'))
        {
            string row = line.Split(';')[0].Trim();
            if (row.StartsWith('['))
            {
                if (!result.TryAdd(row, section = []))
                    throw new InvalidDataException($"重複設定區段 {row}，無法驗證旗幟來源。");
            }
            else if (row.Length != 0) section?.Add(row);
        }
        return result;
    }
}
