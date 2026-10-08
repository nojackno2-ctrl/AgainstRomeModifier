using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.Nature;

/// <summary>
/// 生態圈單一物種定義項。包含名稱前綴、所屬層級、出現權重、佔位尺寸與地形適配限制。
/// </summary>
public sealed record BiomeSpeciesEntry(
    string NamePrefix,
    VegetationLayer Layer,
    double Weight,
    float FootprintRadiusTiles = 0.75f,
    byte MinElevation = 0,
    byte MaxElevation = 255,
    float MaxSlope = 35.0f,
    string? DescriptionZh = null);

/// <summary>
/// 已成功映射至遊戲實體範本（<see cref="LevelObjectTemplate"/>）的解析物種。
/// </summary>
public sealed record ResolvedSpecies(
    LevelObjectTemplate Template,
    string Name,
    VegetationLayer Layer,
    double Weight,
    float FootprintRadiusTiles,
    byte MinElevation,
    byte MaxElevation,
    float MaxSlope);

/// <summary>
/// 生態圈已解析之物種名冊。提供按生態分層進行高效加權隨機抽選。
/// </summary>
public sealed class ResolvedEcologyRoster
{
    private readonly Dictionary<VegetationLayer, List<ResolvedSpecies>> _byLayer = new();

    public ResolvedEcologyRoster(IEnumerable<ResolvedSpecies> species)
    {
        ArgumentNullException.ThrowIfNull(species);
        foreach (VegetationLayer layer in Enum.GetValues<VegetationLayer>())
            _byLayer[layer] = new List<ResolvedSpecies>();

        foreach (ResolvedSpecies item in species)
            _byLayer[item.Layer].Add(item);
    }

    public IReadOnlyList<ResolvedSpecies> GetLayerSpecies(VegetationLayer layer) => _byLayer.GetValueOrDefault(layer) ?? (IReadOnlyList<ResolvedSpecies>)Array.Empty<ResolvedSpecies>();

    public bool HasLayer(VegetationLayer layer) => _byLayer.TryGetValue(layer, out var list) && list.Count > 0;

    public int TotalSpeciesCount => _byLayer.Values.Sum(list => list.Count);

    /// <summary>
    /// 從指定生境層依權重隨機挑選一株物種。若該層為空則回傳 false。
    /// </summary>
    public bool TryPick(VegetationLayer layer, Random random, out ResolvedSpecies picked)
    {
        picked = null!;
        if (!_byLayer.TryGetValue(layer, out var list) || list.Count == 0) return false;
        if (list.Count == 1) { picked = list[0]; return true; }

        double totalWeight = list.Sum(s => s.Weight);
        if (totalWeight <= 0) { picked = list[random.Next(list.Count)]; return true; }

        double roll = random.NextDouble() * totalWeight;
        double current = 0;
        foreach (ResolvedSpecies item in list)
        {
            current += item.Weight;
            if (roll <= current) { picked = item; return true; }
        }
        picked = list[^1];
        return true;
    }
}

/// <summary>
/// 生態圈群落設定檔。定義不同歷史地理區域的植被組合、生態分層閾值與環境適配參數。
/// </summary>
public sealed class BiomeEcologyProfile
{
    public BiomeType Biome { get; init; }
    public string RegionCode { get; init; } = "";
    public string DisplayNameZh { get; init; } = "";
    public string DisplayNameEn { get; init; } = "";

    /// <summary>深林核心林冠雜訊閾值（0.0 ~ 1.0）。雜訊大於此值時生成高密林冠喬木。</summary>
    public float CanopyThreshold { get; init; } = 0.58f;

    /// <summary>林緣次生灌木雜訊閾值。介於此值與 CanopyThreshold 之間時生成灌木與次冠層。</summary>
    public float UnderstoryThreshold { get; init; } = 0.40f;

    /// <summary>林窗草花雜訊閾值。介於此值與 UnderstoryThreshold 之間時生成野花與草叢。</summary>
    public float GroundFloraThreshold { get; init; } = 0.22f;

