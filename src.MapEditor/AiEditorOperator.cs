using System.Text;

namespace AgainstRomeMapEditor;

/// <summary>
/// AI 編輯器操作員：將 AI 地圖特徵（<see cref="AiMapFeature"/>）轉為一連串透過
/// <see cref="EditorBrushExecutor"/> 執行的 <see cref="EditorBrushStroke"/> 筆刷操作。
/// 像人類設計師看著地圖一樣閉環測量並調整筆刷，完全符合編輯器的操作規範。
/// </summary>
internal static class AiEditorOperator
{
    public static AiMapApplyResult Apply(
        AiMapPlan plan,
        TerrainHeightEditSession? heights,
        TerrainBlendEditSession? blend,
        int dimension,
        float waterLevelSample,
        bool commitPerStroke = false,
        Func<string, float, float, float, bool>? paintMaterial = null,
        Action<TerrainTextureChange>? onTexture = null,
        Action<float, float, float>? onRejectedMaterial = null)
    {
        ArgumentNullException.ThrowIfNull(plan);

        int totalHeightSamples = 0;
        int totalCollisionPixels = 0;
        int totalMaterialStrokes = 0;
        int totalRejectedMaterialStrokes = 0;
        var operationLogs = new List<string>();

        AiMapEditScope? editScope = plan.EditScope;
        Func<int, int, bool>? allowsTile = editScope is not null ? editScope.AllowsTile : null;

        EditorBrushResult ExecuteStroke(EditorBrushStroke stroke)
        {
            EditorBrushResult result;
            if (stroke.Tool == EditorBrushTool.Material && blend is null && paintMaterial is not null)
            {
                int matChanges = 0, rej = 0;
                var visited = new HashSet<int>();
                (int X, int Y)? last = null;
                foreach ((int px, int py) in stroke.Path)
                {
                    IEnumerable<(int X, int Y)> tiles = last is { } l ? TerrainStrokePath.Between(l.X, l.Y, px, py) : [(px, py)];
                    last = (px, py);
                    foreach ((int x0, int y0) in tiles)
                    {
                        int x = Math.Clamp(x0, 0, dimension - 1), y = Math.Clamp(y0, 0, dimension - 1);
                        if (!visited.Add(y * dimension + x)) continue;
                        if (allowsTile is not null && !allowsTile(x, y)) continue;
                        totalMaterialStrokes++;
                        bool ok = paintMaterial(stroke.Material!, x + 0.5f, y + 0.5f, stroke.RadiusTiles);
                        if (!ok)
                        {
                            rej++;
                            totalRejectedMaterialStrokes++;
                            onRejectedMaterial?.Invoke(x + 0.5f, y + 0.5f, stroke.RadiusTiles);
                        }
                        else
                        {
                            matChanges++;
                        }
                    }
                }
                result = new EditorBrushResult(0, 0, matChanges, rej, null);
            }
            else
            {
                result = EditorBrushExecutor.Execute(stroke, heights, blend, dimension, waterLevelSample, roughnessSeed: 0, allowsTile: allowsTile, onTexture: onTexture,
                    onRejectedDab: (rx, ry) => onRejectedMaterial?.Invoke(rx + 0.5f, ry + 0.5f, stroke.RadiusTiles));
                if (stroke.Tool == EditorBrushTool.Material)
                {
                    totalMaterialStrokes++;
                    if (result.RejectedMaterialDabs > 0)
                    {
                        totalRejectedMaterialStrokes++;
                    }
                }
            }

            if (commitPerStroke)
            {
                heights?.CommitStroke();
                blend?.CommitStroke();
            }

            totalHeightSamples += result.HeightSamplesChanged;
            totalCollisionPixels += result.CollisionPixelsChanged;

            string command = FormatBrushCommand(stroke);
            string human = FormatHumanDescription(stroke, result);
            operationLogs.Add($"{operationLogs.Count + 1}. {human}\r\n   {command}");

            return result;
        }

        // 1. baseHeight：以區域蛇形筆刷閉環升降接近目標高度，再以整平筆刷拉平
        if (plan.BaseHeight is int baseHeight)
        {
            int target = Math.Clamp(baseHeight, 0, 255);
            int minX = 0, minY = 0, maxX = dimension - 1, maxY = dimension - 1;
            if (editScope is not null)
            {
                minX = Math.Clamp(editScope.Bounds.Left, 0, dimension - 1);
                minY = Math.Clamp(editScope.Bounds.Top, 0, dimension - 1);
                maxX = Math.Clamp(editScope.Bounds.Right - 1, 0, dimension - 1);
                maxY = Math.Clamp(editScope.Bounds.Bottom - 1, 0, dimension - 1);
            }
            int midX = (minX + maxX) / 2, midY = (minY + maxY) / 2;
            (int X, int Y) refPoint = (midX, midY);
            if (allowsTile is not null && !allowsTile(midX, midY))
            {
                for (int y = minY; y <= maxY; y++)
                    for (int x = minX; x <= maxX; x++)
                        if (allowsTile(x, y)) { refPoint = (x, y); goto FoundRef; }
            }
        FoundRef:

            for (int iter = 0; iter < 40; iter++)
            {
                int curr = GetCenterHeight(heights, refPoint.X, refPoint.Y, dimension);
                int diff = target - curr;
                if (Math.Abs(diff) <= 1) break;
                int strength = Math.Abs(diff) >= 10 ? 2 : (Math.Abs(diff) >= 4 ? 1 : 0);
                var op = diff > 0 ? EditorHeightBrush.Raise : EditorHeightBrush.Lower;
                var stroke = new EditorBrushStroke(EditorBrushTool.Height, [(refPoint.X, refPoint.Y)], Size: 15, StrengthLevel: strength, HeightOperation: op);
                var res = ExecuteStroke(stroke);
                if (res.HeightSamplesChanged == 0) break;
            }

            var flattenPath = new List<(int X, int Y)> { (refPoint.X, refPoint.Y) };
            flattenPath.AddRange(GenerateSerpentinePath(minX, minY, maxX, maxY, step: 6));
            var flattenStroke = new EditorBrushStroke(EditorBrushTool.Height, flattenPath, Size: 15, StrengthLevel: 2, HeightOperation: EditorHeightBrush.Flatten);
            ExecuteStroke(flattenStroke);
            ExecuteStroke(flattenStroke);
        }

        // 2. baseMaterial：以蛇形筆刷塗抹底層材質
        if (plan.BaseMaterial is string baseMat)
        {
            int minX = 0, minY = 0, maxX = dimension - 1, maxY = dimension - 1;
            if (editScope is not null)
            {
                minX = Math.Clamp(editScope.Bounds.Left, 0, dimension - 1);
                minY = Math.Clamp(editScope.Bounds.Top, 0, dimension - 1);
                maxX = Math.Clamp(editScope.Bounds.Right - 1, 0, dimension - 1);
                maxY = Math.Clamp(editScope.Bounds.Bottom - 1, 0, dimension - 1);
            }
            // 整區換底色與玩家相同：使用編輯器「區域工具 → 矩形填色」（一次烘焙），逐點筆刷會在途中產生原版沒有的過渡而被拒絕。
            if (blend is not null)
            {
                var cells = TerrainRegionPlanner.Rectangle(dimension, (minX, minY), (maxX, maxY))
                    .Where(cell => allowsTile is null || allowsTile(cell.X, cell.Y)).ToArray();
                TerrainBlendPaintResult fill = blend.PaintTiles(cells, baseMat);
                totalMaterialStrokes++;
                if (!fill.Succeeded)
                {
                    totalRejectedMaterialStrokes++;
                    foreach (var issue in fill.Issues) onRejectedMaterial?.Invoke(issue.TileX + 0.5f, issue.TileY + 0.5f, 0.5f);
                }
                foreach (TerrainTextureChange change in fill.TextureChanges) onTexture?.Invoke(change);
                if (commitPerStroke) blend.CommitStroke();
                operationLogs.Add($"{operationLogs.Count + 1}. 區域工具：矩形填色 ({minX},{minY})–({maxX},{maxY})，材質 {baseMat}（{(fill.Succeeded ? $"{fill.TextureChanges.Count} 格" : "缺少原版接縫，未修改")}）");
            }
            else if (paintMaterial is not null)
            {
                ExecuteStroke(new EditorBrushStroke(EditorBrushTool.Material, GenerateSerpentinePath(minX, minY, maxX, maxY, step: 6), Size: 15, Material: baseMat));
            }
        }

        // 3. 各地貌特徵
        foreach (AiMapFeature feature in plan.Features)
        {
            switch (feature.Type)
            {
                case "hill":
                case "mountain":
                case "valley":
                {
                    bool isValley = feature.Type == "valley";
                    int defaultAmount = feature.Type == "mountain" ? 110 : feature.Type == "hill" ? 45 : 40;
                    int amount = feature.Amount ?? defaultAmount;
                    int cx = feature.X, cy = feature.Y;
                    int h0 = GetCenterHeight(heights, cx, cy, dimension);
                    int targetGain = amount;

                    for (int iter = 0; iter < 40; iter++)
                    {
                        int curr = GetCenterHeight(heights, cx, cy, dimension);
                        int currentGain = isValley ? (h0 - curr) : (curr - h0);
                        if (currentGain >= targetGain) break;
                        int remaining = targetGain - currentGain;
                        float progress = Math.Clamp((float)currentGain / Math.Max(1, targetGain), 0f, 1f);
                        int currentRadius = Math.Max(1, (int)Math.Round(feature.Radius * (1f - progress * 0.75f)));
                        int brushSize = ChooseBrushSize(currentRadius);
                        int strength = remaining >= 10 ? 2 : (remaining >= 4 ? 1 : 0);

                        var op = isValley ? EditorHeightBrush.Lower : EditorHeightBrush.Raise;
                        var stroke = new EditorBrushStroke(EditorBrushTool.Height, [(cx, cy)], Size: brushSize, StrengthLevel: strength, HeightOperation: op);
                        var res = ExecuteStroke(stroke);
                        if (res.HeightSamplesChanged == 0) break;
                    }

                    // 錐形塑形後進行一次平滑筆刷
                    int smoothSize = ChooseBrushSize(feature.Radius);
                    var smoothStroke = new EditorBrushStroke(EditorBrushTool.Height, [(cx, cy)], Size: smoothSize, StrengthLevel: 0, HeightOperation: EditorHeightBrush.Smooth);
                    ExecuteStroke(smoothStroke);
                    break;
                }

                case "ridge":
                {
                    int amount = feature.Amount ?? 70;
                    int x1 = feature.X, y1 = feature.Y;
                    int x2 = feature.X2 ?? x1, y2 = feature.Y2 ?? y1;
                    int mx = (x1 + x2) / 2, my = (y1 + y2) / 2;
                    int h0 = GetCenterHeight(heights, mx, my, dimension);
                    int brushSize = ChooseBrushSize(feature.Radius);
                    var path = new (int X, int Y)[] { (x1, y1), (x2, y2) };

                    for (int iter = 0; iter < 40; iter++)
                    {
                        int curr = GetCenterHeight(heights, mx, my, dimension);
                        int gained = curr - h0;
                        if (gained >= amount) break;
                        int remaining = amount - gained;
                        int strength = remaining >= 10 ? 2 : (remaining >= 4 ? 1 : 0);
                        var stroke = new EditorBrushStroke(EditorBrushTool.Height, path, Size: brushSize, StrengthLevel: strength, HeightOperation: EditorHeightBrush.Raise);
                        var res = ExecuteStroke(stroke);
                        if (res.HeightSamplesChanged == 0) break;
                    }

                    var smoothStroke = new EditorBrushStroke(EditorBrushTool.Height, path, Size: brushSize, StrengthLevel: 0, HeightOperation: EditorHeightBrush.Smooth);
                    ExecuteStroke(smoothStroke);
                    break;
                }

                case "plateau":
                {
                    int target = feature.Amount ?? 150;
                    int cx = feature.X, cy = feature.Y;
                    int brushSize = ChooseBrushSize(feature.Radius);

                    for (int iter = 0; iter < 40; iter++)
                    {
                        int curr = GetCenterHeight(heights, cx, cy, dimension);
                        int diff = target - curr;
                        if (Math.Abs(diff) <= 1) break;
                        int strength = Math.Abs(diff) >= 10 ? 2 : (Math.Abs(diff) >= 4 ? 1 : 0);
                        var op = diff > 0 ? EditorHeightBrush.Raise : EditorHeightBrush.Lower;

                        var stroke = new EditorBrushStroke(EditorBrushTool.Height, [(cx, cy)], Size: brushSize, StrengthLevel: strength, HeightOperation: op);
                        var res = ExecuteStroke(stroke);
                        if (res.HeightSamplesChanged == 0) break;
                    }

                    var flattenList = new List<(int X, int Y)> { (cx, cy) };
                    if (feature.Radius > brushSize / 2 + 1)
                    {
                        flattenList.AddRange(GenerateCircleSerpentinePath(cx, cy, feature.Radius, Math.Max(1, (int)(brushSize * 0.75f)), dimension));
                    }
                    var flattenStroke = new EditorBrushStroke(EditorBrushTool.Height, flattenList, Size: brushSize, StrengthLevel: 2, HeightOperation: EditorHeightBrush.Flatten);
                    ExecuteStroke(flattenStroke);
                    ExecuteStroke(flattenStroke);
                    break;
                }

                case "lake":
                {
                    int cx = feature.X, cy = feature.Y;
                    int brushSize = ChooseBrushSize(feature.Radius);
                    var path = new List<(int X, int Y)> { (cx, cy) };
                    if (feature.Radius > brushSize / 2 + 1)
                    {
                        path.AddRange(GenerateCircleSerpentinePath(cx, cy, feature.Radius, Math.Max(1, (int)(brushSize * 0.75f)), dimension));
                    }
                    var stroke = new EditorBrushStroke(EditorBrushTool.Height, path, Size: brushSize, StrengthLevel: 2, HeightOperation: EditorHeightBrush.Water);
                    ExecuteStroke(stroke);
                    ExecuteStroke(stroke);
                    break;
                }

                case "river":
                {
                    int brushSize = ChooseBrushSize(feature.Radius);
                    int x1 = feature.X, y1 = feature.Y;
                    int x2 = feature.X2 ?? x1, y2 = feature.Y2 ?? y1;
                    var path = new (int X, int Y)[] { (x1, y1), (x2, y2) };
                    var stroke = new EditorBrushStroke(EditorBrushTool.Height, path, Size: brushSize, StrengthLevel: 2, HeightOperation: EditorHeightBrush.Water);
                    ExecuteStroke(stroke);
                    ExecuteStroke(stroke);
                    break;
                }

                case "material":
                {
                    if (feature.Material is not null)
                    {
                        int brushSize = ChooseBrushSize(feature.Radius);
                        var path = new List<(int X, int Y)> { (feature.X, feature.Y) };
                        if (feature.Radius > brushSize / 2 + 1)
                        {
                            path.AddRange(GenerateCircleSerpentinePath(feature.X, feature.Y, feature.Radius, Math.Max(1, (int)(brushSize * 0.75f)), dimension));
                        }
                        var stroke = new EditorBrushStroke(EditorBrushTool.Material, path, Size: brushSize, Material: feature.Material);
                        ExecuteStroke(stroke);
                    }
                    break;
                }

                case "blocked":
                case "passable":
                {
                    if (plan.EditScope is null || plan.EditScope.EditPassability)
                    {
                        bool block = feature.Type == "blocked";
                        int brushSize = ChooseBrushSize(feature.Radius);
                        IReadOnlyList<(int X, int Y)> path;
                        if (feature.X2 is int x2 && feature.Y2 is int y2)
                        {
                            path = [(feature.X, feature.Y), (x2, y2)];
                        }
                        else if (feature.Radius <= brushSize / 2 + 1)
                        {
                            path = [(feature.X, feature.Y)];
                        }
                        else
                        {
                            path = GenerateCircleSerpentinePath(feature.X, feature.Y, feature.Radius, Math.Max(1, (int)(brushSize * 0.75f)), dimension);
                        }
                        var stroke = new EditorBrushStroke(EditorBrushTool.Collision, path, Size: brushSize, Block: block);
                        ExecuteStroke(stroke);
                    }
                    break;
                }
            }

            // 特徵附加材質
            if (feature.Material is not null && feature.Type != "material")
            {
                bool linear = feature.Type is "river" or "ridge" or "blocked" or "passable";
                if (linear && feature.X2 is int x2 && feature.Y2 is int y2)
                {
                    int brushSize = ChooseBrushSize(feature.Radius + 1);
                    var path = new (int X, int Y)[] { (feature.X, feature.Y), (x2, y2) };
                    var stroke = new EditorBrushStroke(EditorBrushTool.Material, path, Size: brushSize, Material: feature.Material);
                    ExecuteStroke(stroke);
                }
                else
                {
                    int brushSize = ChooseBrushSize(feature.Radius);
                    var path = new List<(int X, int Y)> { (feature.X, feature.Y) };
                    if (feature.Radius > brushSize / 2 + 1)
                    {
                        path.AddRange(GenerateCircleSerpentinePath(feature.X, feature.Y, feature.Radius, Math.Max(1, (int)(brushSize * 0.75f)), dimension));
                    }
                    var stroke = new EditorBrushStroke(EditorBrushTool.Material, path, Size: brushSize, Material: feature.Material);
                    ExecuteStroke(stroke);
                }
            }
        }

        return new AiMapApplyResult(totalHeightSamples, totalMaterialStrokes, totalRejectedMaterialStrokes, totalCollisionPixels, operationLogs);
    }

