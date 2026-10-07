using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Core.Services;

internal static class CustomMapRestoreService
{
    internal static void Execute(string gamePath, FileRollbackScope rollback, Action restoreOriginals, bool preserveCustomMaps = true)
    {
        var maps = new EndlessMapCatalog().List(gamePath).Where(map => map.IsCustom).ToArray();
        if (!preserveCustomMaps && maps.Length > 0)
        {
            var manifest = CustomMapManifest.Load(gamePath);
            foreach (var map in maps)
            {
                if (!manifest.Entries.Any(entry => entry.Slot == map.Slot)) throw new InvalidOperationException("自製地圖未登記，已取消完整還原: " + map.Id);
                if (Directory.Exists(map.DirectoryPath + ".deleting_arm")) throw new IOException("地圖刪除暫存資料夾已存在: " + map.Id);
            }
            rollback.TrackFile(Path.Combine(gamePath, "MAPS", CustomMapManifest.FileName));
        }
        // Capture before restoring: legacy backups and AI repairs must not overwrite the user's maps.
        foreach (var map in maps) rollback.TrackDirectory(map.DirectoryPath);
        restoreOriginals();
        foreach (var map in maps)
        {
            if (preserveCustomMaps) rollback.RestoreDirectory(map.DirectoryPath);
            else new EndlessMapDeleter().Delete(gamePath, map.Slot);
        }
    }
}
