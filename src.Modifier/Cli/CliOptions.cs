using System;
using System.Collections.Generic;

namespace AgainstRomeModifier.Cli;

public sealed class CliOptions
{
    public CliCommandType Command { get; set; } = CliCommandType.Help;
    public string? SubAction { get; set; }
    public string? SubArg { get; set; }
    public string? GamePath { get; set; }
    public bool Json { get; set; }
    public bool Verbose { get; set; }
    public List<string>? EnableFeatures { get; set; }
    public List<string>? DisableFeatures { get; set; }
    public bool EnableAll { get; set; }
    public bool DisableAll { get; set; }
    public bool? Balance { get; set; }
    public int? GameSpeed { get; set; }
    public int? VillageGarrisonQuotaMultiplier { get; set; }
    public string? PresetFile { get; set; }
    public string? ProfileFile { get; set; }
    public bool DryRun { get; set; }
    public bool RestoreAll { get; set; }
    public bool RestoreStats { get; set; }
    public bool RestoreCompat { get; set; }
    public bool RestoreLanguage { get; set; }
    public bool PreserveCustomMaps { get; set; } = true;
    public string? OutputFile { get; set; }
    public string? Format { get; set; }
    public string? Category { get; set; }
    public string? Language { get; set; }

    public static CliOptions Parse(string[] args)
    {
        var options = new CliOptions();
        var positionals = new List<string>();

        for (int i = 0; i < args.Length; i++)
        {
            string arg = args[i];

            if (arg.Equals("--cli", StringComparison.OrdinalIgnoreCase))
            {
                // Mode indicator, ignore
                continue;
            }

            if (arg.Equals("--json", StringComparison.OrdinalIgnoreCase))
            {
                options.Json = true;
                continue;
            }

            if (arg.Equals("--verbose", StringComparison.OrdinalIgnoreCase) || arg.Equals("-v", StringComparison.OrdinalIgnoreCase))
            {
                options.Verbose = true;
                continue;
            }

            if (arg.Equals("--game", StringComparison.OrdinalIgnoreCase) || arg.Equals("-g", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length) options.GamePath = args[++i];
                else throw new ArgumentException("--game 缺少路徑參數。");
                continue;
            }

            if (arg.Equals("--enable", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length)
                {
                    string list = args[++i];
                    options.EnableFeatures ??= new List<string>();
                    foreach (string id in list.Split(new[] { ',', ';' }, StringSplitOptions.RemoveEmptyEntries))
                    {
                        options.EnableFeatures.Add(id.Trim());
                    }
                }
                else throw new ArgumentException("--enable 缺少功能 ID 參數。");
                continue;
            }

            if (arg.Equals("--disable", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length)
                {
                    string list = args[++i];
                    options.DisableFeatures ??= new List<string>();
                    foreach (string id in list.Split(new[] { ',', ';' }, StringSplitOptions.RemoveEmptyEntries))
                    {
                        options.DisableFeatures.Add(id.Trim());
                    }
                }
                else throw new ArgumentException("--disable 缺少功能 ID 參數。");
                continue;
            }

            if (arg.Equals("--all", StringComparison.OrdinalIgnoreCase))
            {
                options.EnableAll = true;
                options.RestoreAll = true;
                continue;
            }

            if (arg.Equals("--disable-all", StringComparison.OrdinalIgnoreCase))
            {
                options.DisableAll = true;
                continue;
            }

            if (arg.Equals("--balance", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length && bool.TryParse(args[i + 1], out bool balanceVal))
                {
                    options.Balance = balanceVal;
                    i++;
                }
                else
                {
                    options.Balance = true;
                }
                continue;
            }

            if (arg.Equals("--game-speed", StringComparison.OrdinalIgnoreCase) || arg.Equals("--speed", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length && int.TryParse(args[i + 1], out int speed))
                {
                    if (speed < 1 || speed > 10) throw new ArgumentException("遊戲加速倍率必須介於 1 到 10 之間。");
                    options.GameSpeed = speed;
                    i++;
                }
                else throw new ArgumentException("--game-speed 缺少有效的數值倍率 (1..10)。");
                continue;
            }

            if (arg.Equals("--garrison-quota", StringComparison.OrdinalIgnoreCase) || arg.Equals("--quota", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length && int.TryParse(args[i + 1], out int quota))
                {
                    if (quota < 1 || quota > 10) throw new ArgumentException("村莊駐軍配額倍率必須介於 1 到 10 之間。");
                    options.VillageGarrisonQuotaMultiplier = quota;
                    i++;
                }
                else throw new ArgumentException("--garrison-quota 缺少有效的數值倍率 (1..10)。");
                continue;
            }

            if (arg.Equals("--preset", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length) options.PresetFile = args[++i];
                else throw new ArgumentException("--preset 缺少預設檔案路徑。");
                continue;
            }

            if (arg.Equals("--profile", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length) options.ProfileFile = args[++i];
                else throw new ArgumentException("--profile 缺少設定檔路徑。");
                continue;
            }

            if (arg.Equals("--dry-run", StringComparison.OrdinalIgnoreCase))
            {
                options.DryRun = true;
                continue;
            }

            if (arg.Equals("--stats", StringComparison.OrdinalIgnoreCase))
            {
                options.RestoreStats = true;
                continue;
            }

            if (arg.Equals("--compat", StringComparison.OrdinalIgnoreCase))
            {
                options.RestoreCompat = true;
                continue;
            }

            if (arg.Equals("--language", StringComparison.OrdinalIgnoreCase) || arg.Equals("--lang-pack", StringComparison.OrdinalIgnoreCase))
            {
                options.RestoreLanguage = true;
                continue;
            }

            if (arg.Equals("--preserve-custom-maps", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length && bool.TryParse(args[i + 1], out bool preserveVal))
                {
                    options.PreserveCustomMaps = preserveVal;
                    i++;
                }
                else
                {
                    options.PreserveCustomMaps = true;
                }
                continue;
            }

            if (arg.Equals("--delete-custom-maps", StringComparison.OrdinalIgnoreCase))
            {
                options.PreserveCustomMaps = false;
                continue;
            }

            if (arg.Equals("--output", StringComparison.OrdinalIgnoreCase) || arg.Equals("-o", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length) options.OutputFile = args[++i];
                else throw new ArgumentException("--output 缺少輸出路徑參數。");
                continue;
            }

            if (arg.Equals("--format", StringComparison.OrdinalIgnoreCase) || arg.Equals("-f", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length) options.Format = args[++i];
                else throw new ArgumentException("--format 缺少格式參數 (ini 或 json)。");
                continue;
            }

            if (arg.Equals("--category", StringComparison.OrdinalIgnoreCase) || arg.Equals("-c", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length) options.Category = args[++i];
                else throw new ArgumentException("--category 缺少分類參數。");
                continue;
            }

            if (arg.Equals("--unit", StringComparison.OrdinalIgnoreCase) || arg.Equals("-u", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length) options.SubArg = args[++i];
                else throw new ArgumentException("--unit 缺少兵種代號參數。");
                continue;
            }

            if (arg.Equals("--lang", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 < args.Length) options.Language = args[++i];
                else throw new ArgumentException("--lang 缺少語系參數。");
                continue;
            }

            if (arg.Equals("--help", StringComparison.OrdinalIgnoreCase) || arg.Equals("-h", StringComparison.OrdinalIgnoreCase) || arg.Equals("/?", StringComparison.OrdinalIgnoreCase))
            {
                options.Command = CliCommandType.Help;
                return options;
            }

            if (!arg.StartsWith('-'))
            {
                positionals.Add(arg);
            }
        }

        if (positionals.Count > 0)
        {
            options.Command = positionals[0].ToLowerInvariant() switch
            {
                "detect-game" or "game" => CliCommandType.DetectGame,
                "status" or "detect" or "info" => CliCommandType.Status,
                "features" or "list" => CliCommandType.Features,
                "apply" => CliCommandType.Apply,
                "restore" => CliCommandType.Restore,
                "backup" => CliCommandType.Backup,
                "stats" => CliCommandType.Stats,
                "profile" => CliCommandType.Profile,
                "saves" => CliCommandType.Saves,
                "maps" => CliCommandType.Maps,
                "help" => CliCommandType.Help,
                _ => throw new ArgumentException($"未知的指令: '{positionals[0]}'。使用 'AgainstRomeModifier.exe help' 檢視說明。")
            };

            if (positionals.Count > 1)
            {
                options.SubAction = positionals[1];
            }
            if (positionals.Count > 2 && options.SubArg == null)
            {
                options.SubArg = positionals[2];
            }
        }

        return options;
    }
}
