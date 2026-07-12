namespace AgainstRomeModifier.Maps;

public sealed class EndlessMapDeleter
{
    private readonly EndlessMapCatalog _catalog;
    public EndlessMapDeleter(EndlessMapCatalog? catalog = null) => _catalog = catalog ?? new EndlessMapCatalog();

    public void Delete(string gamePath, int slot)
    {
        string normalizedGamePath = EndlessMapCatalog.ValidateGamePath(gamePath);
        EndlessMapInfo map = _catalog.Require(normalizedGamePath, slot);
        if (!map.IsCustom || !CustomMapManifest.IsCustomMapDirectory(map.DirectoryPath))
            throw new InvalidOperationException("只能刪除由地圖編輯器建立的自製地圖。");

        string mapsPath = Path.GetFullPath(Path.Combine(normalizedGamePath, "MAPS"));
        string mapPath = Path.GetFullPath(map.DirectoryPath);
        if (!StringComparer.OrdinalIgnoreCase.Equals(Path.GetDirectoryName(mapPath), mapsPath))
            throw new InvalidOperationException("地圖資料夾不在預期的位置，已取消刪除。");

        string temporary = mapPath + ".deleting_arm";
        if (Directory.Exists(temporary)) throw new IOException("地圖刪除暫存資料夾已存在，請先檢查: " + temporary);
        Directory.Move(mapPath, temporary);
        try
        {
            using var rollback = new FileRollbackScope();
            CustomMapManifest manifest = CustomMapManifest.Load(normalizedGamePath);
            manifest.Remove(slot);
            manifest.Save(normalizedGamePath, rollback);
            Directory.Delete(temporary, recursive: true);
            rollback.Commit();
        }
        catch
        {
            if (!Directory.Exists(mapPath) && Directory.Exists(temporary)) Directory.Move(temporary, mapPath);
            throw;
        }
    }
}