    public static int ChooseBrushSize(int radius)
    {
        if (radius >= 7) return 15;
        if (radius >= 4) return 9;
        if (radius >= 2) return 5;
        if (radius >= 1) return 3;
        return 1;
    }

    public static List<(int X, int Y)> GenerateSerpentinePath(int minX, int minY, int maxX, int maxY, int step)
    {
        var path = new List<(int X, int Y)>();
        step = Math.Max(1, step);
        bool leftToRight = true;
        for (int y = minY; y <= maxY; y += step)
        {
            if (minX == maxX)
            {
                path.Add((minX, y));
            }
            else if (leftToRight)
            {
                path.Add((minX, y));
                path.Add((maxX, y));
            }
            else
            {
                path.Add((maxX, y));
                path.Add((minX, y));
            }
            leftToRight = !leftToRight;
        }
        if (path.Count == 0)
            path.Add((minX, minY));
        return path;
    }

    public static List<(int X, int Y)> GenerateCircleSerpentinePath(int cx, int cy, int radius, int step, int dimension)
    {
        var path = new List<(int X, int Y)>();
        step = Math.Max(1, step);
        // 確保 cy (中心行) 一定被納入
        var yList = new List<int> { cy };
        for (int y = cy - step; y >= Math.Max(0, cy - radius); y -= step) yList.Add(y);
        for (int y = cy + step; y <= Math.Min(dimension - 1, cy + radius); y += step) yList.Add(y);
        yList.Sort();

        bool leftToRight = true;
        foreach (int y in yList)
        {
            int dy = Math.Abs(y - cy);
            int dx = (int)MathF.Round(MathF.Sqrt(Math.Max(0, radius * radius - dy * dy)));
            int minX = Math.Max(0, cx - dx);
            int maxX = Math.Min(dimension - 1, cx + dx);
            if (minX <= maxX)
            {
                if (minX == maxX)
                {
                    path.Add((minX, y));
                }
                else if (leftToRight)
                {
                    path.Add((minX, y));
                    path.Add((maxX, y));
                }
                else
                {
                    path.Add((maxX, y));
                    path.Add((minX, y));
                }
                leftToRight = !leftToRight;
            }
        }
        if (path.Count == 0)
            path.Add((Math.Clamp(cx, 0, dimension - 1), Math.Clamp(cy, 0, dimension - 1)));
        return path;
    }

