using System;
using System.Collections.Generic;
using System.Globalization;
using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Core.Services;

internal sealed class UnitBaselineCatalog
{
    private const string ObjdefKey = "SYSTEM/DATA_MP/DEFAULTS/objdef.dau";
    private readonly IReadOnlyDictionary<string, byte[]> _backupFiles;
    private readonly object _rowsLock = new();
    private Dictionary<string, string[]>? _unitRows;

    internal UnitBaselineCatalog(IReadOnlyDictionary<string, byte[]> backupFiles) =>
        _backupFiles = backupFiles;

    internal void Reset()
    {
        lock (_rowsLock)
            _unitRows = null;
    }

    internal Dictionary<string, string[]> GetUnitRows()
    {
        lock (_rowsLock)
        {
            if (_unitRows != null) return _unitRows;

            _unitRows = new Dictionary<string, string[]>(StringComparer.OrdinalIgnoreCase);
            if (!_backupFiles.TryGetValue(ObjdefKey, out byte[]? dauBytes)) return _unitRows;

            byte[] decompressed = GameLZSS.DecompressPfil(dauBytes);
            string text = PatchText.GameEncoding.GetString(decompressed);
            string lineEnding = text.Contains("\r\n", StringComparison.Ordinal) ? "\r\n" : "\n";
            string[] lines = text.Split(new[] { lineEnding }, StringSplitOptions.None);
            for (int index = 2; index < lines.Length; index++)
            {
                string line = lines[index];
                if (line.Length < 100) continue;
                string[] columns = PatchText.ParseCsvLine(line);
                if (columns.Length < 192) continue;

                string name = columns[(int)ObjdefIndex.Name].Trim();
                if (TroopConfig.UnitMeta.ContainsKey(name) || name == "FigZivMan00_Zivilist")
                    _unitRows[name] = columns;
            }

            return _unitRows;
        }
    }

    internal double[] GetOriginalStats(string key)
    {
        Dictionary<string, string[]> rows = GetUnitRows();
        if (!rows.TryGetValue(key, out string[]? columns))
            return new double[9];

        double.TryParse(columns[(int)ObjdefIndex.Hp].Trim(), NumberStyles.Any,
            CultureInfo.InvariantCulture, out double hp);
        double.TryParse(columns[(int)ObjdefIndex.Vw].Trim(), NumberStyles.Any,
            CultureInfo.InvariantCulture, out double vw);
        double.TryParse(columns[(int)ObjdefIndex.Aw].Trim(), NumberStyles.Any,
            CultureInfo.InvariantCulture, out double aw);

        string unitType = TroopConfig.UnitMeta.TryGetValue(key, out var metadata)
            ? metadata.UnitType
            : "melee_inf";
        UnitStatParser.GetMeleeAndRangedDamage(columns, unitType, out double meleeDamage, out double rangedDamage);
        UnitStatParser.GetMeleeAndRangedReload(columns, unitType, out double meleeReload, out double rangedReload);

        double damage = meleeDamage;
        double reload = meleeReload;
        if (unitType is "ranged_inf" or "ranged_cav")
        {
            damage = rangedDamage;
            reload = rangedReload;
        }
        else if (unitType == "siege")
        {
            damage = Math.Max(meleeDamage, rangedDamage);
            reload = Math.Max(meleeReload, rangedReload);
        }

        double.TryParse(columns[(int)ObjdefIndex.Moves].Trim(), NumberStyles.Any,
            CultureInfo.InvariantCulture, out double originalMoves);
        double speed = originalMoves > 0 ? Math.Round(originalMoves * 2.0, 1) : 0;
        double.TryParse(columns[(int)ObjdefIndex.Sirad].Trim(), NumberStyles.Any,
            CultureInfo.InvariantCulture, out double sight);
        double range = UnitStatParser.GetMaximumRange(columns, unitType);
        double spellRadius = UnitStatParser.SupportsConfigurableSpellRadius(key) ? 500 : 0;

        return new[] { hp, damage, vw, aw, speed, sight, reload, range, spellRadius };
    }

    internal double[] GetDefaultBalancedStats(string key) =>
        TroopConfig.BalancedUnitStats.TryGetValue(key, out double[]? stats)
            ? (double[])stats.Clone()
            : GetOriginalStats(key);

    internal double[] GetBaseStatsForUnit(string key, PatchProfile profile)
    {
        double[] original = GetOriginalStats(key);
        double[] baseline = profile.Balance ? GetDefaultBalancedStats(key) : original;
        if (!profile.Balance || profile.CustomUnitStats == null ||
            !profile.CustomUnitStats.TryGetValue(key, out double[]? custom) || custom == null)
            return baseline;

        bool ignoreRange = TroopConfig.UnitMeta.TryGetValue(key, out var metadata) &&
            ((profile.RangedRange3x && TroopConfig.SupportsRangedRange3x(metadata.UnitType)) ||
             (profile.SpellEntireMap && metadata.UnitType == "priest"));
        bool supportsSpellRadius = UnitStatParser.SupportsConfigurableSpellRadius(key);
        return UnitStatParser.MergeLayers(
            baseline,
            custom,
            supportsSpellRadius,
            profile.UnitMovementSpeed2x,
            ignoreRange,
            profile.SpellRange3x && supportsSpellRadius,
            metadata?.UnitType == "priest");
    }
}
