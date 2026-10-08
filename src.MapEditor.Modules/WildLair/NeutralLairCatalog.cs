namespace AgainstRomeMapEditor.Modules.WildLair;

/// <summary>
/// 野外中立巢穴目錄（NeutralLairCatalog）：
/// 管理所有中立野獸巢穴、蠻族盜匪營地與古老哨站原型定義，
/// 支援階級難度過濾、生態環境篩選與威脅度評估。
/// </summary>
public sealed class NeutralLairCatalog
{
    private static readonly Lazy<NeutralLairCatalog> _defaultInstance = new(() => new NeutralLairCatalog(CreateCanonicalDefinitions()));
    public static NeutralLairCatalog Default => _defaultInstance.Value;

    private readonly Dictionary<string, NeutralLairDefinition> _definitions;

    public IReadOnlyList<NeutralLairDefinition> AllDefinitions => _definitions.Values.ToArray();

    public NeutralLairCatalog(IEnumerable<NeutralLairDefinition> definitions)
    {
        ArgumentNullException.ThrowIfNull(definitions);
        _definitions = new Dictionary<string, NeutralLairDefinition>(StringComparer.OrdinalIgnoreCase);
        foreach (var def in definitions)
        {
            ValidateDefinition(def);
            _definitions[def.Id] = def;
        }
    }

    public NeutralLairDefinition? GetById(string id)
    {
        if (string.IsNullOrWhiteSpace(id)) return null;
        return _definitions.GetValueOrDefault(id);
    }

    public IEnumerable<NeutralLairDefinition> GetByCategory(LairCategory category) =>
        _definitions.Values.Where(def => def.Category == category);

    public IEnumerable<NeutralLairDefinition> GetByTier(LairDifficultyTier tier) =>
        _definitions.Values.Where(def => def.Tier == tier);

    public IEnumerable<NeutralLairDefinition> Filter(
        LairCategory? category = null,
        LairDifficultyTier? tier = null,
        string? biome = null)
    {
        return _definitions.Values.Where(def =>
            (!category.HasValue || def.Category == category.Value) &&
            (!tier.HasValue || def.Tier == tier.Value) &&
            (string.IsNullOrWhiteSpace(biome) || string.Equals(def.BiomeAffinity, biome, StringComparison.OrdinalIgnoreCase)));
    }

    /// <summary>
    /// 計算巢穴綜合危險度評分（Threat Rating: 10 ~ 100+）。
    /// 依初始常駐守衛戰力、刷怪波次頻率與單位人數綜合權重評估。
    /// </summary>
    public static float CalculateThreatRating(NeutralLairDefinition definition)
    {
        ArgumentNullException.ThrowIfNull(definition);

        float guardPower = 0f;
        foreach (var guard in definition.DefaultGuards)
        {
            guardPower += guard.Count * 2.5f;
            if (guard.RespawnIntervalSeconds > 0)
                guardPower += 150f / Math.Max(30f, guard.RespawnIntervalSeconds);
        }

        float wavePower = 0f;
        foreach (var wave in definition.WaveRules)
        {
            float ratePerMinute = 60f / Math.Max(30f, wave.IntervalSeconds);
            wavePower += wave.SpawnCount * ratePerMinute * 3.0f * Math.Min(3, wave.MaxActiveWaves);
        }

        float tierMultiplier = (int)definition.Tier switch
        {
            1 => 1.0f,
            2 => 1.35f,
            3 => 1.8f,
            4 => 2.5f,
            _ => 1.0f
        };

        return (guardPower + wavePower) * tierMultiplier;
    }

