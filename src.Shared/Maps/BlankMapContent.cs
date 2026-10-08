using System.Buffers.Binary;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Maps;

/// <summary>新空白場景的內容初始化。只能作用於 Cloner 尚未發布的暫存副本；遊戲開局仍須獨立驗收。</summary>
public static class BlankMapContent
{
    // Record and parallel-column widths from the native readers; no in-memory strides are used as file strides.
    private sealed record Slots(string Name, int Header, int Record, int[] Columns, int[] ExtraHeader, int StateWidth = 1);
    private static readonly Slots[] Runtime = [
        new("anim.dat", 8, 21, [2, 2], []),
        new("gfxtype.dat", 8, 15, [2], []),
        new("action.dat", 12, 25, [], [6]),
        new("hirarchy.dat", 12, 103, [2], [50]),
        new("formatio.dat", 8, 15, [4, 2, 4], []),
        new("lager.dat", 16, 43, [2], [6, 10]),
        new("biglager.dat", 12, 1601, [], [800]),
        new("light.dat", 8, 4, [4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4], [], 4),
        new("particle.dat", 12, 2252, [4, 4, 4, 4], [64], 2),
        new("explos.dat", 8, 46, [4, 4], [], 2),
        new("hitex.dat", 8, 28, [4, 4, 4], [], 2),
        new("flash.dat", 12, 201, [32], [16])
    ];

    public static void ResetStagingDirectory(string directory)
    {
        string full = Path.GetFullPath(directory);
        if (!Path.GetFileName(full).EndsWith(".tmp_arm", StringComparison.OrdinalIgnoreCase)
            || !string.Equals(Path.GetFileName(Path.GetDirectoryName(full)), "MAPS", StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException("空白初始化只能作用於尚未登錄的地圖暫存副本。");
        foreach (string path in Directory.EnumerateFileSystemEntries(full, "*", SearchOption.AllDirectories).Prepend(full))
            if (File.GetAttributes(path).HasFlag(FileAttributes.ReparsePoint)) throw new IOException("空白初始化不接受 reparse points。");

        // Prepare every binary in memory before writing. A full pool or unknown layout is a failure, never a guessed reset.
        var pending = Runtime.Select(layout => (layout.Name, Bytes: EmptySlots(Path.Combine(full, "DATA", layout.Name), layout))).ToList();
        pending.Add(("way.dat", EmptyWays(Path.Combine(full, "DATA", "way.dat"))));
        LevelObjectStore store = LevelObjectStore.Load(full);
        store.ResetForBlankMap();
        using var rollback = new FileRollbackScope();
        store.Save(full, rollback);
        foreach (var (name, bytes) in pending) SafeFileWriter.WriteAllBytes(Path.Combine(full, "DATA", name), bytes, rollback);
        // SDL 聚落範本必須保持原樣：2026-10-08 遊戲內實測，把 SDL 清成沒有任何 object 區段會讓遊戲在載入時卡死
        // （Responding=False）；保留原範本且使用閒置腳本時可正常載入，且腳本不呼叫就不會生成聚落。
        new ScenarioDocument().Save(full, rollback);
        string scripts = LevelScriptInjector.ScriptDirectory(full);
        string level = Path.Combine(scripts, LevelScriptInjector.ScriptFile);
        byte[] original = File.ReadAllBytes(level), idle = BciImage.CreateIdleLevel().Serialize();
        byte[] bootstrap = IsPfil(original) ? GameLZSS.CompressPfil(idle, original.AsSpan(0, 64).ToArray()) : idle;
        foreach (string path in Directory.GetFiles(scripts, "*", SearchOption.AllDirectories))
            if (Path.GetExtension(path).Equals(".bci", StringComparison.OrdinalIgnoreCase)
                || Path.GetFileName(path).Equals(LevelScriptInjector.OriginalBackupFile, StringComparison.OrdinalIgnoreCase))
            { rollback.TrackFile(path); File.Delete(path); }
        SafeFileWriter.WriteAllBytes(level, bootstrap, rollback);
        // Future spawn/event saves must restore this bootstrap, never the source map's auto-spawn main.
        SafeFileWriter.WriteAllBytes(Path.Combine(scripts, LevelScriptInjector.OriginalBackupFile), bootstrap, rollback);
        rollback.Commit();
    }

    private static byte[] EmptySlots(string path, Slots layout)
    {
        byte[] original = File.ReadAllBytes(path), data = Payload(original);
        if (data.Length < layout.Header || Int(data, 0) != 1) throw new InvalidDataException($"{layout.Name} 版本或標頭不符。");
        int count = Int(data, 4), size = layout.Record + layout.Columns.Sum();
        if (count < 1 || count > 33000 || (long)layout.Header + (long)count * size != data.Length)
            throw new InvalidDataException($"{layout.Name} 槽位／長度不符。");
        for (int index = 0; index < layout.ExtraHeader.Length; index++)
            if (Int(data, 8 + index * 4) != layout.ExtraHeader[index]) throw new InvalidDataException($"{layout.Name} 陣列欄寬不符。");
        int empty = Enumerable.Range(0, count).FirstOrDefault(slot =>
            data.AsSpan(layout.Header + slot * layout.Record, layout.StateWidth).IndexOfAnyExcept((byte)0) < 0, -1);
        if (empty < 0) throw new InvalidDataException($"{layout.Name} 無空槽範本。");
        byte[] record = data.AsSpan(layout.Header + empty * layout.Record, layout.Record).ToArray();
        for (int slot = 0; slot < count; slot++) record.CopyTo(data, layout.Header + slot * layout.Record);
        int start = layout.Header + count * layout.Record;
        foreach (int width in layout.Columns)
        {
            byte[] column = data.AsSpan(start + empty * width, width).ToArray();
            for (int slot = 0; slot < count; slot++) column.CopyTo(data, start + slot * width);
            start += count * width;
        }
        return Pack(data, original);
    }

    private static byte[] EmptyWays(string path)
    {
        byte[] original = File.ReadAllBytes(path), data = Payload(original);
        if (data.Length < 12 || Int(data, 0) != 1 || Int(data, 8) != 256) throw new InvalidDataException("way.dat 版本或路徑容量不符。");
        int count = Int(data, 4);
        if (count < 1 || count > 33000 || 12L + count * 1030L != data.Length) throw new InvalidDataException("way.dat 槽位／長度不符。");
        // 0x48dba0: count × (uint16 usedPoints + 256 × (uint16 X, uint16 Z)), then count × int32 state.
        data.AsSpan(12).Clear();
        return Pack(data, original);
    }

    private static int Int(byte[] data, int offset) => BinaryPrimitives.ReadInt32LittleEndian(data.AsSpan(offset));
    private static bool IsPfil(byte[] bytes) => bytes.Length >= 64 && bytes.AsSpan(0, 4).SequenceEqual("PFIL"u8);
    private static byte[] Payload(byte[] bytes) => IsPfil(bytes) ? GameLZSS.DecompressPfil(bytes) : bytes.ToArray();
    private static byte[] Pack(byte[] data, byte[] original) => IsPfil(original) ? GameLZSS.CompressPfil(data, original.AsSpan(0, 64).ToArray()) : data;
}