    /// <summary>水岸生境高度緩衝差（單位：米/高度步長單位）。水面附近此高度差內視為濱水帶。</summary>
    public float RiparianElevationDelta { get; init; } = 3.0f;

    /// <summary>懸崖陡坡判定閾值（度數）。超過此坡度禁止生長喬木，轉為散佈岩石或裸露地。</summary>
    public float CliffSlopeThreshold { get; init; } = 25.0f;

    /// <summary>該生態圈定義的各類物種清單。</summary>
    public IReadOnlyList<BiomeSpeciesEntry> Species { get; init; } = Array.Empty<BiomeSpeciesEntry>();

    // =========================================================================
    // 預設生態圈定義 (Presets)
    // =========================================================================

    /// <summary>
    /// 日耳曼原始森林（Germanic Coniferous &amp; Mixed Forest - "Ger"）
    /// 針葉冷杉雲杉為主體，林下刺灌木、原野野花與花崗岩塊。
    /// </summary>
    public static BiomeEcologyProfile GermanicForest { get; } = new()
    {
        Biome = BiomeType.GermanicForest,
        RegionCode = "Ger",
        DisplayNameZh = "日耳曼針葉黑森林",
        DisplayNameEn = "Germanic Black Forest",
        CanopyThreshold = 0.55f,
        UnderstoryThreshold = 0.38f,
        GroundFloraThreshold = 0.20f,
        RiparianElevationDelta = 3.0f,
        CliffSlopeThreshold = 26.0f,
        Species = new BiomeSpeciesEntry[]
        {
            // 喬木層 (Canopy): 高大冷杉 (Nad)、雲杉 (Fic)、白樺 (Bir)、闊葉 (Lau)
            new("LanGerNad", VegetationLayer.Canopy, 60.0, FootprintRadiusTiles: 1.15f, MaxSlope: 32f, DescriptionZh: "日耳曼針葉高樹 (Tanne/Fichte)"),
            new("LanGerBir", VegetationLayer.Canopy, 18.0, FootprintRadiusTiles: 0.90f, MaxSlope: 35f, DescriptionZh: "高地白樺 (Birke)"),
            new("LanGerLau", VegetationLayer.Canopy, 12.0, FootprintRadiusTiles: 1.05f, MaxSlope: 30f, DescriptionZh: "混交闊葉樹 (Laubbaum)"),
            new("LanGerEic", VegetationLayer.Canopy, 10.0, FootprintRadiusTiles: 1.25f, MaxSlope: 28f, DescriptionZh: "古老巨橡 (Eiche)"),

            // 灌木層 (Understory): 灌木叢 (Bus)、荊棘刺叢 (Dor)、地被矮灌 (Bod)
            new("LanGerBus", VegetationLayer.Understory, 45.0, FootprintRadiusTiles: 0.65f, MaxSlope: 40f, DescriptionZh: "日耳曼林下灌木 (Busch)"),
            new("LanGerDor", VegetationLayer.Understory, 30.0, FootprintRadiusTiles: 0.55f, MaxSlope: 42f, DescriptionZh: "黑森林荊棘 (Dornen)"),
            new("LanGerBod", VegetationLayer.Understory, 25.0, FootprintRadiusTiles: 0.50f, MaxSlope: 45f, DescriptionZh: "地被矮灌木 (Bodenbusch)"),

            // 地表草花層 (GroundFlora): 野草 (Gra)、林地野花 (Blu/Bli)
            new("LanGerGra", VegetationLayer.GroundFlora, 55.0, FootprintRadiusTiles: 0.35f, MaxSlope: 50f, DescriptionZh: "日耳曼林間野草 (Gras)"),
            new("LanGerBlu", VegetationLayer.GroundFlora, 30.0, FootprintRadiusTiles: 0.35f, MaxSlope: 45f, DescriptionZh: "耐陰野花 (Blumen)"),
            new("LanGerBli", VegetationLayer.GroundFlora, 15.0, FootprintRadiusTiles: 0.35f, MaxSlope: 45f, DescriptionZh: "盛開林花 (Blüten)"),

            // 水岸生境層 (Riparian): 湖澤蘆葦 (Sch)、河岸垂柳 (Wei)、浮萍 (Ent)
            new("LanGerSch", VegetationLayer.Riparian, 50.0, FootprintRadiusTiles: 0.45f, MaxSlope: 20f, DescriptionZh: "水岸蘆葦 (Schilf)"),
            new("LanGerWei", VegetationLayer.Riparian, 30.0, FootprintRadiusTiles: 0.85f, MaxSlope: 22f, DescriptionZh: "濱水垂柳 (Weide)"),
            new("LanGerEnt", VegetationLayer.Riparian, 20.0, FootprintRadiusTiles: 0.40f, MaxSlope: 15f, DescriptionZh: "澤地水草 (Entengrütze)"),

            // 地質岩石層 (Rock): 沉積岩塊 (Ste)、懸崖石壁 (Fel)
            new("LanGerSte", VegetationLayer.Rock, 60.0, FootprintRadiusTiles: 0.80f, MaxSlope: 90f, DescriptionZh: "黑森林岩石塊 (Stein)"),
            new("LanGerFel", VegetationLayer.Rock, 40.0, FootprintRadiusTiles: 1.20f, MaxSlope: 90f, DescriptionZh: "斷崖花崗巨石 (Fels)"),
        }
    };

