using System;
using System.Collections.Generic;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Cli;

public enum CliCommandType
{
    Help,
    DetectGame,
    Status,
    Features,
    Apply,
    Restore,
    Backup,
    Stats,
    Profile,
    Saves,
    Maps
}

public sealed class CliResponse<T>
{
    public bool Success { get; set; }
    public string Command { get; set; } = "";
    public string? Message { get; set; }
    public string? Error { get; set; }
    public T? Data { get; set; }
    public List<string>? Logs { get; set; }
}

public sealed class DetectGameResult
{
    public string? DetectedPath { get; set; }
    public bool IsValid { get; set; }
    public bool HasExe { get; set; }
    public string? RegistryPath { get; set; }
    public string DefaultPath { get; set; } = "";
}

public sealed class BackupSummary
{
    public bool Loaded { get; set; }
    public int MissingCount { get; set; }
    public List<string> MissingFiles { get; set; } = new();
}

public sealed class GameStatusResult
{
    public string GamePath { get; set; } = "";
    public BackupSummary Backup { get; set; } = new();
    public int GameSpeed { get; set; } = 1;
    public int VillageGarrisonQuota { get; set; } = 1;
    public bool DgVoodooInstalled { get; set; }
    public bool ArgmTraceInstalled { get; set; }
    public bool HasCustomUnitStats { get; set; }
    public bool Balance { get; set; }
    public int ActiveFeaturesCount { get; set; }
    public Dictionary<string, bool> Features { get; set; } = new(StringComparer.OrdinalIgnoreCase);
}

public sealed class FeatureItemResult
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Description { get; set; } = "";
    public string Category { get; set; } = "";
    public string ControlKind { get; set; } = "";
    public bool IsExcludedFromEnableAll { get; set; }
    public bool? CurrentValue { get; set; }
}

public sealed class ApplyModificationResult
{
    public string GamePath { get; set; } = "";
    public bool IsDryRun { get; set; }
    public int AppliedFeaturesCount { get; set; }
    public List<string> EnabledFeatures { get; set; } = new();
    public int GameSpeed { get; set; } = 1;
    public int VillageGarrisonQuotaMultiplier { get; set; } = 1;
    public bool Balance { get; set; }
    public int CustomStatsUnitsCount { get; set; }
}

public sealed class RestoreModificationResult
{
    public string GamePath { get; set; } = "";
    public List<string> CategoriesRestored { get; set; } = new();
    public bool PreserveCustomMaps { get; set; } = true;
}

public sealed class BackupOperationResult
{
    public string Action { get; set; } = "";
    public string GamePath { get; set; } = "";
    public bool Success { get; set; }
    public int MissingCount { get; set; }
    public List<string> MissingFiles { get; set; } = new();
}

public sealed class UnitStatItemResult
{
    public string Key { get; set; } = "";
    public string Name { get; set; } = "";
    public string Faction { get; set; } = "";
    public string Tier { get; set; } = "";
    public string UnitType { get; set; } = "";
    public string Style { get; set; } = "";
    public double Hp { get; set; }
    public double Damage { get; set; }
    public double DefenseVw { get; set; }
    public double CombatAw { get; set; }
    public double Speed { get; set; }
    public double Sight { get; set; }
    public double ReloadRelt { get; set; }
    public double Range { get; set; }
    public double SpellRadius { get; set; }
}

public sealed class PatchProfileDto
{
    public Dictionary<string, bool> Features { get; set; } = new(StringComparer.OrdinalIgnoreCase);
    public int GameSpeed { get; set; } = 1;
    public int VillageGarrisonQuotaMultiplier { get; set; } = 1;
    public bool Balance { get; set; }
    public Dictionary<string, double[]>? CustomUnitStats { get; set; }

    public static PatchProfileDto FromProfile(PatchProfile profile)
    {
        var dto = new PatchProfileDto
        {
            GameSpeed = profile.GameSpeed,
            VillageGarrisonQuotaMultiplier = profile.VillageGarrisonQuotaMultiplier,
            Balance = profile.Balance,
            CustomUnitStats = profile.CustomUnitStats != null
                ? new Dictionary<string, double[]>(profile.CustomUnitStats, StringComparer.OrdinalIgnoreCase)
                : null
        };
        foreach (var def in FeatureRegistry.ToggleFeatures)
        {
            dto.Features[def.Id] = profile.GetBool(def.Id);
        }
        return dto;
    }

    public PatchProfile ToProfile()
    {
        var profile = new PatchProfile
        {
            GameSpeed = GameSpeed,
            VillageGarrisonQuotaMultiplier = VillageGarrisonQuotaMultiplier,
            Balance = Balance,
            CustomUnitStats = CustomUnitStats != null
                ? new Dictionary<string, double[]>(CustomUnitStats, StringComparer.OrdinalIgnoreCase)
                : null
        };
        foreach (var (id, enabled) in Features)
        {
            profile.Set(id, FeatureValue.Of(enabled));
        }
        return profile;
    }
}

public sealed class GameSaveItemResult
{
    public string Folder { get; set; } = "";
    public string Title { get; set; } = "";
    public string Level { get; set; } = "";
    public DateTime LastWriteTime { get; set; }
    public bool Parsed { get; set; }
}

public sealed class SaveBackupItemResult
{
    public string FileName { get; set; } = "";
    public string Title { get; set; } = "";
    public string Level { get; set; } = "";
    public string BackupTime { get; set; } = "";
    public string OrigFolder { get; set; } = "";
    public DateTime LastWriteTime { get; set; }
    public bool Parsed { get; set; }
}

public sealed class SavesCatalogResult
{
    public List<GameSaveItemResult> Saves { get; set; } = new();
    public List<SaveBackupItemResult> Backups { get; set; } = new();
}

public sealed class GameMapItemResult
{
    public string Id { get; set; } = "";
    public string? DisplayName { get; set; }
    public string Category { get; set; } = "";
    public bool IsCustom { get; set; }
    public int? EndlessSlot { get; set; }
}

public sealed class CommandHelpInfo
{
    public string Name { get; set; } = "";
    public string Description { get; set; } = "";
    public List<string> Options { get; set; } = new();
    public List<string>? Subcommands { get; set; }
}

public sealed class CliHelpResult
{
    public string AppName { get; set; } = "Against Rome Modifier CLI";
    public string Version { get; set; } = "1.2.2";
    public string Description { get; set; } = "遊戲修改器 CLI 模式，供 AI 代理人與自動化腳本操作";
    public List<CommandHelpInfo> Commands { get; set; } = new();
    public List<string> Examples { get; set; } = new();
}