    internal static int GetCenterHeight(TerrainHeightEditSession? heights, int tileX, int tileY, int dimension)
    {
        if (heights is null) return 0;
        float step = (heights.VertexSize - 1) / (float)dimension;
        int vx = Math.Clamp((int)MathF.Round((tileX + 0.5f) * step), 0, heights.VertexSize - 1);
        int vy = Math.Clamp((int)MathF.Round((tileY + 0.5f) * step), 0, heights.VertexSize - 1);
        return heights.Heights[vy * heights.VertexSize + vx];
    }

    internal static string FormatBrushCommand(EditorBrushStroke stroke)
    {
        var sb = new StringBuilder();
        sb.Append("/brush ");
        switch (stroke.Tool)
        {
            case EditorBrushTool.Height:
                string op = stroke.HeightOperation switch
                {
                    EditorHeightBrush.Raise => "raise",
                    EditorHeightBrush.Lower => "lower",
                    EditorHeightBrush.Smooth => "smooth",
                    EditorHeightBrush.Flatten => "flatten",
                    EditorHeightBrush.Roughen => "roughen",
                    EditorHeightBrush.Water => "water",
                    _ => "raise"
                };
                sb.Append("height ").Append(op);
                break;
            case EditorBrushTool.Collision:
                sb.Append("collision ").Append(stroke.Block ? "block" : "clear");
                break;
            case EditorBrushTool.Material:
                sb.Append("material ").Append(stroke.Material ?? "default");
                break;
        }

        foreach (var pt in stroke.Path)
        {
            sb.Append(' ').Append(pt.X).Append(',').Append(pt.Y);
        }

        sb.Append(" --size ").Append(stroke.Size);

        if (stroke.Tool == EditorBrushTool.Height)
        {
            string strength = stroke.StrengthLevel switch
            {
                0 => "light",
                2 => "strong",
                _ => "medium"
            };
            sb.Append(" --strength ").Append(strength);
        }

        if (stroke.AutoBridge)
        {
            sb.Append(" --auto-bridge");
        }

        return sb.ToString();
    }