    /// <summary>
    /// 多瑙河溫帶闊葉林（Danubian Temperate Broadleaf Forest - "Kel"/"Ger"）
    /// 溫暖丘陵大櫟木、山毛櫸、繁茂樹籬灌木與繁茂水岸。
    /// </summary>
    public static BiomeEcologyProfile DanubeBroadleaf { get; } = new()
    {
        Biome = BiomeType.DanubeBroadleaf,
        RegionCode = "Ger",
        DisplayNameZh = "多瑙河溫帶闊葉林",
        DisplayNameEn = "Danubian Broadleaf Forest",
        CanopyThreshold = 0.52f,
        UnderstoryThreshold = 0.36f,
        GroundFloraThreshold = 0.18f,
        RiparianElevationDelta = 3.5f,
        CliffSlopeThreshold = 28.0f,
        Species = new BiomeSpeciesEntry[]
        {
            // 喬木層: 櫟木 (Eic)、山毛櫸 (Buc)、闊葉樹 (Lau)、樺樹 (Bir)
            new("LanGerEic", VegetationLayer.Canopy, 45.0, FootprintRadiusTiles: 1.25f, MaxSlope: 30f, DescriptionZh: "溫帶巨橡 (Eiche)"),
            new("LanGerBuc", VegetationLayer.Canopy, 25.0, FootprintRadiusTiles: 1.15f, MaxSlope: 32f, DescriptionZh: "多瑙河山毛櫸 (Buche)"),
            new("LanGerLau", VegetationLayer.Canopy, 20.0, FootprintRadiusTiles: 1.05f, MaxSlope: 30f, DescriptionZh: "沖積平原闊葉樹 (Laubbaum)"),
            new("LanGerBir", VegetationLayer.Canopy, 10.0, FootprintRadiusTiles: 0.90f, MaxSlope: 35f, DescriptionZh: "丘陵樺木 (Birke)"),

            // 灌木層: 灌叢 (Bus)、樹籬 (Hec)、灌木 (Str)
            new("LanGerBus", VegetationLayer.Understory, 50.0, FootprintRadiusTiles: 0.65f, MaxSlope: 40f, DescriptionZh: "闊葉林灌木 (Busch)"),
            new("LanGerHec", VegetationLayer.Understory, 30.0, FootprintRadiusTiles: 0.60f, MaxSlope: 38f, DescriptionZh: "自然樹籬灌木 (Hecke)"),
            new("LanGerStr", VegetationLayer.Understory, 20.0, FootprintRadiusTiles: 0.55f, MaxSlope: 42f, DescriptionZh: "原野矮灌木 (Strauch)"),

            // 地表草花: 野草 (Gra)、繁花 (Blu/Bli)
            new("LanGerGra", VegetationLayer.GroundFlora, 40.0, FootprintRadiusTiles: 0.35f, MaxSlope: 50f, DescriptionZh: "豐茂草地 (Gras)"),
            new("LanGerBlu", VegetationLayer.GroundFlora, 35.0, FootprintRadiusTiles: 0.35f, MaxSlope: 45f, DescriptionZh: "谷地野花 (Blumen)"),
            new("LanGerBli", VegetationLayer.GroundFlora, 25.0, FootprintRadiusTiles: 0.35f, MaxSlope: 45f, DescriptionZh: "河谷繽紛花叢 (Blüten)"),

            // 水岸生境: 水岸垂柳 (Wei)、蘆葦 (Sch)、浮萍 (Ent)
            new("LanGerWei", VegetationLayer.Riparian, 40.0, FootprintRadiusTiles: 0.85f, MaxSlope: 22f, DescriptionZh: "河岸垂柳 (Weide)"),
            new("LanGerSch", VegetationLayer.Riparian, 35.0, FootprintRadiusTiles: 0.45f, MaxSlope: 20f, DescriptionZh: "沿岸蘆葦 (Schilf)"),
            new("LanGerEnt", VegetationLayer.Riparian, 25.0, FootprintRadiusTiles: 0.40f, MaxSlope: 15f, DescriptionZh: "淺灘浮萍 (Entengrütze)"),

            // 地質岩石
            new("LanGerSte", VegetationLayer.Rock, 65.0, FootprintRadiusTiles: 0.75f, MaxSlope: 90f, DescriptionZh: "河床卵石岩塊 (Stein)"),
            new("LanGerFel", VegetationLayer.Rock, 35.0, FootprintRadiusTiles: 1.10f, MaxSlope: 90f, DescriptionZh: "丘陵風化岩壁 (Fels)"),
        }
    };