    public static void ValidateDefinition(NeutralLairDefinition def)
    {
        ArgumentNullException.ThrowIfNull(def);
        if (string.IsNullOrWhiteSpace(def.Id))
            throw new ArgumentException("巢穴識別碼不得為空。", nameof(def));
        if (string.IsNullOrWhiteSpace(def.DisplayNameZh))
            throw new ArgumentException("巢穴繁體中文名稱不得為空。", nameof(def));
        if (string.IsNullOrWhiteSpace(def.NativeBuildingOrLandscapeType))
            throw new ArgumentException("原生建築或地景代碼不得為空。", nameof(def));
        if (def.FootprintRadiusTiles <= 0f)
            throw new ArgumentOutOfRangeException(nameof(def), "佔地半徑必須大於 0。");
        if (def.DefaultGuards is null)
            throw new ArgumentNullException(nameof(def), "常駐守衛清單不得為 null。");
        if (def.WaveRules is null)
            throw new ArgumentNullException(nameof(def), "刷怪波次規則不得為 null。");
        if (def.Loot is null)
            throw new ArgumentNullException(nameof(def), "戰利品獎勵不得為 null。");

        foreach (var wave in def.WaveRules)
        {
            if (wave.IntervalSeconds < 10)
                throw new ArgumentOutOfRangeException(nameof(def), $"刷怪週期過短（{wave.IntervalSeconds} 秒），最低間隔須 >= 10 秒。");
            if (wave.SpawnCount is < 1 or > 20)
                throw new ArgumentOutOfRangeException(nameof(def), $"刷怪人數必須介於 1 至 20 人之間。");
        }
    }

