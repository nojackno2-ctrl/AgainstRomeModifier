namespace AgainstRomeMapEditor.Modules.Fortification;

/// <summary>
/// 防禦工事原生物件目錄管理器。
/// 分類直牆、拐角、T型牆、十字接頭、城門與防禦塔底座的原生物件與圖元定義，
/// 並依據文化風格（羅馬、日耳曼、凱爾特、匈人）提供自動拓撲匹配與容錯回退。
/// </summary>
public sealed class WallTileCatalog
{
    private readonly Dictionary<FortificationStyle, List<WallComponentDefinition>> _definitionsByStyle = new();

    public WallTileCatalog()
    {
        RegisterDefaults();
    }

    /// <summary>
    /// 註冊自訂或擴充防禦工事元件。
    /// </summary>
    public void Register(FortificationStyle style, WallComponentDefinition definition)
    {
        ArgumentNullException.ThrowIfNull(definition);
        if (!_definitionsByStyle.TryGetValue(style, out var list))
        {
            list = new List<WallComponentDefinition>();
            _definitionsByStyle[style] = list;
        }
        list.Add(definition);
    }

    /// <summary>
    /// 取得指定風格下所有已註冊元件。
    /// </summary>
    public IReadOnlyList<WallComponentDefinition> GetComponents(FortificationStyle style)
    {
        return _definitionsByStyle.TryGetValue(style, out var list) ? list : Array.Empty<WallComponentDefinition>();
    }

    /// <summary>
    /// 依據連通遮罩與偏好，為拓撲節點解析最適之防禦元件與旋轉角。
    /// </summary>
    public (WallComponentDefinition Definition, float AngleDeg) ResolveComponent(
        FortificationStyle style,
        WallConnections connections,
        bool preferTowerOnCorner = true)
    {
        var components = GetComponents(style);

        // 1. 判斷度數與拓撲型態
        int degree = CountConnections(connections);

        if (degree == 0)
        {
            // 孤立樁 / 哨所
            var post = components.FirstOrDefault(c => c.Kind == WallComponentKind.EndCap)
                      ?? components.FirstOrDefault(c => c.Kind == WallComponentKind.Tower)
                      ?? components[0];
            return (post, post.DefaultAngleDeg);
        }

        if (degree == 1)
        {
            // 單端點：以 EndCap 或直牆封閉收尾
            var endCap = components.FirstOrDefault(c => c.Kind == WallComponentKind.EndCap);
            float angle = EndCapAngle(connections);
            if (endCap is not null) return (endCap, angle);

            // 回退到直牆
            var straight = components.FirstOrDefault(c => c.Kind == WallComponentKind.Straight);
            if (straight is not null)
            {
                bool isVertical = connections.HasFlag(WallConnections.North) || connections.HasFlag(WallConnections.South);
                return (straight, isVertical ? 90f : 0f);
            }
        }

        if (degree == 2)
        {
            // 直線：南北或東西
            if (connections == (WallConnections.North | WallConnections.South))
            {
                var straight = components.FirstOrDefault(c => c.Kind == WallComponentKind.Straight);
                if (straight is not null) return (straight, 90f);
            }
            if (connections == (WallConnections.East | WallConnections.West))
            {
                var straight = components.FirstOrDefault(c => c.Kind == WallComponentKind.Straight);
                if (straight is not null) return (straight, 0f);
            }

            // 轉角：NE, ES, SW, WN
            if (preferTowerOnCorner)
            {
                var cornerTower = components.FirstOrDefault(c => c.Kind == WallComponentKind.Tower);
                if (cornerTower is not null) return (cornerTower, cornerTower.DefaultAngleDeg);
            }

            var corner = components.FirstOrDefault(c => c.Kind == WallComponentKind.Corner);
            if (corner is not null)
            {
                float cornerAngle = CornerAngle(connections);
                return (corner, cornerAngle);
            }
        }

        if (degree == 3)
        {
            // T型接頭：若無特定 T型牆，升級為防禦塔作為強固樞紐
            var tJunction = components.FirstOrDefault(c => c.Kind == WallComponentKind.TJunction);
            if (tJunction is not null)
            {
                float tAngle = TJunctionAngle(connections);
                return (tJunction, tAngle);
            }

            var tower = components.FirstOrDefault(c => c.Kind == WallComponentKind.Tower);
            if (tower is not null) return (tower, tower.DefaultAngleDeg);
        }

        if (degree == 4)
        {
            // 十字交叉：優先以防禦塔樞紐或十字接頭錨定
            var cross = components.FirstOrDefault(c => c.Kind == WallComponentKind.CrossJunction);
            if (cross is not null) return (cross, 0f);

            var tower = components.FirstOrDefault(c => c.Kind == WallComponentKind.Tower);
            if (tower is not null) return (tower, tower.DefaultAngleDeg);
        }

        // 終極安全回退
        var fallback = components.FirstOrDefault(c => c.Kind == WallComponentKind.Straight) ?? components[0];
        return (fallback, fallback.DefaultAngleDeg);
    }