    /// <summary>
    /// 義大利地中海灌木丘陵（Italian Mediterranean Scrub Hills - "Ita"/"Rom"）
    /// 絲柏、石松、耐旱馬基灌木、露頭石灰岩石。
    /// </summary>
    public static BiomeEcologyProfile ItalianHills { get; } = new()
    {
        Biome = BiomeType.ItalianHills,
        RegionCode = "Ita",
        DisplayNameZh = "義大利地中海灌木丘陵",
        DisplayNameEn = "Italian Mediterranean Hills",
        CanopyThreshold = 0.62f,
        UnderstoryThreshold = 0.42f,
        GroundFloraThreshold = 0.24f,
        RiparianElevationDelta = 2.5f,
        CliffSlopeThreshold = 22.0f,
        Species = new BiomeSpeciesEntry[]
        {
            // 喬木層: 義大利絲柏 (Zyp)、地中海石松 (Pin)、橄欖/果樹 (Obs)
            new("LanItaZyp", VegetationLayer.Canopy, 50.0, FootprintRadiusTiles: 0.70f, MaxSlope: 35f, DescriptionZh: "義大利絲柏 (Zypresse)"),
            new("LanItaPin", VegetationLayer.Canopy, 35.0, FootprintRadiusTiles: 1.15f, MaxSlope: 28f, DescriptionZh: "地中海石松 (Pinie)"),
            new("LanItaObs", VegetationLayer.Canopy, 15.0, FootprintRadiusTiles: 0.85f, MaxSlope: 25f, DescriptionZh: "丘陵果木 (Obstbaum)"),

            // 灌木層: 硬葉馬基灌木 (Bus)、棘刺叢 (Dor)、矮地被 (Bod)
            new("LanItaBus", VegetationLayer.Understory, 55.0, FootprintRadiusTiles: 0.60f, MaxSlope: 45f, DescriptionZh: "地中海硬葉灌木 (Busch)"),
            new("LanItaDor", VegetationLayer.Understory, 25.0, FootprintRadiusTiles: 0.50f, MaxSlope: 45f, DescriptionZh: "乾燥棘刺叢 (Dornen)"),
            new("LanItaBod", VegetationLayer.Understory, 20.0, FootprintRadiusTiles: 0.45f, MaxSlope: 45f, DescriptionZh: "旱生矮灌叢 (Bodenbusch)"),

            // 地表草花: 旱草 (Gra)、乾燥野花 (Blu)
            new("LanItaGra", VegetationLayer.GroundFlora, 65.0, FootprintRadiusTiles: 0.35f, MaxSlope: 50f, DescriptionZh: "旱地草叢 (Gras)"),
            new("LanItaBlu", VegetationLayer.GroundFlora, 35.0, FootprintRadiusTiles: 0.35f, MaxSlope: 45f, DescriptionZh: "陽生野花 (Blumen)"),

            // 水岸生境: 溪澗水草 (Sch)、睡蓮 (Ser)
            new("LanItaSch", VegetationLayer.Riparian, 60.0, FootprintRadiusTiles: 0.45f, MaxSlope: 20f, DescriptionZh: "溪谷水草 (Schilf)"),
            new("LanItaSer", VegetationLayer.Riparian, 40.0, FootprintRadiusTiles: 0.45f, MaxSlope: 15f, DescriptionZh: "泉池睡蓮 (Seerose)"),

            // 地質岩石: 露頭白堊岩/石灰岩塊 (Ste)、岩峰 (Fel)
            new("LanItaSte", VegetationLayer.Rock, 55.0, FootprintRadiusTiles: 0.80f, MaxSlope: 90f, DescriptionZh: "石灰岩塊 (Stein)"),
            new("LanItaFel", VegetationLayer.Rock, 45.0, FootprintRadiusTiles: 1.15f, MaxSlope: 90f, DescriptionZh: "裸露石灰岩壁 (Fels)"),
        }
    };