    /// <summary>
    /// 內建原版相容野外中立巢穴清單。
    /// </summary>
    private static IReadOnlyList<NeutralLairDefinition> CreateCanonicalDefinitions()
    {
        return new List<NeutralLairDefinition>
        {
            // 1. 小型野狼穴 (T1)
            new(
                Id: "LAIR_WOLF_DEN_SMALL",
                DisplayNameZh: "荒野狼穴",
                DisplayNameEn: "Wild Wolf Den",
                Category: LairCategory.BeastDen,
                Tier: LairDifficultyTier.Tier1Scout,
                NativeBuildingOrLandscapeType: "LanGerStein01",
                FootprintRadiusTiles: 2.5f,
                DefaultGuards: new[]
                {
                    new LairGuardUnit("GER_INF01", Count: 4, PatrolRadiusTiles: 3.5f, AggroRadiusTiles: 7.0f, RespawnIntervalSeconds: 60)
                },
                WaveRules: new[]
                {
                    new LairWaveSpawnRule("wolf_foragers", "GER_INF01", SpawnCount: 2, IntervalSeconds: 90, InitialDelaySeconds: 90, MaxActiveWaves: 2, SpawnRadiusTiles: 2.0f)
                },
                Loot: new LairLootReward(Wood: 40, Food: 160, Gold: 20, HonorPoints: 15, CompletionMessage: "Wolf den eradicated!"),
                BiomeAffinity: "TemperateForest",
                Description: "隱匿於岩石叢中的野狼洞穴，定期派出覓食狼群襲擊落單的伐木工與村民。"
            ),

            // 2. 嗜血巨熊洞窟 (T2)
            new(
                Id: "LAIR_BEAR_CAVE_FEROCIOUS",
                DisplayNameZh: "嗜血巨熊岩窟",
                DisplayNameEn: "Ferocious Bear Grotto",
                Category: LairCategory.BeastDen,
                Tier: LairDifficultyTier.Tier2Standard,
                NativeBuildingOrLandscapeType: "LanGerFelsen01",
                FootprintRadiusTiles: 3.5f,
                DefaultGuards: new[]
                {
                    new LairGuardUnit("GER_INF02", Count: 2, PatrolRadiusTiles: 2.0f, AggroRadiusTiles: 6.0f, RespawnIntervalSeconds: 120)
                },
                WaveRules: new[]
                {
                    new LairWaveSpawnRule("roaming_bear", "GER_INF02", SpawnCount: 1, IntervalSeconds: 150, InitialDelaySeconds: 150, MaxActiveWaves: 2, SpawnRadiusTiles: 2.5f)
                },
                Loot: new LairLootReward(Wood: 0, Food: 350, Gold: 40, HonorPoints: 30, CompletionMessage: "Ferocious bear slain! The cave is clear."),
                BiomeAffinity: "Alpine",
                Description: "盤踞於深山岩壁的大型掠食者巢穴，盤據豐饒草場，守衛兇猛且領域性極強。"
            ),

            // 3. 密林野豬灌木叢 (T1)
            new(
                Id: "LAIR_WILD_BOAR_GROVE",
                DisplayNameZh: "密林野豬叢",
                DisplayNameEn: "Wild Boar Thicket",
                Category: LairCategory.BeastDen,
                Tier: LairDifficultyTier.Tier1Scout,
                NativeBuildingOrLandscapeType: "LanGerGest01",
                FootprintRadiusTiles: 2.0f,
                DefaultGuards: new[]
                {
                    new LairGuardUnit("GER_INF01", Count: 5, PatrolRadiusTiles: 3.0f, AggroRadiusTiles: 5.5f, RespawnIntervalSeconds: 80)
                },
                WaveRules: new[]
                {
                    new LairWaveSpawnRule("boar_pack", "GER_INF01", SpawnCount: 2, IntervalSeconds: 110, InitialDelaySeconds: 70, MaxActiveWaves: 2, SpawnRadiusTiles: 1.8f)
                },
                Loot: new LairLootReward(Wood: 50, Food: 280, Gold: 10, HonorPoints: 20, CompletionMessage: "Wild boar thicket cleared! Rich hunting rewards gained."),
                BiomeAffinity: "TemperateForest",
                Description: "灌木叢密佈的野豬出沒地，雖為獵人糧食來源，但會頻繁拱壞林區路徑。"
            ),

            // 4. 蠻族前哨斥候營 (T1)
            new(
                Id: "LAIR_BARBARIAN_SCOUT_POST",
                DisplayNameZh: "蠻族流寇前哨帳",
                DisplayNameEn: "Barbarian Scout Post",
                Category: LairCategory.BarbarianCamp,
                Tier: LairDifficultyTier.Tier1Scout,
                NativeBuildingOrLandscapeType: "BauGerZelt01",
                FootprintRadiusTiles: 3.0f,
                DefaultGuards: new[]
                {
                    new LairGuardUnit("GER_INF01", Count: 6, PatrolRadiusTiles: 4.0f, AggroRadiusTiles: 8.0f, RespawnIntervalSeconds: 90)
                },
                WaveRules: new[]
                {
                    new LairWaveSpawnRule("scout_raiders", "GER_INF01", SpawnCount: 3, IntervalSeconds: 120, InitialDelaySeconds: 120, MaxActiveWaves: 2, SpawnRadiusTiles: 3.0f, AggroBehavior: "PatrolHostile")
                },
                Loot: new LairLootReward(Wood: 120, Food: 80, Gold: 50, HonorPoints: 25, CompletionMessage: "Barbarian outpost burned to the ground!"),
                BiomeAffinity: "TemperateForest",
                Description: "荒野邊緣的粗製帳棚哨站，游擊匪徒常以此為據點伺機偷襲邊境聚落。"
            ),

            // 5. 日耳曼流寇大營 (T2)
            new(
                Id: "LAIR_BARBARIAN_RAIDER_CAMP",
                DisplayNameZh: "日耳曼蠻族掠奪營",
                DisplayNameEn: "Germanic Raider Stronghold",
                Category: LairCategory.BarbarianCamp,
                Tier: LairDifficultyTier.Tier2Standard,
                NativeBuildingOrLandscapeType: "BauGerBar00",
                FootprintRadiusTiles: 5.0f,
                DefaultGuards: new[]
                {
                    new LairGuardUnit("GER_INF01", Count: 12, PatrolRadiusTiles: 5.5f, AggroRadiusTiles: 11.0f, RespawnIntervalSeconds: 120)
                },
                WaveRules: new[]
                {
                    new LairWaveSpawnRule("raiding_warband", "GER_INF01", SpawnCount: 6, IntervalSeconds: 180, InitialDelaySeconds: 200, MaxActiveWaves: 3, SpawnRadiusTiles: 4.0f, AggroBehavior: "RaidNearestSettlement")
                },
                Loot: new LairLootReward(Wood: 250, Food: 200, Gold: 120, HonorPoints: 50, CompletionMessage: "Germanic raider camp destroyed!"),
                BiomeAffinity: "TemperateForest",
                Description: "結構完整的原木掠奪者大營，擁有重裝常駐守衛，定期派遣洗劫隊伍襲擊敵對陣營。"
            ),

            // 6. 羅馬逃兵石砌哨站 (T3)
            new(
                Id: "LAIR_ROMAN_DESERTERS_OUTPOST",
                DisplayNameZh: "羅馬逃兵石砌碉堡",
                DisplayNameEn: "Roman Deserters Watchpost",
                Category: LairCategory.BanditStronghold,
                Tier: LairDifficultyTier.Tier3Major,
                NativeBuildingOrLandscapeType: "BauRomWac00",
                FootprintRadiusTiles: 4.5f,
                DefaultGuards: new[]
                {
                    new LairGuardUnit("ROM_INF01", Count: 10, PatrolRadiusTiles: 4.0f, AggroRadiusTiles: 12.0f, RespawnIntervalSeconds: 150)
                },
                WaveRules: new[]
                {
                    new LairWaveSpawnRule("legion_renegades", "ROM_INF01", SpawnCount: 5, IntervalSeconds: 210, InitialDelaySeconds: 240, MaxActiveWaves: 2, SpawnRadiusTiles: 3.5f, AggroBehavior: "PatrolHostile")
                },
                Loot: new LairLootReward(Wood: 100, Food: 150, Gold: 300, HonorPoints: 75, CompletionMessage: "Rogue Roman watchtower fallen! Imperial gold seized."),
                BiomeAffinity: "Mediterranean",
                Description: "佔據羅馬石造瞭望塔的逃亡軍團殘兵，裝備齊全且防禦嚴密，剿滅可獲得大量金幣戰利品。"
            ),

            // 7. 匈奴遊牧劫掠大營 (T2)
            new(
                Id: "LAIR_HUNNIC_NOMAD_CAMP",
                DisplayNameZh: "匈奴遊牧劫掠帳",
                DisplayNameEn: "Hunnic Raider Yurt",
                Category: LairCategory.BarbarianCamp,
                Tier: LairDifficultyTier.Tier2Standard,
                NativeBuildingOrLandscapeType: "BauHunZelt01",
                FootprintRadiusTiles: 4.0f,
                DefaultGuards: new[]
                {
                    new LairGuardUnit("HUN_INF01", Count: 8, PatrolRadiusTiles: 6.0f, AggroRadiusTiles: 12.0f, RespawnIntervalSeconds: 90)
                },
                WaveRules: new[]
                {
                    new LairWaveSpawnRule("nomad_skirmishers", "HUN_INF01", SpawnCount: 4, IntervalSeconds: 140, InitialDelaySeconds: 150, MaxActiveWaves: 2, SpawnRadiusTiles: 3.0f, AggroBehavior: "RaidNearestSettlement")
                },
                Loot: new LairLootReward(Wood: 80, Food: 150, Gold: 150, HonorPoints: 45, CompletionMessage: "Hunnic yurt camp cleared!"),
                BiomeAffinity: "Steppe",
                Description: "大草原上的流動劫掠帳棚，部隊以快速機動見長，反覆洗劫周遭平原資源點。"
            ),

            // 8. 凱爾特狂戰士巨石圈 (T2)
            new(
                Id: "LAIR_CELTIC_STONE_CIRCLE",
                DisplayNameZh: "凱爾特狂戰士石陣",
                DisplayNameEn: "Celtic Berserker Stone Ring",
                Category: LairCategory.SacredGroveOrRuins,
                Tier: LairDifficultyTier.Tier2Standard,
                NativeBuildingOrLandscapeType: "LanKelStein01",
                FootprintRadiusTiles: 5.0f,
                DefaultGuards: new[]
                {
                    new LairGuardUnit("KEL_INF01", Count: 8, PatrolRadiusTiles: 4.5f, AggroRadiusTiles: 9.0f, RespawnIntervalSeconds: 100)
                },
                WaveRules: new[]
                {
                    new LairWaveSpawnRule("fanatic_zealots", "KEL_INF01", SpawnCount: 4, IntervalSeconds: 160, InitialDelaySeconds: 180, MaxActiveWaves: 2, SpawnRadiusTiles: 3.5f, AggroBehavior: "PatrolHostile")
                },
                Loot: new LairLootReward(Wood: 60, Food: 80, Gold: 200, HonorPoints: 60, CompletionMessage: "Celtic stone circle cleansed of fanatics!"),
                BiomeAffinity: "TemperateForest",
                Description: "深山中神秘的凱爾特儀式巨石環，集結了不畏死亡的狂戰信徒，擅長近戰狂暴衝擊。"
            )
        };
    }
}
