using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor.Modules.WildLair;

/// <summary>
/// 動態資源再生規劃器（ResourceRegenerationPlanner）：
/// 基於二維細胞自動機（Cellular Automata）之森林生態演替模擬器。
/// 支援樹木採伐後殘樁標記、種子擴散、樹苗發芽、幼木生長、自疏稀釋與原始林冠演替。
/// </summary>
public sealed class ResourceRegenerationPlanner
{
    private readonly int _dimension;
    private readonly float _tileWorldSize;
    private ForestGridCell[,] _grid;
    private readonly Dictionary<(int X, int Y), StumpHarvestRecord> _activeStumps = new();
    private readonly Random _random;

    public int Dimension => _dimension;
    public float TileWorldSize => _tileWorldSize;
    public IReadOnlyCollection<StumpHarvestRecord> ActiveStumps => _activeStumps.Values;

    public ResourceRegenerationPlanner(
        int dimension,
        float tileWorldSize = 64.0f,
        int seed = 1337)
    {
        _dimension = dimension > 0 ? dimension : throw new ArgumentOutOfRangeException(nameof(dimension));
        _tileWorldSize = tileWorldSize > 0 ? tileWorldSize : 64.0f;
        _random = new Random(seed);
        _grid = new ForestGridCell[_dimension, _dimension];

        // 預設填入 Barren 空地
        for (int y = 0; y < _dimension; y++)
        for (int x = 0; x < _dimension; x++)
        {
            _grid[x, y] = new ForestGridCell(x, y, ForestCellState.Barren, Fertility: 0.5f, Moisture: 0.5f);
        }
    }

    /// <summary>
    /// 從現有地圖自然物件、水域與地貌坡度初始化森林生態網格。
    /// </summary>
    public void InitializeFromMap(
        IEnumerable<(float WorldX, float WorldZ, string Species)> existingTrees,
        Func<int, int, bool>? isWater = null,
        Func<int, int, float>? getSlope = null,
        Func<int, int, float>? getMoisture = null)
    {
        ArgumentNullException.ThrowIfNull(existingTrees);

        // 1. 初始化水體與地貌屬性
        for (int y = 0; y < _dimension; y++)
        for (int x = 0; x < _dimension; x++)
        {
            bool water = isWater?.Invoke(x, y) ?? false;
            float slope = getSlope?.Invoke(x, y) ?? 0f;
            float moisture = getMoisture?.Invoke(x, y) ?? 0.5f;

            ForestCellState state = ForestCellState.Barren;
            if (water) state = ForestCellState.Water;
            else if (slope > 0.40f) state = ForestCellState.Obstacle; // 峭壁不可生長

            _grid[x, y] = new ForestGridCell(
                x, y,
                state,
                Fertility: state == ForestCellState.Barren ? 0.6f : 0f,
                Moisture: Math.Clamp(moisture, 0f, 1f));
        }

        // 2. 標記既有成熟林木
        foreach (var (wx, wz, species) in existingTrees)
        {
            int tx = (int)MathF.Floor(wx / _tileWorldSize);
            int ty = (int)MathF.Floor(wz / _tileWorldSize);

            if (tx >= 0 && tx < _dimension && ty >= 0 && ty < _dimension)
            {
                if (_grid[tx, ty].State != ForestCellState.Water && _grid[tx, ty].State != ForestCellState.Obstacle)
                {
                    _grid[tx, ty] = _grid[tx, ty] with
                    {
                        State = ForestCellState.MatureTree,
                        TreeSpecies = string.IsNullOrWhiteSpace(species) ? "LanGerTanne01" : species,
                        AgeTicks = 10
                    };
                }
            }
        }
    }

    /// <summary>
    /// 取得指定格點之生態細胞狀態。
    /// </summary>
    public ForestGridCell GetCell(int x, int y)
    {
        if (x < 0 || x >= _dimension || y < 0 || y >= _dimension)
            throw new ArgumentOutOfRangeException($"座標 ({x}, {y}) 超出地圖範圍 [0, {_dimension})。");
        return _grid[x, y];
    }

    /// <summary>
    /// 標記一株樹木被砍伐後之殘樁（Stump）。
    /// 紀錄殘樁剩餘腐朽計時，並阻擋該格重新立即生長樹木。
    /// </summary>
    public void MarkHarvestedStump(
        int tileX,
        int tileY,
        string species = "LanGerTanne01",
        int slotIndex = -1,
        float worldX = -1f,
        float worldZ = -1f)
    {
        if (tileX < 0 || tileX >= _dimension || tileY < 0 || tileY >= _dimension) return;

        float wx = worldX >= 0f ? worldX : (tileX + 0.5f) * _tileWorldSize;
        float wz = worldZ >= 0f ? worldZ : (tileY + 0.5f) * _tileWorldSize;

        var record = new StumpHarvestRecord(tileX, tileY, wx, wz, species, slotIndex);
        _activeStumps[(tileX, tileY)] = record;

        _grid[tileX, tileY] = _grid[tileX, tileY] with
        {
            State = ForestCellState.Stump,
            TreeSpecies = species,
            AgeTicks = 0
        };
    }