    /// <summary>
    /// 不列顛沼澤荒原（British Bog &amp; Heathland - "Bri"/"Kel"）
    /// 石楠灌叢、金雀花、泥炭沼澤莎草蘆葦與荒原巨石。
    /// </summary>
    public static BiomeEcologyProfile BritishMarsh { get; } = new()
    {
        Biome = BiomeType.BritishMarsh,
        RegionCode = "Kel",
        DisplayNameZh = "不列顛沼澤荒原",
        DisplayNameEn = "British Bog & Heathland",
        CanopyThreshold = 0.70f, // 喬木非常稀少，僅深處零星分佈
        UnderstoryThreshold = 0.45f,
        GroundFloraThreshold = 0.20f,
        RiparianElevationDelta = 4.0f, // 濕地濱水範圍廣
        CliffSlopeThreshold = 24.0f,
        Species = new BiomeSpeciesEntry[]
        {
            // 喬木層: 矮曲樺樹 (Bir)、孤櫟 (Eic)、枯樁 (Sto)
            new("LanKelBir", VegetationLayer.Canopy, 40.0, FootprintRadiusTiles: 0.85f, MaxSlope: 30f, DescriptionZh: "荒原矮樺 (Birke)"),
            new("LanKelEic", VegetationLayer.Canopy, 30.0, FootprintRadiusTiles: 1.05f, MaxSlope: 28f, DescriptionZh: "風蝕孤櫟 (Eiche)"),
            new("LanKelSto", VegetationLayer.Canopy, 30.0, FootprintRadiusTiles: 0.60f, MaxSlope: 35f, DescriptionZh: "泥炭枯樹樁 (Stock)"),

            // 灌木層: 泥炭灌叢 (Bus)、石楠棘刺 (Dor)
            new("LanKelBus", VegetationLayer.Understory, 55.0, FootprintRadiusTiles: 0.60f, MaxSlope: 40f, DescriptionZh: "石楠灌叢 (Busch)"),
            new("LanKelDor", VegetationLayer.Understory, 45.0, FootprintRadiusTiles: 0.55f, MaxSlope: 42f, DescriptionZh: "荒原荊棘叢 (Dornen)"),

            // 地表草花: 沼澤薹草 (Gra)、濕原苔蘚野花 (Blu)
            new("LanKelGra", VegetationLayer.GroundFlora, 65.0, FootprintRadiusTiles: 0.35f, MaxSlope: 45f, DescriptionZh: "沼澤薹草 (Gras)"),
            new("LanKelBlu", VegetationLayer.GroundFlora, 35.0, FootprintRadiusTiles: 0.35f, MaxSlope: 45f, DescriptionZh: "荒原苔蘚花 (Blumen)"),

            // 水岸生境: 廣布沼澤蘆葦 (Sch)、浮萍 (Ent)、水岸灌柳 (Wei)
            new("LanKelSch", VegetationLayer.Riparian, 50.0, FootprintRadiusTiles: 0.45f, MaxSlope: 18f, DescriptionZh: "泥沼蘆葦 (Schilf)"),
            new("LanKelEnt", VegetationLayer.Riparian, 30.0, FootprintRadiusTiles: 0.40f, MaxSlope: 15f, DescriptionZh: "泥塘浮萍 (Entengrütze)"),
            new("LanKelWei", VegetationLayer.Riparian, 20.0, FootprintRadiusTiles: 0.80f, MaxSlope: 20f, DescriptionZh: "澤地矮柳 (Weide)"),

            // 地質岩石: 荒原巨石 (Ste)、巨石陣式石柱岩 (Fel)
            new("LanKelSte", VegetationLayer.Rock, 50.0, FootprintRadiusTiles: 0.90f, MaxSlope: 90f, DescriptionZh: "荒原玄武岩塊 (Stein)"),
            new("LanKelFel", VegetationLayer.Rock, 50.0, FootprintRadiusTiles: 1.30f, MaxSlope: 90f, DescriptionZh: "巨石陣荒原石柱 (Fels)"),
        }
    };

