using System.Globalization;
using AgainstRomeModifier;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

/// <summary>在登錄新槽前完成空場景與平坦地形；不要求使用者先儲存才能清除來源內容。</summary>
internal static class BlankMapBuilder
{
    internal static EndlessMapInfo Create(string gamePath, string sourceId, int slot, string name)
        => new EndlessMapCloner().ClonePrepared(gamePath, sourceId, slot, name, directory => Prepare(gamePath, directory), standaloneLevel: true);

    private static void Prepare(string gamePath, string directory)
    {
        using var library = new FloorTextureLibrary(Path.Combine(gamePath, "floortex.dat"));
        if (!library.IsAvailable) throw new InvalidDataException("建立空白場景需要有效的 floortex.dat。");
        var materials = new FloorMaterialCatalog(library);
        var textures = BodenTexturesDocument.Load(Path.Combine(directory, "boden.txt"));
        FloorMaterial material = textures.Textures.Select(texture => materials.FindByTexture(texture)?.Id).OfType<string>()
            .GroupBy(id => id, StringComparer.OrdinalIgnoreCase).OrderByDescending(group => group.Count())
            .Select(group => materials.Materials.Single(m => m.Id == group.Key)).FirstOrDefault()
            ?? (materials.Materials.Count > 0 ? materials.Materials[0] : throw new InvalidDataException("找不到可用的原生地表材質。"));
        string texture = material.RepresentativeTexture;
        Bitmap floor = library.Get(texture) ?? throw new InvalidDataException("無法解碼空白場景地表材質。");
        var ini = BodenIniDocument.Load(Path.Combine(directory, "boden.ini"));
        if (!float.TryParse(ini.GetValue("Heightmapstep"), NumberStyles.Float, CultureInfo.InvariantCulture, out float step) || !float.IsFinite(step) || step <= 0
            || !float.TryParse(ini.GetValue("Waterlevel"), NumberStyles.Float, CultureInfo.InvariantCulture, out float water) || !float.IsFinite(water))
            throw new InvalidDataException("空白場景的高度比例或水面資料不符。");
        int height = (int)MathF.Round(water / step) + 20;
        if (height is < 0 or > 255 || height * step <= water) throw new InvalidDataException("水面太高，無法建立高於水面的空白場景。");
        using var rollback = new FileRollbackScope();
        textures.SetTextures(Enumerable.Repeat(texture, textures.Dimension * textures.Dimension).ToArray()); textures.Save(rollback);
        foreach (var (file, value, expected) in new[] { ("boden.bmp", (byte)height, 257), ("vertex.bmp", (byte)255, 257),
            ("smooth.bmp", (byte)0, 257), ("emboss.bmp", (byte)0, 257), ("collision.bmp", (byte)0, 256) })
        {
            var source = TerrainLayerFiles.Read(Path.Combine(directory, file)) ?? throw new InvalidDataException($"空白場景缺少 {file}。");
            if (textures.Dimension != 64 || source.Width != expected || source.Height != expected) throw new InvalidDataException($"{file} 尺寸不符。");
            var pixels = Enumerable.Repeat(Color.FromArgb(value, value, value).ToArgb(), expected * expected).ToArray();
            var values = Enumerable.Repeat(value, expected * expected).ToArray();
            // Deliberately supply uniform ARGB too: EncodeWithGreen otherwise preserves old red/blue when green already matches.
            TerrainLayerFiles.Write(Path.Combine(directory, file), new(expected, expected, pixels, values), values, rollback);
        }
        using (var minimap = new Bitmap(256, 256))
        using (var graphics = Graphics.FromImage(minimap))
        {
            for (int y = 0; y < 64; y++) for (int x = 0; x < 64; x++) graphics.DrawImage(floor, new Rectangle(x * 4, y * 4, 4, 4));
            using var stream = new MemoryStream(); minimap.Save(stream, System.Drawing.Imaging.ImageFormat.Bmp);
            SafeFileWriter.WriteAllBytes(Path.Combine(directory, "minimap.bmp"), stream.ToArray(), rollback);
        }
        TerrainLayerFiles.InvalidateHeightCaches(directory, rollback);
        BlankMapContent.ResetStagingDirectory(directory);
        rollback.Commit();
    }

    internal static string Describe(string sourceId, bool en) => en
        ? $"Source: {sourceId}\r\nCleared: all native and SDL objects (including markers and linked objects), editor placements/events, source level scripts and saved spawn bindings.\r\nCreated: flat dry terrain, one native material, passable ground and a new idle level script. No settlements, troops, resources or AI are supplied. Place starting objects and victory events yourself.\r\nKept: faction/population defaults, environment and other native DATA pools from the template.\r\nExperimental: game startup and native DATA compatibility still require in-game validation."
        : $"來源：{sourceId}\r\n清除：所有原生與 SDL 物件（含標記及連結物件）、編輯器放置／事件、来源關卡腳本與出生綁定。\r\n建立：高於水面的平坦地形、單一原生材質、可通行地面與新的等待腳本。不提供聚落、部隊、資源或 AI；起始物件與勝利事件需自行放置。\r\n保留：範本派系／人口預設、環境及其他原生 DATA 池。\r\n實驗功能：遊戲開局及原生 DATA 相容性仍須在遊戲內驗證。";
}