    /// <summary>
    /// 執行單步或多步 Cellular Automata 森林動態再生模擬迭代。
    /// </summary>
    public void StepSimulation(int steps = 1, RegenerationParameters? parameters = null)
    {
        if (steps <= 0) return;
        var config = parameters ?? new RegenerationParameters();

        for (int s = 0; s < steps; s++)
        {
            ForestGridCell[,] next = new ForestGridCell[_dimension, _dimension];

            for (int y = 0; y < _dimension; y++)
            for (int x = 0; x < _dimension; x++)
            {
                ForestGridCell current = _grid[x, y];

                switch (current.State)
                {
                    case ForestCellState.Water:
                    case ForestCellState.Obstacle:
                        next[x, y] = current;
                        break;

                    case ForestCellState.Stump:
                        int newStumpAge = current.AgeTicks + 1;
                        if (newStumpAge >= config.StumpDecayTicks)
                        {
                            // 殘樁完全腐朽風化，轉化為養分豐富之開闊土壤
                            _activeStumps.Remove((x, y));
                            next[x, y] = current with
                            {
                                State = ForestCellState.Barren,
                                Fertility = Math.Min(1.0f, current.Fertility + 0.35f),
                                AgeTicks = 0
                            };
                        }
                        else
                        {
                            next[x, y] = current with { AgeTicks = newStumpAge };
                            if (_activeStumps.TryGetValue((x, y), out var rec))
                            {
                                _activeStumps[(x, y)] = rec with { RemainingDecayTicks = config.StumpDecayTicks - newStumpAge };
                            }
                        }
                        break;

                    case ForestCellState.Barren:
                        // 檢查周圍 Moore 8 鄰域之種子擴散
                        var (matureCount, ancientCount, dominantSpecies) = CountParentCanopyNeighbors(x, y);
                        if (matureCount > 0 || ancientCount > 0)
                        {
                            float pBase = config.SeedDispersalProbability;
                            float pAncient = pBase * config.AncientSeedDispersalBonus;

                            // 結合鄰近母樹散播機率
                            float notDispersed = MathF.Pow(1.0f - pBase, matureCount) * MathF.Pow(Math.Max(0f, 1.0f - pAncient), ancientCount);
                            float pDispersal = 1.0f - notDispersed;

                            // 肥沃度與水源加成權重
                            float pEffective = Math.Clamp(
                                pDispersal * current.Fertility * (1.0f + (current.Moisture - 0.5f) * (config.MoistureGrowthBonus - 1.0f)),
                                0.0f,
                                1.0f);

                            if (_random.NextDouble() < pEffective)
                            {
                                next[x, y] = current with
                                {
                                    State = ForestCellState.Sapling,
                                    TreeSpecies = dominantSpecies ?? current.TreeSpecies,
                                    AgeTicks = 0
                                };
                                break;
                            }
                        }
                        next[x, y] = current with { AgeTicks = current.AgeTicks + 1 };
                        break;

                    case ForestCellState.Sapling:
                        int saplingAge = current.AgeTicks + 1;
                        if (saplingAge >= config.SaplingToYoungTicks)
                        {
                            next[x, y] = current with
                            {
                                State = ForestCellState.YoungTree,
                                AgeTicks = 0
                            };
                        }
                        else
                        {
                            next[x, y] = current with { AgeTicks = saplingAge };
                        }
                        break;

                    case ForestCellState.YoungTree:
                        int youngAge = current.AgeTicks + 1;
                        if (youngAge >= config.YoungToMatureTicks)
                        {
                            next[x, y] = current with
                            {
                                State = ForestCellState.MatureTree,
                                AgeTicks = 0
                            };
                        }
                        else
                        {
                            next[x, y] = current with { AgeTicks = youngAge };
                        }
                        break;

                    case ForestCellState.MatureTree:
                        // 森林自疏機制（Competition & Thinning）：過度擁擠時部分樹木衰亡
                        var (nMat, nAnc, _) = CountParentCanopyNeighbors(x, y);
                        int totalCrowding = nMat + nAnc;

                        if (totalCrowding >= 7 && _random.NextDouble() < config.CrowdingMortalityRate)
                        {
                            // 自然枯萎倒伏為風化殘樁
                            next[x, y] = current with
                            {
                                State = ForestCellState.Stump,
                                AgeTicks = 0
                            };
                            _activeStumps[(x, y)] = new StumpHarvestRecord(x, y, (x + 0.5f) * _tileWorldSize, (y + 0.5f) * _tileWorldSize, current.TreeSpecies);
                            break;
                        }

                        int matureAge = current.AgeTicks + 1;
                        if (matureAge >= config.MatureToAncientTicks)
                        {
                            next[x, y] = current with
                            {
                                State = ForestCellState.AncientCanopy,
                                AgeTicks = 0
                            };
                        }
                        else
                        {
                            next[x, y] = current with { AgeTicks = matureAge };
                        }
                        break;

                    case ForestCellState.AncientCanopy:
                        next[x, y] = current with { AgeTicks = current.AgeTicks + 1 };
                        break;
                }
            }

            _grid = next;
        }
    }