    /// <summary>取得預設生態圈設定實例。</summary>
    public static BiomeEcologyProfile GetPreset(BiomeType type) => type switch
    {
        BiomeType.GermanicForest => GermanicForest,
        BiomeType.DanubeBroadleaf => DanubeBroadleaf,
        BiomeType.ItalianHills => ItalianHills,
        BiomeType.BritishMarsh => BritishMarsh,
        _ => GermanicForest,
    };

    /// <summary>
    /// 將生態圈物種前綴映射至地圖中實際載入的 <see cref="LevelObjectTemplate"/>。
    /// 包含多層級自動降級與 fallback 備援機制：
    /// 1. 精確前綴符合（例如 "LanGerNad" 匹配 "LanGerNad00_Tanne_gross"）。
    /// 2. 同地區同生態層符合（例如找不到 LanItaZyp 時，以該地圖其他 Ita 樹木代替）。
    /// 3. 全局同生態層符合（若地圖完全無該地區物件，退回任何同 Layer 的景觀物件）。
    /// </summary>
    public ResolvedEcologyRoster ResolveTemplates(
        IReadOnlyDictionary<int, LevelObjectTemplate> templates,
        IReadOnlyDictionary<int, string> objdefNames)
    {
        ArgumentNullException.ThrowIfNull(templates);
        ArgumentNullException.ThrowIfNull(objdefNames);

        var resolvedList = new List<ResolvedSpecies>();

        // 將可用的範本建立索引：(id, template, name, region, category)
        var available = templates
            .Where(kvp => objdefNames.TryGetValue(kvp.Key, out string? name) && ObjDefNames.IsLandscape(name))
            .Select(kvp =>
            {
                string name = objdefNames[kvp.Key];
                string region = name.Length >= 6 ? name.Substring(3, 3) : "";
                string category = ClassifyCategory(name);
                return (Id: kvp.Key, Template: kvp.Value, Name: name, Region: region, Category: category);
            })
            .ToList();

        if (available.Count == 0) return new ResolvedEcologyRoster(Array.Empty<ResolvedSpecies>());

        foreach (BiomeSpeciesEntry entry in Species)
        {
            string categoryCode = LayerToCategoryString(entry.Layer);

            // 1. 精確前綴匹配
            var exactMatches = available
                .Where(item => item.Name.StartsWith(entry.NamePrefix, StringComparison.OrdinalIgnoreCase))
                .ToList();

            if (exactMatches.Count > 0)
            {
                // 將權重均分或賦予匹配項
                double weightPerMatch = entry.Weight / exactMatches.Count;
                foreach (var match in exactMatches)
                {
                    resolvedList.Add(new ResolvedSpecies(
                        match.Template,
                        match.Name,
                        entry.Layer,
                        weightPerMatch,
                        entry.FootprintRadiusTiles,
                        entry.MinElevation,
                        entry.MaxElevation,
                        entry.MaxSlope));
                }
                continue;
            }

            // 2. 退回同地區、同類別匹配
            var regionalMatches = available
                .Where(item => item.Region.Equals(RegionCode, StringComparison.OrdinalIgnoreCase) && item.Category == categoryCode)
                .ToList();

            if (regionalMatches.Count > 0)
            {
                double weightPerMatch = entry.Weight / regionalMatches.Count;
                foreach (var match in regionalMatches)
                {
                    resolvedList.Add(new ResolvedSpecies(
                        match.Template,
                        match.Name,
                        entry.Layer,
                        weightPerMatch,
                        entry.FootprintRadiusTiles,
                        entry.MinElevation,
                        entry.MaxElevation,
                        entry.MaxSlope));
                }
                continue;
            }

            // 3. 退回全局同類別匹配
            var globalMatches = available
                .Where(item => item.Category == categoryCode)
                .ToList();

            if (globalMatches.Count > 0)
            {
                double weightPerMatch = entry.Weight / globalMatches.Count;
                foreach (var match in globalMatches)
                {
                    resolvedList.Add(new ResolvedSpecies(
                        match.Template,
                        match.Name,
                        entry.Layer,
                        weightPerMatch,
                        entry.FootprintRadiusTiles,
                        entry.MinElevation,
                        entry.MaxElevation,
                        entry.MaxSlope));
                }
            }
        }

        // 去除完全重複的 (Template.TypeId, Layer) 項目並合併權重
        var deduped = resolvedList
            .GroupBy(item => (item.Template.TypeId, item.Layer))
            .Select(group =>
            {
                var first = group.First();
                double totalWeight = group.Sum(item => item.Weight);
                return new ResolvedSpecies(
                    first.Template,
                    first.Name,
                    first.Layer,
                    totalWeight,
                    first.FootprintRadiusTiles,
                    first.MinElevation,
                    first.MaxElevation,
                    first.MaxSlope);
            });

        return new ResolvedEcologyRoster(deduped);
    }

    private static string LayerToCategoryString(VegetationLayer layer) => layer switch
    {
        VegetationLayer.Canopy => "tree",
        VegetationLayer.Understory => "bush",
        VegetationLayer.GroundFlora => "grass",
        VegetationLayer.Riparian => "water",
        VegetationLayer.Rock => "rock",
        _ => "other",
    };

    private static string ClassifyCategory(string name)
    {
        string code = name.Length >= 9 ? name.Substring(6, 3) : "";
        return code switch
        {
            "Nad" or "Lau" or "Bau" or "Pal" or "Bir" or "Eic" or "Buc" or "Kie" or "Tan" or "Fic" or "Obs" or "Zyp" or "Pin" => "tree",
            "Gra" or "Bli" or "Blu" => "grass",
            "Dor" or "Bod" or "Bus" or "Str" or "Ger" or "Hec" => "bush",
            "Sch" or "Ent" or "Wei" or "Ser" => "water",
            "Ste" or "Fel" or "Sto" or "Kie" => "rock",
            _ => "other",
        };
    }
}