    internal static string FormatHumanDescription(EditorBrushStroke stroke, EditorBrushResult result)
    {
        string loc = stroke.Path.Count switch
        {
            0 => "無路徑",
            1 => $"({stroke.Path[0].X}, {stroke.Path[0].Y})",
            2 => $"({stroke.Path[0].X}, {stroke.Path[0].Y}) → ({stroke.Path[1].X}, {stroke.Path[1].Y})",
            _ => $"({stroke.Path[0].X}, {stroke.Path[0].Y}) ... 共 {stroke.Path.Count} 點"
        };

        return stroke.Tool switch
        {
            EditorBrushTool.Height => stroke.HeightOperation switch
            {
                EditorHeightBrush.Raise => $"提高地形：位置 {loc}，大小 {stroke.Size}，強度 {(stroke.StrengthLevel == 2 ? "強" : stroke.StrengthLevel == 0 ? "輕" : "中")}（變更 {result.HeightSamplesChanged} 個高度頂點）",
                EditorHeightBrush.Lower => $"降低地形：位置 {loc}，大小 {stroke.Size}，強度 {(stroke.StrengthLevel == 2 ? "強" : stroke.StrengthLevel == 0 ? "輕" : "中")}（變更 {result.HeightSamplesChanged} 個高度頂點）",
                EditorHeightBrush.Smooth => $"平滑地形：位置 {loc}，大小 {stroke.Size}（變更 {result.HeightSamplesChanged} 個高度頂點）",
                EditorHeightBrush.Flatten => $"整平地形：位置 {loc}，大小 {stroke.Size}（變更 {result.HeightSamplesChanged} 個高度頂點）",
                EditorHeightBrush.Roughen => $"粗糙化地形：位置 {loc}，大小 {stroke.Size}（變更 {result.HeightSamplesChanged} 個高度頂點）",
                EditorHeightBrush.Water => $"水域下挖：位置 {loc}，大小 {stroke.Size}（變更 {result.HeightSamplesChanged} 個高度頂點，{result.CollisionPixelsChanged} 個通行像素）",
                _ => $"高度操作：位置 {loc}，大小 {stroke.Size}"
            },
            EditorBrushTool.Collision => $"通行設定（{(stroke.Block ? "阻擋" : "通行")}）：路徑 {loc}，大小 {stroke.Size}（變更 {result.CollisionPixelsChanged} 個通行像素）",
            EditorBrushTool.Material => $"塗抹材質 [{stroke.Material}]：位置 {loc}，大小 {stroke.Size}（變更 {result.MaterialTilesChanged} 格材質）",
            _ => $"筆刷操作：位置 {loc}"
        };
    }
}