    /// <summary>
    /// 解析城門元件（依據城牆走向對齊門向）。
    /// </summary>
    public (WallComponentDefinition GateDef, float AngleDeg) ResolveGate(
        FortificationStyle style,
        WallConnections wallRunConnections)
    {
        var components = GetComponents(style);
        var gate = components.FirstOrDefault(c => c.Kind == WallComponentKind.Gate)
                   ?? components.FirstOrDefault(c => c.Kind == WallComponentKind.Straight)
                   ?? components[0];

        // 若城牆為南北走向（North | South），通道為東西向，城門旋轉 90°
        // 若城牆為東西走向（East | West），通道為南北向，城門旋轉 0°
        bool isNorthSouthWall = (wallRunConnections & (WallConnections.North | WallConnections.South)) != 0;
        float angle = isNorthSouthWall ? 90f : 0f;

        return (gate, angle);
    }

    /// <summary>
    /// 解析防禦塔元件。
    /// </summary>
    public WallComponentDefinition ResolveTower(FortificationStyle style)
    {
        var components = GetComponents(style);
        return components.FirstOrDefault(c => c.Kind == WallComponentKind.Tower)
               ?? components[0];
    }

    private static int CountConnections(WallConnections conn)
    {
        int count = 0;
        if (conn.HasFlag(WallConnections.North)) count++;
        if (conn.HasFlag(WallConnections.East)) count++;
        if (conn.HasFlag(WallConnections.South)) count++;
        if (conn.HasFlag(WallConnections.West)) count++;
        return count;
    }

    private static float CornerAngle(WallConnections connections)
    {
        // 依照原版旋轉慣例：
        // North + East = 0°
        // East + South = 90°
        // South + West = 180°
        // West + North = 270°
        if (connections.HasFlag(WallConnections.North) && connections.HasFlag(WallConnections.East)) return 0f;
        if (connections.HasFlag(WallConnections.East) && connections.HasFlag(WallConnections.South)) return 90f;
        if (connections.HasFlag(WallConnections.South) && connections.HasFlag(WallConnections.West)) return 180f;
        if (connections.HasFlag(WallConnections.West) && connections.HasFlag(WallConnections.North)) return 270f;
        return 0f;
    }

    private static float TJunctionAngle(WallConnections connections)
    {
        // 缺西（北-東-南） = 0°
        // 缺北（東-南-西） = 90°
        // 缺東（南-西-北） = 180°
        // 缺南（西-北-東） = 270°
        if (!connections.HasFlag(WallConnections.West)) return 0f;
        if (!connections.HasFlag(WallConnections.North)) return 90f;
        if (!connections.HasFlag(WallConnections.East)) return 180f;
        if (!connections.HasFlag(WallConnections.South)) return 270f;
        return 0f;
    }

    private static float EndCapAngle(WallConnections connections)
    {
        // 朝向單一開口方向的反向（向外封口）
        if (connections.HasFlag(WallConnections.North)) return 180f;
        if (connections.HasFlag(WallConnections.East)) return 270f;
        if (connections.HasFlag(WallConnections.South)) return 0f;
        if (connections.HasFlag(WallConnections.West)) return 90f;
        return 0f;
    }

