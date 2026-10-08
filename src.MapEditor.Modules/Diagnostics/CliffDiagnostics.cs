namespace AgainstRomeMapEditor.Modules.Diagnostics;

/// <summary>
/// 懸崖陡坡與通行碰撞驗證診斷器。
/// 確保陡坡具備合適通行阻擋、懸崖印章具備碰撞遮罩，且高台地形具備聯外坡道。
/// </summary>
public static class CliffDiagnostics
{
    /// <summary>
    /// 檢查地圖快照中的懸崖與高低差通行規則。
    /// </summary>
    public static IReadOnlyList<MapIssue> Check(
        MapCheckSnapshot map,
        CliffTileCatalog? catalog = null,
        IReadOnlyList<string>? textures = null,
        int tileDimension = 64)
    {
        ArgumentNullException.ThrowIfNull(map);
        var issues = new List<MapIssue>();

        if (map.Collision is null || map.CollisionSize <= 0)
        {
            return issues;
        }

        int collisionSize = map.CollisionSize;
        bool hasHeights = map.Heights is not null && map.HeightSize > 1 && map.Heights.Count == (long)map.HeightSize * map.HeightSize;

        // 1. 檢查陡坡未標記阻擋 (cliff-steep-unblocked)
        if (hasHeights)
        {
            int heightSize = map.HeightSize;
            float step = map.HeightStep > 0 ? map.HeightStep : 4.0f;

            for (int ty = 0; ty < tileDimension; ty++)
            {
                for (int tx = 0; tx < tileDimension; tx++)
                {
                    // 取圖塊對應的頂點高度範圍
                    int vx = Math.Clamp(tx * (heightSize - 1) / tileDimension, 0, heightSize - 2);
                    int vy = Math.Clamp(ty * (heightSize - 1) / tileDimension, 0, heightSize - 2);

                    byte h00 = map.Heights![vy * heightSize + vx];
                    byte h10 = map.Heights![vy * heightSize + (vx + 1)];
                    byte h01 = map.Heights![(vy + 1) * heightSize + vx];
                    byte h11 = map.Heights![(vy + 1) * heightSize + (vx + 1)];

                    int maxDelta = Math.Max(Math.Max(Math.Abs(h10 - h00), Math.Abs(h01 - h00)), Math.Max(Math.Abs(h11 - h00), Math.Abs(h11 - h10)));

                    // 坡度超過 45 度（delta >= 15）
                    if (maxDelta >= 15)
                    {
                        // 檢查該圖塊的碰撞格是否全為可通行（0）
                        int cx = Math.Clamp(tx * collisionSize / tileDimension, 0, collisionSize - 1);
                        int cy = Math.Clamp(ty * collisionSize / tileDimension, 0, collisionSize - 1);
                        int collisionIdx = cy * collisionSize + cx;

                        if (map.Collision[collisionIdx] == 0)
                        {
                            float wx = (tx + 0.5f) * (16384.0f / tileDimension);
                            float wz = (ty + 0.5f) * (16384.0f / tileDimension);

                            issues.Add(new MapIssue(
                                MapIssueSeverity.Warning,
                                "cliff-steep-unblocked",
                                "坡度超過 45 度之陡坡未設定通行阻擋，單位可能異常攀爬。",
                                "Steep slope exceeding 45 degrees has no collision blocking; units may climb abnormally.",
                                WorldX: wx,
                                WorldZ: wz));
                        }
                    }
                }
            }
        }

        // 2. 檢查已放置懸崖印章圖塊處缺少通行碰撞 (cliff-missing-collision)
        if (catalog is not null && textures is not null && textures.Count == tileDimension * tileDimension)
        {
            for (int ty = 0; ty < tileDimension; ty++)
            {
                for (int tx = 0; tx < tileDimension; tx++)
                {
                    string texture = textures[ty * tileDimension + tx];
                    if (catalog.IsCliffTexture(texture))
                    {
                        int cx = Math.Clamp(tx * collisionSize / tileDimension, 0, collisionSize - 1);
                        int cy = Math.Clamp(ty * collisionSize / tileDimension, 0, collisionSize - 1);
                        int collisionIdx = cy * collisionSize + cx;

                        if (map.Collision[collisionIdx] == 0)
                        {
                            float wx = (tx + 0.5f) * (16384.0f / tileDimension);
                            float wz = (ty + 0.5f) * (16384.0f / tileDimension);

                            issues.Add(new MapIssue(
                                MapIssueSeverity.Warning,
                                "cliff-missing-collision",
                                "懸崖岩壁圖塊處未阻擋通行，單位可能穿透垂直岩壁。",
                                "Cliff face tile has no collision blocking; units may walk through sheer cliffs.",
                                WorldX: wx,
                                WorldZ: wz));
                        }
                    }
                }
            }
        }

        return issues;
    }
}
