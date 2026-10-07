using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier.Cli;

public static class CliRunner
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };

    public static bool IsCliInvocation(string[] args)
    {
        if (args == null || args.Length == 0) return false;

        for (int i = 0; i < args.Length; i++)
        {
            string arg = args[i];
            if (arg.Equals("--cli", StringComparison.OrdinalIgnoreCase)) return true;
            if (arg.Equals("--help", StringComparison.OrdinalIgnoreCase) || arg.Equals("-h", StringComparison.OrdinalIgnoreCase) || arg.Equals("/?", StringComparison.OrdinalIgnoreCase)) return true;
            if (arg.Equals("--version", StringComparison.OrdinalIgnoreCase) || arg.Equals("-v", StringComparison.OrdinalIgnoreCase)) return true;
            if (arg.Equals("--json", StringComparison.OrdinalIgnoreCase)) return true;

            if (arg.StartsWith('-'))
            {
                if (arg.Equals("--game", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("-g", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--map", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--preset", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--profile", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--output", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("-o", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--format", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("-f", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--category", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("-c", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--lang", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--unit", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("-u", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--enable", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--disable", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--game-speed", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--speed", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--garrison-quota", StringComparison.OrdinalIgnoreCase) ||
                    arg.Equals("--quota", StringComparison.OrdinalIgnoreCase))
                {
                    i++; // skip option value
                    continue;
                }
                continue;
            }

            if (IsKnownCommand(arg)) return true;
        }

        return false;
    }

    public static bool IsKnownCommand(string cmd) => cmd.ToLowerInvariant() switch
    {
        "detect-game" or "game" => true,
        "status" or "detect" or "info" => true,
        "features" or "list" => true,
        "apply" => true,
        "restore" => true,
        "backup" => true,
        "stats" => true,
        "profile" => true,
        "saves" => true,
        "maps" => true,
        "help" => true,
        _ => false
    };

    public static int Run(string[] args, TextWriter? stdout = null, TextWriter? stderr = null)
    {
        return RunAsync(args, stdout, stderr).GetAwaiter().GetResult();
    }

    public static async Task<int> RunAsync(string[] args, TextWriter? stdout = null, TextWriter? stderr = null)
    {
        if (stdout == null)
        {
            ConsoleHelper.EnsureConsoleAttached();
            stdout = Console.Out;
            stderr = Console.Error;
        }
        stderr ??= stdout;

        CliOptions options;
        try
        {
            options = CliOptions.Parse(args);
        }
        catch (ArgumentException ex)
        {
            bool isJson = args.Any(a => a.Equals("--json", StringComparison.OrdinalIgnoreCase));
            if (isJson)
            {
                var errResponse = new CliResponse<object>
                {
                    Success = false,
                    Command = "unknown",
                    Error = ex.Message
                };
                stdout.WriteLine(JsonSerializer.Serialize(errResponse, JsonOptions));
            }
            else
            {
                stderr.WriteLine($"參數錯誤: {ex.Message}");
                stderr.WriteLine("執行 'AgainstRomeModifier.exe help' 可檢視說明。");
            }
            return 2;
        }

        if (!string.IsNullOrWhiteSpace(options.Language))
        {
            if (options.Language.Equals("en", StringComparison.OrdinalIgnoreCase))
                Loc.CurrentLanguage = Language.English;
            else
                Loc.CurrentLanguage = Language.TraditionalChinese;
        }

        var logger = new CliCollectorLogger(options.Verbose, options.Json, stderr);

        try
        {
            switch (options.Command)
            {
                case CliCommandType.DetectGame:
                {
                    var result = await CliCommands.DetectGameAsync(options, logger);
                    OutputResult(stdout, options, "detect-game", result, r =>
                    {
                        stdout.WriteLine($"遊戲目錄: {r.DetectedPath ?? "(未找到)"}");
                        stdout.WriteLine($"目錄有效: {(r.IsValid ? "是" : "否")}");
                        stdout.WriteLine($"包含主程式 Against_Rome.exe: {(r.HasExe ? "是" : "否")}");
                        if (r.RegistryPath != null) stdout.WriteLine($"登錄檔偵測路徑: {r.RegistryPath}");
                    }, logger);
                    return 0;
                }

                case CliCommandType.Status:
                {
                    var result = await CliCommands.GetStatusAsync(options, logger);
                    OutputResult(stdout, options, "status", result, r =>
                    {
                        stdout.WriteLine($"遊戲目錄: {r.GamePath}");
                        stdout.WriteLine($"備份狀態: {(r.Backup.Loaded ? "已載入" : "未載入")} (缺少 {r.Backup.MissingCount} 個檔案)");
                        stdout.WriteLine($"遊戲加速倍率: {r.GameSpeed}x");
                        stdout.WriteLine($"村莊駐軍配額: {r.VillageGarrisonQuota}x");
                        stdout.WriteLine($"平衡模式: {(r.Balance ? "開" : "關")}");
                        stdout.WriteLine($"dgVoodoo: {(r.DgVoodooInstalled ? "已安裝" : "未安裝")}");
                        stdout.WriteLine($"ArgmTrace: {(r.ArgmTraceInstalled ? "已安裝" : "未安裝")}");
                        stdout.WriteLine($"自訂兵種屬性: {(r.HasCustomUnitStats ? "已設定" : "無")}");
                        stdout.WriteLine($"已啟用修改項目 ({r.ActiveFeaturesCount} 項):");
                        foreach (var (k, v) in r.Features.Where(kv => kv.Value).OrderBy(kv => kv.Key))
                        {
                            stdout.WriteLine($"  - {k} ({Loc.Get(k)})");
                        }
                    }, logger);
                    return 0;
                }

                case CliCommandType.Features:
                {
                    var result = await CliCommands.ListFeaturesAsync(options, logger);
                    OutputResult(stdout, options, "features", result, list =>
                    {
                        stdout.WriteLine($"修改功能清單 (共 {list.Count} 項):");
                        string currentCat = "";
                        foreach (var item in list.OrderBy(f => f.Category).ThenBy(f => f.Id))
                        {
                            if (item.Category != currentCat)
                            {
                                currentCat = item.Category;
                                stdout.WriteLine($"\n[{currentCat}]");
                            }
                            string state = item.CurrentValue.HasValue ? (item.CurrentValue.Value ? " [已開啟]" : " [已關閉]") : "";
                            stdout.WriteLine($"  - {item.Id}: {item.Name}{state}");
                            if (!string.IsNullOrWhiteSpace(item.Description))
                            {
                                stdout.WriteLine($"      說明: {item.Description}");
                            }
                        }
                    }, logger);
                    return 0;
                }

                case CliCommandType.Apply:
                {
                    var result = await CliCommands.ApplyAsync(options, logger);
                    OutputResult(stdout, options, "apply", result, r =>
                    {
                        string prefix = r.IsDryRun ? "[DRY-RUN 預覽] " : "";
                        stdout.WriteLine($"{prefix}成功套用修改至: {r.GamePath}");
                        stdout.WriteLine($"已啟用項目 ({r.AppliedFeaturesCount} 項):");
                        foreach (string id in r.EnabledFeatures.OrderBy(x => x))
                        {
                            stdout.WriteLine($"  - {id} ({Loc.Get(id)})");
                        }
                        stdout.WriteLine($"遊戲加速: {r.GameSpeed}x");
                        stdout.WriteLine($"駐軍配額: {r.VillageGarrisonQuotaMultiplier}x");
                        stdout.WriteLine($"平衡模式: {(r.Balance ? "開" : "關")}");
                        if (r.CustomStatsUnitsCount > 0)
                        {
                            stdout.WriteLine($"自訂兵種數值: {r.CustomStatsUnitsCount} 種兵種");
                        }
                    }, logger);
                    return 0;
                }

                case CliCommandType.Restore:
                {
                    var result = await CliCommands.RestoreAsync(options, logger);
                    OutputResult(stdout, options, "restore", result, r =>
                    {
                        stdout.WriteLine($"已完成還原遊戲目錄: {r.GamePath}");
                        stdout.WriteLine($"還原分類: {string.Join(", ", r.CategoriesRestored)}");
                        stdout.WriteLine($"保留自訂地圖: {(r.PreserveCustomMaps ? "是" : "否")}");
                    }, logger);
                    return 0;
                }

                case CliCommandType.Backup:
                {
                    var result = await CliCommands.BackupAsync(options, logger);
                    OutputResult(stdout, options, "backup", result, r =>
                    {
                        stdout.WriteLine($"備份操作 [{r.Action}]: {(r.Success ? "成功" : "失敗")}");
                        stdout.WriteLine($"遊戲目錄: {r.GamePath}");
                        stdout.WriteLine($"缺少備份檔案數: {r.MissingCount}");
                        if (r.MissingFiles.Count > 0)
                        {
                            stdout.WriteLine("缺少的檔案:");
                            foreach (string f in r.MissingFiles) stdout.WriteLine($"  - {f}");
                        }
                    }, logger);
                    return result.Success ? 0 : 1;
                }

                case CliCommandType.Stats:
                {
                    var result = await CliCommands.StatsAsync(options, logger);
                    OutputResult(stdout, options, "stats", result, data =>
                    {
                        if (data is UnitStatItemResult single)
                        {
                            stdout.WriteLine($"兵種: {single.Key} ({single.Name}) - {single.Faction} {single.Tier} {single.UnitType}");
                            stdout.WriteLine($"生命: {single.Hp}, 近戰/遠程傷害: {single.Damage}, 防禦(VW): {single.DefenseVw}, 戰鬥(AW): {single.CombatAw}");
                            stdout.WriteLine($"速度: {single.Speed}, 視野: {single.Sight}, 冷卻: {single.ReloadRelt}, 射程: {single.Range}, 法術半徑: {single.SpellRadius}");
                        }
                        else if (data is List<UnitStatItemResult> list)
                        {
                            stdout.WriteLine($"兵種數值列表 (共 {list.Count} 筆):");
                            foreach (var u in list)
                            {
                                stdout.WriteLine($"  [{u.Key}] {u.Name} (HP:{u.Hp}, Dmg:{u.Damage}, Def:{u.DefenseVw}, Spd:{u.Speed}, Sight:{u.Sight}, Rng:{u.Range})");
                            }
                        }
                        else
                        {
                            stdout.WriteLine(JsonSerializer.Serialize(data, JsonOptions));
                        }
                    }, logger);
                    return 0;
                }

                case CliCommandType.Profile:
                {
                    var result = await CliCommands.ProfileAsync(options, logger);
                    OutputResult(stdout, options, "profile", result, data =>
                    {
                        stdout.WriteLine(JsonSerializer.Serialize(data, JsonOptions));
                    }, logger);
                    return 0;
                }

                case CliCommandType.Saves:
                {
                    var result = await CliCommands.SavesAsync(options, logger);
                    OutputResult(stdout, options, "saves", result, catalog =>
                    {
                        stdout.WriteLine($"遊戲存檔 (共 {catalog.Saves.Count} 個):");
                        foreach (var s in catalog.Saves)
                        {
                            stdout.WriteLine($"  - {s.Folder}: {s.Title} ({s.Level}) [{s.LastWriteTime:yyyy-MM-dd HH:mm}]");
                        }
                        stdout.WriteLine($"備份存檔 (共 {catalog.Backups.Count} 個):");
                        foreach (var b in catalog.Backups)
                        {
                            stdout.WriteLine($"  - {b.FileName}: {b.Title} [{b.BackupTime}]");
                        }
                    }, logger);
                    return 0;
                }

                case CliCommandType.Maps:
                {
                    var result = await CliCommands.MapsAsync(options, logger);
                    OutputResult(stdout, options, "maps", result, maps =>
                    {
                        stdout.WriteLine($"地圖列表 (共 {maps.Count} 張):");
                        foreach (var m in maps)
                        {
                            string custom = m.IsCustom ? " [自訂/可編輯]" : "";
                            stdout.WriteLine($"  - {m.Id}: {m.DisplayName ?? m.Id} ({m.Category}){custom}");
                        }
                    }, logger);
                    return 0;
                }

                case CliCommandType.Help:
                default:
                {
                    var result = CliCommands.GetHelp();
                    OutputResult(stdout, options, "help", result, h =>
                    {
                        stdout.WriteLine($"=== {h.AppName} v{h.Version} ===");
                        stdout.WriteLine($"{h.Description}\n");
                        stdout.WriteLine("可用指令 (Commands):");
                        foreach (var c in h.Commands)
                        {
                            stdout.WriteLine($"  {c.Name,-15} {c.Description}");
                            if (c.Subcommands != null && c.Subcommands.Count > 0)
                            {
                                stdout.WriteLine($"    子指令: {string.Join(", ", c.Subcommands)}");
                            }
                            if (c.Options.Count > 0)
                            {
                                stdout.WriteLine($"    選項: {string.Join(" ", c.Options)}");
                            }
                        }
                        stdout.WriteLine("\n使用範例 (Examples):");
                        foreach (string ex in h.Examples)
                        {
                            stdout.WriteLine($"  {ex}");
                        }
                    }, logger);
                    return 0;
                }
            }
        }
        catch (ArgumentException ex)
        {
            HandleException(stdout, stderr, options, ex, 2, logger);
            return 2;
        }
        catch (Exception ex)
        {
            HandleException(stdout, stderr, options, ex, 1, logger);
            return 1;
        }
    }

    private static void OutputResult<T>(TextWriter stdout, CliOptions options, string command, T data, Action<T> renderText, CliCollectorLogger logger)
    {
        if (options.Json)
        {
            var response = new CliResponse<T>
            {
                Success = true,
                Command = command,
                Data = data,
                Logs = logger.Logs.Count > 0 ? logger.Logs : null
            };
            stdout.WriteLine(JsonSerializer.Serialize(response, JsonOptions));
        }
        else
        {
            renderText(data);
        }
    }

    private static void HandleException(TextWriter stdout, TextWriter stderr, CliOptions options, Exception ex, int exitCode, CliCollectorLogger logger)
    {
        if (options.Json)
        {
            var response = new CliResponse<object>
            {
                Success = false,
                Command = options.Command.ToString().ToLowerInvariant(),
                Error = ex.Message,
                Logs = logger.Logs.Count > 0 ? logger.Logs : null
            };
            stdout.WriteLine(JsonSerializer.Serialize(response, JsonOptions));
        }
        else
        {
            stderr.WriteLine($"錯誤 ({exitCode}): {ex.Message}");
            if (options.Verbose)
            {
                stderr.WriteLine(ex.StackTrace);
            }
        }
    }

    private sealed class CliCollectorLogger : ILogger
    {
        private readonly bool _verbose;
        private readonly bool _isJson;
        private readonly TextWriter _writer;
        public List<string> Logs { get; } = new();

        public CliCollectorLogger(bool verbose, bool isJson, TextWriter writer)
        {
            _verbose = verbose;
            _isJson = isJson;
            _writer = writer;
        }

        public void Log(string message)
        {
            Logs.Add(message);
            if (!_isJson && _verbose)
            {
                _writer.WriteLine($"[LOG] {message}");
            }
        }
    }
}
