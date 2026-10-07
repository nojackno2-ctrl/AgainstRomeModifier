using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using AgainstRomeModifier.Core.Features;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Cli;

public static class CliCommands
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping
    };

    public static string ResolveGamePath(CliOptions options)
    {
        string? candidate = options.GamePath;
        if (!string.IsNullOrWhiteSpace(candidate))
        {
            if (!Directory.Exists(candidate))
                throw new DirectoryNotFoundException($"指定的遊戲目錄不存在: {candidate}");
            return Path.GetFullPath(candidate);
        }

        string detected = GameDirectoryLocator.ResolveInitialGamePath(null);
        if (string.IsNullOrWhiteSpace(detected) || !Directory.Exists(detected))
        {
            throw new InvalidOperationException("無法自動偵測到遊戲目錄。請使用 --game <路徑> 明確指定 Against Rome 安裝目錄。");
        }

        return Path.GetFullPath(detected);
    }

    public static void ValidateGameExe(string gamePath)
    {
        string exe = Path.Combine(gamePath, "Against_Rome.exe");
        if (!File.Exists(exe))
            throw new FileNotFoundException($"遊戲目錄下未找到主執行檔 Against_Rome.exe: {gamePath}");
    }

    public static Task<DetectGameResult> DetectGameAsync(CliOptions options, ILogger logger)
    {
        string? preferred = options.GamePath;
        string detected = GameDirectoryLocator.ResolveInitialGamePath(preferred);
        string registry = GameDirectoryLocator.DetectFromRegistry();
        bool isValid = !string.IsNullOrWhiteSpace(detected) && Directory.Exists(detected);
        bool hasExe = isValid && File.Exists(Path.Combine(detected, "Against_Rome.exe"));

        var result = new DetectGameResult
        {
            DetectedPath = isValid ? Path.GetFullPath(detected) : null,
            IsValid = isValid,
            HasExe = hasExe,
            RegistryPath = !string.IsNullOrEmpty(registry) ? registry : null,
            DefaultPath = GameDirectoryLocator.DefaultInstallPath
        };

        return Task.FromResult(result);
    }

    public static Task<GameStatusResult> GetStatusAsync(CliOptions options, ILogger logger)
    {
        string gamePath = ResolveGamePath(options);
        ValidateGameExe(gamePath);

        var backupManager = new BackupManager(logger);
        backupManager.LoadBackupZipToMemory(gamePath);
        var missingFiles = backupManager.FindMissingBackupResources();

        var patchEngine = new PatchEngine(logger);
        PatchProfile profile = patchEngine.DetectCurrentPatchState(gamePath, backupManager);

        var result = new GameStatusResult
        {
            GamePath = gamePath,
            Backup = new BackupSummary
            {
                Loaded = backupManager.BackupFiles.Count > 0,
                MissingCount = missingFiles.Count,
                MissingFiles = missingFiles
            },
            GameSpeed = profile.GameSpeed,
            VillageGarrisonQuota = profile.VillageGarrisonQuotaMultiplier,
            DgVoodooInstalled = patchEngine.IsDgVoodooInstalled(gamePath),
            ArgmTraceInstalled = patchEngine.IsArgmTraceInstalled(gamePath),
            HasCustomUnitStats = profile.CustomUnitStats != null && profile.CustomUnitStats.Count > 0,
            Balance = profile.Balance
        };

        int activeCount = 0;
        foreach (var def in FeatureRegistry.ToggleFeatures)
        {
            bool isActive = profile.GetBool(def.Id);
            result.Features[def.Id] = isActive;
            if (isActive) activeCount++;
        }
        result.ActiveFeaturesCount = activeCount;

        return Task.FromResult(result);
    }

    public static Task<List<FeatureItemResult>> ListFeaturesAsync(CliOptions options, ILogger logger)
    {
        PatchProfile? currentProfile = null;
        if (!string.IsNullOrWhiteSpace(options.GamePath) || Directory.Exists(GameDirectoryLocator.ResolveInitialGamePath(null)))
        {
            try
            {
                string gamePath = ResolveGamePath(options);
                if (File.Exists(Path.Combine(gamePath, "Against_Rome.exe")))
                {
                    var backupManager = new BackupManager(logger);
                    backupManager.LoadBackupZipToMemory(gamePath);
                    var patchEngine = new PatchEngine(logger);
                    currentProfile = patchEngine.DetectCurrentPatchState(gamePath, backupManager);
                }
            }
            catch
            {
                // Current profile remains null if path fails
            }
        }

        var results = new List<FeatureItemResult>();
        var excludedSet = new HashSet<string>(ModifierForm.EnableAllExcludedFeatureIds, StringComparer.OrdinalIgnoreCase);

        foreach (var def in FeatureRegistry.All)
        {
            if (!string.IsNullOrWhiteSpace(options.Category))
            {
                if (!string.Equals(def.Category.ToString(), options.Category, StringComparison.OrdinalIgnoreCase))
                {
                    continue;
                }
            }

            bool? currentVal = null;
            if (currentProfile != null)
            {
                if (def.ControlKind == FeatureControlKind.Toggle)
                {
                    currentVal = currentProfile.GetBool(def.Id);
                }
            }

            results.Add(new FeatureItemResult
            {
                Id = def.Id,
                Name = Loc.Get(def.Id),
                Description = Loc.Get(def.Id + "Tip"),
                Category = def.Category.ToString(),
                ControlKind = def.ControlKind.ToString(),
                IsExcludedFromEnableAll = excludedSet.Contains(def.Id),
                CurrentValue = currentVal
            });
        }

        return Task.FromResult(results);
    }

    public static async Task<ApplyModificationResult> ApplyAsync(CliOptions options, ILogger logger)
    {
        if (!string.IsNullOrWhiteSpace(options.Category))
        {
            if (!Enum.TryParse<FeatureCategory>(options.Category, true, out _))
                throw new ArgumentException($"未知的修改分類: '{options.Category}'。可用分類: Stats, Compat, Language。可使用 'AgainstRomeModifier.exe features' 查詢。");
        }

        if (options.EnableFeatures != null)
        {
            foreach (string id in options.EnableFeatures)
            {
                if (FindFeature(id) == null)
                    throw new ArgumentException($"未知的修改功能 ID: '{id}'。可使用 'AgainstRomeModifier.exe features' 查詢所有可用 ID。");
            }
        }

        if (options.DisableFeatures != null)
        {
            foreach (string id in options.DisableFeatures)
            {
                if (FindFeature(id) == null)
                    throw new ArgumentException($"未知的修改功能 ID: '{id}'。可使用 'AgainstRomeModifier.exe features' 查詢所有可用 ID。");
            }
        }

        if (options.GameSpeed.HasValue)
        {
            int speed = options.GameSpeed.Value;
            if (speed < 1 || speed > 10)
                throw new ArgumentException("遊戲加速倍率必須介於 1 到 10 之間。");
        }

        if (options.VillageGarrisonQuotaMultiplier.HasValue)
        {
            int quota = options.VillageGarrisonQuotaMultiplier.Value;
            if (quota < 1 || quota > 10)
                throw new ArgumentException("村莊駐軍配額倍率必須介於 1 到 10 之間。");
        }

        if (!string.IsNullOrWhiteSpace(options.ProfileFile) && !File.Exists(options.ProfileFile))
            throw new FileNotFoundException($"找不到設定檔: {options.ProfileFile}");

        if (!string.IsNullOrWhiteSpace(options.PresetFile) && !File.Exists(options.PresetFile))
            throw new FileNotFoundException($"找不到兵種預設檔: {options.PresetFile}");

        string gamePath = ResolveGamePath(options);
        ValidateGameExe(gamePath);

        var backupManager = new BackupManager(logger);
        backupManager.LoadBackupZipToMemory(gamePath);
        if (!backupManager.EnsureBackupLoadedForGamePath(gamePath))
        {
            throw new InvalidOperationException("無法載入或建立遊戲備份基準。請先確認遊戲目錄包含原版檔案。");
        }

        var patchEngine = new PatchEngine(logger);
        PatchProfile profile;

        if (!string.IsNullOrWhiteSpace(options.ProfileFile))
        {
            if (!File.Exists(options.ProfileFile))
                throw new FileNotFoundException($"找不到設定檔: {options.ProfileFile}");
            string json = File.ReadAllText(options.ProfileFile);
            var dto = JsonSerializer.Deserialize<PatchProfileDto>(json, JsonOptions)
                ?? throw new InvalidDataException("設定檔格式無效。");
            profile = dto.ToProfile();
        }
        else
        {
            if (options.DisableAll)
            {
                profile = new PatchProfile();
                profile.GameSpeed = 1;
                profile.VillageGarrisonQuotaMultiplier = 1;
                profile.Balance = false;
            }
            else
            {
                profile = patchEngine.DetectCurrentPatchState(gamePath, backupManager);
            }

            if (!string.IsNullOrWhiteSpace(options.Category) || options.Compat)
            {
                string catName = !string.IsNullOrWhiteSpace(options.Category) ? options.Category : "Compat";
                if (!Enum.TryParse<FeatureCategory>(catName, true, out var targetCategory))
                {
                    throw new ArgumentException($"未知的分類: '{catName}'。可用分類: Stats, Compat, Language。");
                }

                var excludedSet = new HashSet<string>(ModifierForm.EnableAllExcludedFeatureIds, StringComparer.OrdinalIgnoreCase);
                foreach (var def in FeatureRegistry.ByCategory(targetCategory))
                {
                    if (def.ControlKind == FeatureControlKind.Toggle)
                    {
                        if (options.EnableAll || !excludedSet.Contains(def.Id))
                        {
                            profile.Set(def.Id, FeatureValue.Of(true));
                        }
                    }
                }
            }
            else if (options.EnableAll)
            {
                var excludedSet = new HashSet<string>(ModifierForm.EnableAllExcludedFeatureIds, StringComparer.OrdinalIgnoreCase);
                foreach (var def in FeatureRegistry.ToggleFeatures)
                {
                    if (!excludedSet.Contains(def.Id))
                    {
                        profile.Set(def.Id, FeatureValue.Of(true));
                    }
                }
            }

            if (options.EnableFeatures != null)
            {
                foreach (string id in options.EnableFeatures)
                {
                    var def = FindFeature(id)
                        ?? throw new ArgumentException($"未知的修改功能 ID: '{id}'。可使用 'AgainstRomeModifier.exe features' 查詢所有可用 ID。");
                    profile.Set(def.Id, FeatureValue.Of(true));
                }
            }

            if (options.DisableFeatures != null)
            {
                foreach (string id in options.DisableFeatures)
                {
                    var def = FindFeature(id)
                        ?? throw new ArgumentException($"未知的修改功能 ID: '{id}'。可使用 'AgainstRomeModifier.exe features' 查詢所有可用 ID。");
                    profile.Set(def.Id, FeatureValue.Of(false));
                }
            }

            if (options.Balance.HasValue)
            {
                profile.Balance = options.Balance.Value;
            }

            if (options.GameSpeed.HasValue)
            {
                int speed = options.GameSpeed.Value;
                if (speed < 1 || speed > 10)
                    throw new ArgumentException("遊戲加速倍率必須介於 1 到 10 之間。");
                profile.GameSpeed = speed;
            }

            if (options.VillageGarrisonQuotaMultiplier.HasValue)
            {
                int quota = options.VillageGarrisonQuotaMultiplier.Value;
                if (quota < 1 || quota > 10)
                    throw new ArgumentException("村莊駐軍配額倍率必須介於 1 到 10 之間。");
                profile.VillageGarrisonQuotaMultiplier = quota;
            }

            if (!string.IsNullOrWhiteSpace(options.PresetFile))
            {
                if (!File.Exists(options.PresetFile))
                    throw new FileNotFoundException($"找不到兵種預設檔: {options.PresetFile}");
                var lines = File.ReadAllLines(options.PresetFile);
                var parsed = TroopPresetCodec.Parse(lines, key => backupManager.GetOriginalStats(key));
                profile.CustomUnitStats = new Dictionary<string, double[]>(parsed.Stats, StringComparer.OrdinalIgnoreCase);
            }
        }

        var enabledFeatures = new List<string>();
        foreach (var def in FeatureRegistry.ToggleFeatures)
        {
            if (profile.GetBool(def.Id)) enabledFeatures.Add(def.Id);
        }

        var result = new ApplyModificationResult
        {
            GamePath = gamePath,
            IsDryRun = options.DryRun,
            AppliedFeaturesCount = enabledFeatures.Count,
            EnabledFeatures = enabledFeatures,
            GameSpeed = profile.GameSpeed,
            VillageGarrisonQuotaMultiplier = profile.VillageGarrisonQuotaMultiplier,
            Balance = profile.Balance,
            CustomStatsUnitsCount = profile.CustomUnitStats?.Count ?? 0
        };

        if (!options.DryRun)
        {
            var runner = new PatchOperationRunner(msg => logger.Log(msg));
            await runner.ExecuteAsync(
                rollback => patchEngine.ApplyPatches(gamePath, profile, backupManager, rollback),
                Loc.Get("LogCheckpointApply"),
                Loc.Get("LogRollbackStartedApply"),
                Loc.Get("LogRollbackDoneApply"));
        }

        return result;
    }

    public static async Task<RestoreModificationResult> RestoreAsync(CliOptions options, ILogger logger)
    {
        string gamePath = ResolveGamePath(options);
        ValidateGameExe(gamePath);

        var backupManager = new BackupManager(logger);
        backupManager.LoadBackupZipToMemory(gamePath);

        var patchEngine = new PatchEngine(logger);
        var categoriesRestored = new List<string>();

        bool restoreSpecific = options.RestoreStats || options.RestoreCompat || options.RestoreLanguage;
        bool restoreAll = options.RestoreAll || !restoreSpecific;

        var runner = new PatchOperationRunner(msg => logger.Log(msg));

        if (restoreAll)
        {
            await runner.ExecuteAsync(
                rollback => patchEngine.RestoreOriginalFiles(gamePath, backupManager, rollback, options.PreserveCustomMaps),
                "建立還原回復點...",
                Loc.Get("LogRollbackStartedRestore"),
                Loc.Get("LogRollbackDoneRestore"));

            categoriesRestored.Add("Stats");
            categoriesRestored.Add("Compat");
            categoriesRestored.Add("Language");
        }
        else
        {
            await runner.ExecuteAsync(
                rollback =>
                {
                    if (options.RestoreStats)
                    {
                        patchEngine.RestoreStatsOnly(gamePath, backupManager, rollback);
                    }
                    if (options.RestoreCompat)
                    {
                        patchEngine.RestoreCompatOnly(gamePath, backupManager, rollback);
                    }
                    if (options.RestoreLanguage)
                    {
                        patchEngine.RestoreLanguageOnly(gamePath, rollback);
                    }
                },
                "建立部分還原回復點...",
                Loc.Get("LogRollbackStartedRestore"),
                Loc.Get("LogRollbackDoneRestore"));

            if (options.RestoreStats) categoriesRestored.Add("Stats");
            if (options.RestoreCompat) categoriesRestored.Add("Compat");
            if (options.RestoreLanguage) categoriesRestored.Add("Language");
        }

        return new RestoreModificationResult
        {
            GamePath = gamePath,
            CategoriesRestored = categoriesRestored,
            PreserveCustomMaps = options.PreserveCustomMaps
        };
    }

    public static Task<BackupOperationResult> BackupAsync(CliOptions options, ILogger logger)
    {
        string gamePath = ResolveGamePath(options);
        var backupManager = new BackupManager(logger);
        backupManager.LoadBackupZipToMemory(gamePath);

        string action = (options.SubAction ?? "status").ToLowerInvariant();
        bool success = true;

        switch (action)
        {
            case "ensure":
                success = backupManager.EnsureBackupLoadedForGamePath(gamePath);
                break;
            case "heal":
                backupManager.TryAutoHealBackupFiles(gamePath);
                break;
            case "status":
            default:
                break;
        }

        var missing = backupManager.FindMissingBackupResources();
        return Task.FromResult(new BackupOperationResult
        {
            Action = action,
            GamePath = gamePath,
            Success = success,
            MissingCount = missing.Count,
            MissingFiles = missing
        });
    }

    public static Task<object> StatsAsync(CliOptions options, ILogger logger)
    {
        string gamePath = ResolveGamePath(options);
        var backupManager = new BackupManager(logger);
        backupManager.LoadBackupZipToMemory(gamePath);

        var patchEngine = new PatchEngine(logger);
        PatchProfile profile = patchEngine.DetectCurrentPatchState(gamePath, backupManager);
        if (options.Balance.HasValue) profile.Balance = options.Balance.Value;

        var projectionService = new UnitStatsProjectionService(backupManager);
        string action = (options.SubAction ?? "list").ToLowerInvariant();

        if (action == "get")
        {
            string unitKey = options.SubArg ?? throw new ArgumentException("請指定欲查詢的兵種代號 (例如: stats get FigGerInf01_Schwert)。");
            var match = TroopConfig.UnitMeta.FirstOrDefault(kv => string.Equals(kv.Key, unitKey, StringComparison.OrdinalIgnoreCase));
            if (match.Key == null)
            {
                match = TroopConfig.UnitMeta.FirstOrDefault(kv => kv.Key.Contains(unitKey, StringComparison.OrdinalIgnoreCase));
            }
            if (match.Key == null)
                throw new ArgumentException($"未知的兵種代號: '{unitKey}'。可使用 'stats list' 查看所有可用代號。");

            double[] stats = projectionService.Project(match.Key, profile);
            return Task.FromResult<object>(MapUnitStatItem(match.Key, match.Value, stats));
        }

        var allItems = new List<UnitStatItemResult>();
        var statsDictionary = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);

        foreach (string key in TroopConfig.UnitOrder)
        {
            if (!TroopConfig.UnitMeta.TryGetValue(key, out var meta)) continue;
            double[] stats = projectionService.Project(key, profile);
            allItems.Add(MapUnitStatItem(key, meta, stats));
            statsDictionary[key] = stats;
        }

        if (action == "export")
        {
            if (string.IsNullOrWhiteSpace(options.OutputFile))
                throw new ArgumentException("匯出兵種數值必須指定 --output <檔案路徑>。");

            string format = (options.Format ?? "ini").ToLowerInvariant();
            if (format == "json")
            {
                string json = JsonSerializer.Serialize(allItems, JsonOptions);
                File.WriteAllText(options.OutputFile, json, Encoding.UTF8);
            }
            else
            {
                string ini = TroopPresetCodec.Write(statsDictionary, DateTime.UtcNow);
                File.WriteAllText(options.OutputFile, ini, Encoding.UTF8);
            }

            return Task.FromResult<object>(new
            {
                ExportedUnitsCount = allItems.Count,
                Format = format,
                OutputFile = Path.GetFullPath(options.OutputFile)
            });
        }

        return Task.FromResult<object>(allItems);
    }

    public static Task<object> ProfileAsync(CliOptions options, ILogger logger)
    {
        string action = (options.SubAction ?? "schema").ToLowerInvariant();

        if (action == "schema")
        {
            var featuresList = FeatureRegistry.All.Select(f => new
            {
                f.Id,
                Category = f.Category.ToString(),
                ControlKind = f.ControlKind.ToString(),
                Name = Loc.Get(f.Id),
                Description = Loc.Get(f.Id + "Tip")
            }).ToList();

            return Task.FromResult<object>(new
            {
                Type = "AgainstRome.PatchProfile",
                Properties = new
                {
                    Features = "Dictionary<string, bool> - 開關類修改項目",
                    GameSpeed = "int (1..10) - 遊戲加速倍率",
                    VillageGarrisonQuotaMultiplier = "int (1..10) - 村莊駐軍配額倍率",
                    Balance = "bool - 兵種平衡模式",
                    CustomUnitStats = "Dictionary<string, double[]> - 自訂兵種屬性 [HP, Dmg, VW, AW, Speed, Sight, Relt, Range, SpellRadius]"
                },
                AvailableFeatures = featuresList
            });
        }

        if (action == "export")
        {
            if (string.IsNullOrWhiteSpace(options.OutputFile))
                throw new ArgumentException("匯出設定檔必須指定 --output <檔案路徑>。");

            string gamePath = ResolveGamePath(options);
            var backupManager = new BackupManager(logger);
            backupManager.LoadBackupZipToMemory(gamePath);
            var patchEngine = new PatchEngine(logger);
            PatchProfile profile = patchEngine.DetectCurrentPatchState(gamePath, backupManager);

            var dto = PatchProfileDto.FromProfile(profile);
            string json = JsonSerializer.Serialize(dto, JsonOptions);
            File.WriteAllText(options.OutputFile, json, Encoding.UTF8);

            return Task.FromResult<object>(new
            {
                Exported = true,
                OutputFile = Path.GetFullPath(options.OutputFile),
                EnabledFeaturesCount = dto.Features.Values.Count(v => v)
            });
        }

        throw new ArgumentException($"未知的 profile 子指令: '{action}'。支援: schema, export。");
    }

    public static Task<SavesCatalogResult> SavesAsync(CliOptions options, ILogger logger)
    {
        string gamePath = ResolveGamePath(options);
        string backupDir = Path.Combine(gamePath, "SAVE_BACKUP");
        var service = new SaveBackupService(backupDir);
        string action = (options.SubAction ?? "list").ToLowerInvariant();

        if (action == "backup")
        {
            string saveFolder = options.SubArg
                ?? throw new ArgumentException("請指定欲備份的存檔目錄名稱 (例如: saves backup ESAVE_000)。");
            var scan = service.Scan(gamePath);
            var saveInfo = scan.Saves.FirstOrDefault(s => string.Equals(s.Folder, saveFolder, StringComparison.OrdinalIgnoreCase));
            string title = saveInfo?.Title ?? saveFolder;
            string level = saveInfo?.Level ?? "";
            service.CreateBackup(gamePath, saveFolder, title, level);
        }
        else if (action == "repair-ai")
        {
            string saveFolder = options.SubArg
                ?? throw new ArgumentException("請指定欲修復 AI 的存檔目錄名稱 (例如: saves repair-ai ESAVE_000)。");
            var repairService = new EndlessSaveAiRepairService();
            repairService.Repair(gamePath, saveFolder);
        }

        var catalog = service.Scan(gamePath);
        var result = new SavesCatalogResult
        {
            Saves = catalog.Saves.Select(s => new GameSaveItemResult
            {
                Folder = s.Folder,
                Title = s.Title,
                Level = s.Level,
                LastWriteTime = s.LastWriteTime,
                Parsed = s.Parsed
            }).ToList(),
            Backups = catalog.Backups.Select(b => new SaveBackupItemResult
            {
                FileName = b.FileName,
                Title = b.Title,
                Level = b.Level,
                BackupTime = b.BackupTime,
                OrigFolder = b.OrigFolder,
                LastWriteTime = b.LastWriteTime,
                Parsed = b.Parsed
            }).ToList()
        };

        return Task.FromResult(result);
    }

    public static Task<List<GameMapItemResult>> MapsAsync(CliOptions options, ILogger logger)
    {
        string gamePath = ResolveGamePath(options);
        var catalog = new GameMapCatalog();
        var maps = catalog.List(gamePath);

        var list = maps.Select(m => new GameMapItemResult
        {
            Id = m.Id,
            DisplayName = m.DisplayName,
            Category = m.Category,
            IsCustom = m.IsCustom,
            EndlessSlot = m.EndlessSlot
        }).ToList();

        return Task.FromResult(list);
    }

    public static CliHelpResult GetHelp()
    {
        return new CliHelpResult
        {
            AppName = "Against Rome Modifier CLI",
            Version = "1.2.2",
            Description = "遊戲修改器 CLI 模式，供 AI 代理人與自動化腳本操作",
            Commands = new List<CommandHelpInfo>
            {
                new()
                {
                    Name = "detect-game",
                    Description = "自動偵測或驗證遊戲目錄路徑",
                    Options = new() { "--game <path>" }
                },
                new()
                {
                    Name = "status",
                    Description = "檢視遊戲與修改器目前狀態、已套用修改、備份狀態等",
                    Options = new() { "--game <path>", "--json" }
                },
                new()
                {
                    Name = "features",
                    Description = "列出所有可用的修改功能項目與說明",
                    Options = new() { "--category <Stats|Compat|Language>", "--game <path>", "--json" }
                },
                new()
                {
                    Name = "apply",
                    Description = "套用修改項目到遊戲檔案",
                    Options = new()
                    {
                        "--game <path>",
                        "--category <Stats|Compat|Language>",
                        "--compat",
                        "--enable <ids>",
                        "--disable <ids>",
                        "--all",
                        "--disable-all",
                        "--balance",
                        "--game-speed <1..10>",
                        "--garrison-quota <1..10>",
                        "--preset <filePath>",
                        "--profile <filePath>",
                        "--dry-run",
                        "--json"
                    }
                },
                new()
                {
                    Name = "restore",
                    Description = "將遊戲檔案還原為原版或備份狀態",
                    Options = new()
                    {
                        "--game <path>",
                        "--all",
                        "--stats",
                        "--compat",
                        "--language",
                        "--preserve-custom-maps <true|false>",
                        "--json"
                    }
                },
                new()
                {
                    Name = "backup",
                    Description = "管理遊戲原版檔案備份（status / ensure / heal）",
                    Subcommands = new() { "status", "ensure", "heal" },
                    Options = new() { "--game <path>", "--json" }
                },
                new()
                {
                    Name = "stats",
                    Description = "檢視與匯出兵種數值（list / get / export）",
                    Subcommands = new() { "list", "get <unitKey>", "export --format <ini|json> --output <path>" },
                    Options = new() { "--game <path>", "--balance", "--json" }
                },
                new()
                {
                    Name = "profile",
                    Description = "匯出或檢視修改配置設定檔（export / schema）",
                    Subcommands = new() { "schema", "export --output <path>" },
                    Options = new() { "--game <path>", "--json" }
                },
                new()
                {
                    Name = "saves",
                    Description = "管理遊戲存檔（list / backup / repair-ai）",
                    Subcommands = new() { "list", "backup <folder>", "repair-ai <folder>" },
                    Options = new() { "--game <path>", "--json" }
                },
                new()
                {
                    Name = "maps",
                    Description = "列出遊戲地圖（list）",
                    Subcommands = new() { "list" },
                    Options = new() { "--game <path>", "--json" }
                }
            },
            Examples = new List<string>
            {
                "AgainstRomeModifier.exe status --json",
                "AgainstRomeModifier.exe apply --enable FastCiviProduction,InfiniteMorale --game-speed 3 --json",
                "AgainstRomeModifier.exe apply --all --json",
                "AgainstRomeModifier.exe restore --all --json",
                "AgainstRomeModifier.exe features --category Stats --json",
                "AgainstRomeModifier.exe stats get GER_INF01 --json",
                "AgainstRomeModifier.exe saves list --json"
            }
        };
    }

    private static FeatureDefinition? FindFeature(string id) =>
        FeatureRegistry.All.FirstOrDefault(f => string.Equals(f.Id, id, StringComparison.OrdinalIgnoreCase));

    private static UnitStatItemResult MapUnitStatItem(string key, UnitMetadata meta, double[] stats) => new()
    {
        Key = key,
        Name = Loc.GetUnitName(key),
        Faction = meta.Faction,
        Tier = meta.Tier,
        UnitType = meta.UnitType,
        Style = meta.Style,
        Hp = stats.Length > 0 ? stats[0] : 0,
        Damage = stats.Length > 1 ? stats[1] : 0,
        DefenseVw = stats.Length > 2 ? stats[2] : 0,
        CombatAw = stats.Length > 3 ? stats[3] : 0,
        Speed = stats.Length > 4 ? stats[4] : 0,
        Sight = stats.Length > 5 ? stats[5] : 0,
        ReloadRelt = stats.Length > 6 ? stats[6] : 0,
        Range = stats.Length > 7 ? stats[7] : 0,
        SpellRadius = stats.Length > 8 ? stats[8] : 0
    };
}