    /// <summary>
    /// 計算周邊 Moore 八鄰域成熟喬木與原始巨木老林數量與優勢樹種。
    /// </summary>
    private (int MatureCount, int AncientCount, string? DominantSpecies) CountParentCanopyNeighbors(int cx, int cy)
    {
        int mature = 0;
        int ancient = 0;
        Dictionary<string, int> speciesCounts = new(StringComparer.OrdinalIgnoreCase);

        for (int dy = -1; dy <= 1; dy++)
        for (int dx = -1; dx <= 1; dx++)
        {
            if (dx == 0 && dy == 0) continue;
            int nx = cx + dx;
            int ny = cy + dy;

            if (nx >= 0 && nx < _dimension && ny >= 0 && ny < _dimension)
            {
                ForestGridCell neighbor = _grid[nx, ny];
                if (neighbor.State == ForestCellState.MatureTree)
                {
                    mature++;
                    speciesCounts[neighbor.TreeSpecies] = speciesCounts.GetValueOrDefault(neighbor.TreeSpecies) + 1;
                }
                else if (neighbor.State == ForestCellState.AncientCanopy)
                {
                    ancient++;
                    speciesCounts[neighbor.TreeSpecies] = speciesCounts.GetValueOrDefault(neighbor.TreeSpecies) + 2;
                }
            }
        }

        string? dominant = speciesCounts.Count > 0
            ? speciesCounts.MaxBy(kv => kv.Value).Key
            : null;

        return (mature, ancient, dominant);
    }

    /// <summary>
    /// 統計當前森林生態網格各類狀態分佈與林冠覆蓋率。
    /// </summary>
    public ForestEcologyStatistics GetStatistics()
    {
        int barren = 0, stump = 0, sapling = 0, young = 0, mature = 0, ancient = 0, waterOrObs = 0;
        int total = _dimension * _dimension;

        for (int y = 0; y < _dimension; y++)
        for (int x = 0; x < _dimension; x++)
        {
            switch (_grid[x, y].State)
            {
                case ForestCellState.Barren: barren++; break;
                case ForestCellState.Stump: stump++; break;
                case ForestCellState.Sapling: sapling++; break;
                case ForestCellState.YoungTree: young++; break;
                case ForestCellState.MatureTree: mature++; break;
                case ForestCellState.AncientCanopy: ancient++; break;
                case ForestCellState.Water:
                case ForestCellState.Obstacle: waterOrObs++; break;
            }
        }

        int growable = total - waterOrObs;
        float coverage = growable > 0 ? (float)(mature + ancient) / growable * 100.0f : 0f;

        return new ForestEcologyStatistics(
            total,
            barren,
            stump,
            sapling,
            young,
            mature,
            ancient,
            waterOrObs,
            coverage);
    }

    /// <summary>
    /// 將再生完成的成樹與巨木導出為地圖編輯器 <see cref="NatureAddition"/> 清單，
    /// 可直接透過 <see cref="NatureEditSession.PlantMany"/> 一鍵提交至編輯器變更歷史。
    /// </summary>
    public IReadOnlyList<NatureAddition> ExportRegeneratedNature(
        Func<string, LevelObjectTemplate?> templateResolver,
        Func<float, float, float>? sampleHeight = null)
    {
        ArgumentNullException.ThrowIfNull(templateResolver);

        var additions = new List<NatureAddition>();

        for (int y = 0; y < _dimension; y++)
        for (int x = 0; x < _dimension; x++)
        {
            ForestGridCell cell = _grid[x, y];
            if (cell.State is ForestCellState.MatureTree or ForestCellState.AncientCanopy)
            {
                LevelObjectTemplate? template = templateResolver(cell.TreeSpecies);
                if (template is null) continue;

                // 格內微幅抖動偏移，避免排列整齊呆板
                float jitterX = ((float)_random.NextDouble() - 0.5f) * 0.4f * _tileWorldSize;
                float jitterZ = ((float)_random.NextDouble() - 0.5f) * 0.4f * _tileWorldSize;
                float worldX = (x + 0.5f) * _tileWorldSize + jitterX;
                float worldZ = (y + 0.5f) * _tileWorldSize + jitterZ;
                float worldY = sampleHeight?.Invoke(worldX, worldZ) ?? 0f;
                float rotation = (float)_random.NextDouble() * 360f;

                additions.Add(new NatureAddition(template, cell.TreeSpecies, worldX, worldY, worldZ, rotation));
            }
        }

        return additions;
    }
}