    private void RegisterDefaults()
    {
        // 1. 羅馬石牆風格 (RomanStoneWall)
        Register(FortificationStyle.RomanStoneWall, new WallComponentDefinition(
            NameDef: "BauRomPal00",
            Kind: WallComponentKind.Straight,
            SupportedConnections: WallConnections.East | WallConnections.West,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "Pflaster_braun1"));

        Register(FortificationStyle.RomanStoneWall, new WallComponentDefinition(
            NameDef: "BauRomPal02_Palisadenecke",
            Kind: WallComponentKind.Corner,
            SupportedConnections: WallConnections.North | WallConnections.East,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "Pflaster_braun1"));

        Register(FortificationStyle.RomanStoneWall, new WallComponentDefinition(
            NameDef: "BauRomTur00_Turm",
            Kind: WallComponentKind.Tower,
            SupportedConnections: WallConnections.All,
            DefaultAngleDeg: 0f,
            FootprintTiles: 2,
            CollisionPixelSpan: 8,
            FoundationTexture: "Pflaster_braun2"));

        Register(FortificationStyle.RomanStoneWall, new WallComponentDefinition(
            NameDef: "BauRomMauertor",
            Kind: WallComponentKind.Gate,
            SupportedConnections: WallConnections.East | WallConnections.West,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "weg1"));

        Register(FortificationStyle.RomanStoneWall, new WallComponentDefinition(
            NameDef: "BauRomPalEnd",
            Kind: WallComponentKind.EndCap,
            SupportedConnections: WallConnections.North,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "Pflaster_braun1"));

        // 2. 日耳曼木質柵欄 (GermanicPalisade)
        Register(FortificationStyle.GermanicPalisade, new WallComponentDefinition(
            NameDef: "BauGerPal00",
            Kind: WallComponentKind.Straight,
            SupportedConnections: WallConnections.East | WallConnections.West,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "Erde"));

        Register(FortificationStyle.GermanicPalisade, new WallComponentDefinition(
            NameDef: "BauGerPal02_Palisadenecke",
            Kind: WallComponentKind.Corner,
            SupportedConnections: WallConnections.North | WallConnections.East,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "Erde"));

        Register(FortificationStyle.GermanicPalisade, new WallComponentDefinition(
            NameDef: "BauGerTur00",
            Kind: WallComponentKind.Tower,
            SupportedConnections: WallConnections.All,
            DefaultAngleDeg: 0f,
            FootprintTiles: 2,
            CollisionPixelSpan: 8,
            FoundationTexture: "Erde"));

        Register(FortificationStyle.GermanicPalisade, new WallComponentDefinition(
            NameDef: "BauGerPalTor",
            Kind: WallComponentKind.Gate,
            SupportedConnections: WallConnections.East | WallConnections.West,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "PFAD1"));

        Register(FortificationStyle.GermanicPalisade, new WallComponentDefinition(
            NameDef: "BauGerPalEnd",
            Kind: WallComponentKind.EndCap,
            SupportedConnections: WallConnections.North,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "Erde"));

        // 3. 凱爾特木質防禦 (CelticPalisade)
        Register(FortificationStyle.CelticPalisade, new WallComponentDefinition(
            NameDef: "BauKelPal00",
            Kind: WallComponentKind.Straight,
            SupportedConnections: WallConnections.East | WallConnections.West,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "Erde"));

        Register(FortificationStyle.CelticPalisade, new WallComponentDefinition(
            NameDef: "BauKelPal02_Palisadenecke",
            Kind: WallComponentKind.Corner,
            SupportedConnections: WallConnections.North | WallConnections.East,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "Erde"));

        Register(FortificationStyle.CelticPalisade, new WallComponentDefinition(
            NameDef: "BauKelTur00",
            Kind: WallComponentKind.Tower,
            SupportedConnections: WallConnections.All,
            DefaultAngleDeg: 0f,
            FootprintTiles: 2,
            CollisionPixelSpan: 8,
            FoundationTexture: "Erde"));

        Register(FortificationStyle.CelticPalisade, new WallComponentDefinition(
            NameDef: "BauKelPalTor",
            Kind: WallComponentKind.Gate,
            SupportedConnections: WallConnections.East | WallConnections.West,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "PFAD1"));

        // 4. 匈人拒馬 (HunBarricade)
        Register(FortificationStyle.HunBarricade, new WallComponentDefinition(
            NameDef: "BauHunPal00",
            Kind: WallComponentKind.Straight,
            SupportedConnections: WallConnections.East | WallConnections.West,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "Erde"));

        Register(FortificationStyle.HunBarricade, new WallComponentDefinition(
            NameDef: "BauHunPal02_Palisadenecke",
            Kind: WallComponentKind.Corner,
            SupportedConnections: WallConnections.North | WallConnections.East,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "Erde"));

        Register(FortificationStyle.HunBarricade, new WallComponentDefinition(
            NameDef: "BauHunTur00",
            Kind: WallComponentKind.Tower,
            SupportedConnections: WallConnections.All,
            DefaultAngleDeg: 0f,
            FootprintTiles: 2,
            CollisionPixelSpan: 8,
            FoundationTexture: "Erde"));

        Register(FortificationStyle.HunBarricade, new WallComponentDefinition(
            NameDef: "BauHunPalTor",
            Kind: WallComponentKind.Gate,
            SupportedConnections: WallConnections.East | WallConnections.West,
            DefaultAngleDeg: 0f,
            FootprintTiles: 1,
            CollisionPixelSpan: 4,
            FoundationTexture: "PFAD1"));
    }
}
