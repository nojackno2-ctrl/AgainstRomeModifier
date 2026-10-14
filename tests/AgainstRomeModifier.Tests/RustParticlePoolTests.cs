using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

internal sealed class RequiresRustParticlePoolFactAttribute : FactAttribute
{
    public RequiresRustParticlePoolFactAttribute()
    {
        if (RustLightPoolTests.Required) return;
        if (!RustParticlePool.TryEnsureLoaded(out string? error)) Skip = error;
    }
}

internal sealed class RequiresInstalledRustParticlePoolFactAttribute : FactAttribute
{
    public RequiresInstalledRustParticlePoolFactAttribute()
    {
        if (RustLightPoolTests.Required) return;
        if (!RustParticlePool.TryEnsureLoaded(out string? error)) Skip = error;
        else if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("ARM_GAME_PATH")))
            Skip = "Requires opt-in ARM_GAME_PATH; installed files are read-only.";
    }
}

[Collection("Rust native interop")]
public sealed class RustParticlePoolTests
{
    private const int Capacity = 1024, RecordSize = 2252, PayloadSize = 2322444;

    // Fixture serialization and differential oracle only. Production bridge never knows these offsets.
    private static int RecordOffset(int slot) => 12 + slot * RecordSize;
    private static int StateOffset(int slot, int column) => 12 + Capacity * RecordSize + column * Capacity * 4 + slot * 4;
    private static void I32(byte[] bytes, int offset, int value)
        => BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(offset), value);
    private static byte[] Fixture(bool patterned = false)
    {
        byte[] p = new byte[PayloadSize];
        if (patterned) for (int i = 12; i < p.Length; i++) p[i] = unchecked((byte)(i * 73 + i / 17));
        I32(p, 0, 1); I32(p, 4, Capacity); I32(p, 8, 64);
        return p;
    }
    private static void Emitter(byte[] p, int slot, ushort active, int count, short owner, int age)
    {
        int offset = RecordOffset(slot);
        BinaryPrimitives.WriteUInt16LittleEndian(p.AsSpan(offset), active);
        I32(p, offset + 2, count);
        BinaryPrimitives.WriteInt16LittleEndian(p.AsSpan(offset + 6), owner);
        I32(p, offset + 8, age);
    }
    private static byte[] StateBytes(byte[] payload, int slot)
    {
        byte[] state = new byte[16];
        for (int c = 0; c < 4; c++) payload.AsSpan(StateOffset(slot, c), 4).CopyTo(state.AsSpan(c * 4));
        return state;
    }
    private static byte[] Wrapped(byte[] payload)
    {
        byte[] header = Enumerable.Repeat((byte)0x5a, 64).ToArray();
        "PFIL"u8.CopyTo(header);
        // Reuse the established C# compressor only in the test oracle.
        return GameLZSS.CompressPfil(payload, header);
    }

    [Fact]
    public void Managed_metadata_matches_all_ABI24_offsets()
    {
        Assert.Equal(24, RustParticlePool.ManagedMetadataSize);
        var expected = new Dictionary<string, int>
        {
            ["struct_size"] = 0, ["slot"] = 4, ["active_raw"] = 8, ["count_raw"] = 12,
            ["owner"] = 16, ["reserved"] = 18, ["age_raw"] = 20,
        };
        Assert.Equal(expected.Count, RustParticlePool.ManagedFieldOffsets.Count);
        foreach (var field in expected)
        {
            Assert.Equal(field.Value, RustParticlePool.ExpectedFieldOffsets[field.Key]);
            Assert.Equal(field.Value, RustParticlePool.ManagedFieldOffsets[field.Key]);
        }
    }

    [RequiresNativeGateFact]
    public void Required_gate_demands_appended_particle_exports_and_exact_sizes()
    {
        Assert.True(RustParticlePool.TryEnsureLoaded(out string? error), error);
        Assert.Equal(1u, RustParticlePool.NativeAbiVersion);
        Assert.Equal((24u, (nuint)2252, (nuint)16, (nuint)35, (nuint)2322444), RustParticlePool.NativeSizes);
    }

    [RequiresRustParticlePoolFact]
    public void All_slots_records_four_columns_and_native_metadata_roundtrip_random_bits()
    {
        byte[] p = Fixture(true);
        using var pool = RustParticlePool.Open(p);
        int active = 0;
        for (int slot = 0; slot < Capacity; slot++)
        {
            int offset = RecordOffset(slot);
            ushort flag = BinaryPrimitives.ReadUInt16LittleEndian(p.AsSpan(offset));
            var snapshot = pool.CaptureAny(slot);
            Assert.Equal(slot, snapshot.Slot);
            Assert.Equal((uint)flag, snapshot.ActiveRaw);
            Assert.Equal(BinaryPrimitives.ReadInt32LittleEndian(p.AsSpan(offset + 2)), snapshot.CountRaw);
            Assert.Equal(BinaryPrimitives.ReadInt16LittleEndian(p.AsSpan(offset + 6)), snapshot.Owner);
            Assert.Equal(BinaryPrimitives.ReadInt32LittleEndian(p.AsSpan(offset + 8)), snapshot.AgeRaw);
            Bytes(p.AsSpan(offset, RecordSize), snapshot.RawRecord, $"record {slot}");
            Bytes(StateBytes(p, slot), snapshot.RawState, $"four columns {slot}");
            Assert.Equal(flag != 0, pool.Capture(slot) is not null);
            if (flag != 0) active++;
        }
        Assert.Equal(active, pool.ActiveCount);
        Bytes(p, pool.CopyPayload(), "payload");
    }

    [RequiresRustParticlePoolFact]
    public void Non1_flags_signed_extremes_and_counts_are_preserved_and_validation_is_optional()
    {
        byte[] p = Fixture();
        (ushort Flag, int Count, short Owner, int Age)[] values =
        [ (0, -1, -1, int.MinValue), (7, 0, short.MinValue, int.MaxValue),
          (ushort.MaxValue, 64, short.MaxValue, -1), (0x8000, 65, -2, 0), (1, int.MinValue, 0, 123),
          (0, int.MaxValue, 42, -123) ];
        for (int i = 0; i < values.Length; i++)
        {
            var v = values[i]; Emitter(p, i, v.Flag, v.Count, v.Owner, v.Age);
            for (int c = 0; c < 4; c++) I32(p, StateOffset(i, c), unchecked(int.MinValue + i * 97 + c));
            p[RecordOffset(i) + 12 + 63 * 35] = 0xfe; // inactive emitter/invalid count must still allow raw child access
        }
        using var pool = RustParticlePool.Open(p);
        byte[] before = pool.CopyPayload();
        for (int i = 0; i < values.Length; i++)
        {
            var v = values[i]; var s = pool.CaptureAny(i);
            Assert.Equal(((uint)v.Flag, v.Count, v.Owner, v.Age), (s.ActiveRaw, s.CountRaw, s.Owner, s.AgeRaw));
            Assert.Equal(v.Flag != 0, s.IsActive);
            Assert.Equal(v.Flag != 0, pool.Capture(i) is not null);
            Assert.Equal(0xfe, pool.ChildAny(i, 63)[0]);
            if (v.Count is >= 0 and <= 64) Assert.Equal((uint)v.Count, pool.ValidateRenderableCount(i));
            else
            {
                var ex = Assert.Throws<RustParticlePoolException>(() => pool.ValidateRenderableCount(i));
                Assert.Equal(114, ex.Code);
                Assert.IsAssignableFrom<IOException>(ex);
                Assert.Equal("ar_particle_pool_validate_renderable_count", ex.Operation);
            }
        }
        Bytes(before, pool.CopyPayload(), "optional validation cannot mutate pool");
        Assert.Equal(4, pool.ActiveCount);
    }

    [RequiresRustParticlePoolFact]
    public void Input_and_all_exported_copies_are_detached_and_snapshot_survives_disposal()
    {
        byte[] input = Fixture(true), original = (byte[])input.Clone();
        var pool = RustParticlePool.Open(input);
        var snapshot = pool.CaptureAny(17);
        byte[] expectedRecord = original.AsSpan(RecordOffset(17), RecordSize).ToArray();
        byte[] expectedState = StateBytes(original, 17);
        Array.Clear(input);
        byte[] record = snapshot.RecordBytes(), state = snapshot.StateBytes(), payload = pool.CopyPayload();
        var emitter = snapshot.CopyEmitterRecord();
        Array.Clear(record); Array.Clear(state); Array.Clear(payload);
        byte[] emitterCopy = emitter.ToBytes(); Array.Clear(emitterCopy);
        byte[] child = pool.ChildAny(17, 63), expectedChild = (byte[])child.Clone(); Array.Clear(child);
        Bytes(expectedChild, pool.ChildAny(17, 63), "child copy");
        for (int ordinal = 0; ordinal < 64; ordinal++)
            Bytes(expectedRecord.AsSpan(12 + ordinal * 35, 35), pool.ChildAny(17, ordinal), $"opaque child {ordinal}");
        byte[] sha = pool.CopyInputSha256(); Array.Clear(sha);
        Bytes(SHA256.HashData(original), pool.CopyInputSha256(), "original input digest");
        Bytes(original, pool.CopyPayload(), "owned input");
        pool.Dispose();
        Bytes(expectedRecord, snapshot.RawRecord, "detached record");
        Bytes(expectedState, snapshot.RawState, "detached state");
        Bytes(expectedRecord, emitter.Bytes, "existing RustParticleEmitterRecord integration");
    }

    [RequiresRustParticlePoolFact]
    public void Raw_and_PFIL_two_pass_copies_preserve_exact_header_payload_and_source_digest()
    {
        byte[] payload = Fixture(true);
        foreach (bool wrapped in new[] { false, true })
        {
            byte[] input = wrapped ? Wrapped(payload) : payload;
            using var pool = RustParticlePool.Open(input);
            Bytes(payload, pool.CopyPayload(), "decoded payload");
            byte[] expectedHeader = wrapped ? input[..64] : [];
            Bytes(expectedHeader, pool.CopyPfilHeader(), "header");
            byte[] header = pool.CopyPfilHeader(); Array.Clear(header);
            Bytes(expectedHeader, pool.CopyPfilHeader(), "header defensive copy");
            Bytes(SHA256.HashData(input), pool.CopyInputSha256(), "source digest including wrapper");
            Differential(input, NativeParticlePool.ParsePayload(payload), wrapped ? "synthetic PFIL" : "synthetic raw");
        }
    }

    [RequiresRustParticlePoolFact]
    public void Invalid_open_leaks_no_handles_and_reports_native_operation_and_code()
    {
        long opened = RustParticlePool.OpenedHandleCount, freed = RustParticlePool.FreedHandleCount;
        byte[] valid = Fixture();
        foreach (int length in new[] { 0, 1, 12, PayloadSize - 1, PayloadSize + 1 })
        {
            byte[] bad = new byte[length]; valid.AsSpan(0, Math.Min(length, valid.Length)).CopyTo(bad);
            var ex = Assert.Throws<RustParticlePoolException>(() => RustParticlePool.Open(bad));
            Assert.Equal("ar_particle_pool_open", ex.Operation);
            Assert.NotEqual(0, ex.Code);
        }
        foreach (var (offset, value, code) in new[] { (0, 2, 111), (4, -1, 112), (8, 63, 113) })
        {
            byte[] bad = (byte[])valid.Clone(); I32(bad, offset, value);
            Assert.Equal(code, Assert.Throws<RustParticlePoolException>(() => RustParticlePool.Open(bad)).Code);
        }
        byte[] packed = Wrapped(valid);
        foreach (int length in new[] { 4, 63, 64, packed.Length - 1 })
            Assert.Throws<RustParticlePoolException>(() => RustParticlePool.Open(packed.AsSpan(0, length)));
        Assert.Equal(opened, RustParticlePool.OpenedHandleCount);
        Assert.Equal(freed, RustParticlePool.FreedHandleCount);
    }

    [RequiresRustParticlePoolFact]
    public void Bounds_errors_leave_pool_unchanged_and_every_disposed_operation_throws()
    {
        byte[] p = Fixture(); var pool = RustParticlePool.Open(p);
        long freed = RustParticlePool.FreedHandleCount;
        foreach (int slot in new[] { -1, 1024, int.MaxValue })
        {
            Assert.Throws<ArgumentOutOfRangeException>(() => pool.Capture(slot));
            Assert.Throws<ArgumentOutOfRangeException>(() => pool.CaptureAny(slot));
            Assert.Throws<ArgumentOutOfRangeException>(() => pool.ChildAny(slot, 0));
            Assert.Throws<ArgumentOutOfRangeException>(() => pool.ValidateRenderableCount(slot));
        }
        foreach (int ordinal in new[] { -1, 64, int.MaxValue })
            Assert.Throws<ArgumentOutOfRangeException>(() => pool.ChildAny(0, ordinal));
        Bytes(p, pool.CopyPayload(), "errors do not mutate");
        pool.Dispose(); pool.Dispose();
        Assert.True(pool.IsDisposed);
        Assert.Equal(freed + 1, RustParticlePool.FreedHandleCount);
        Assert.Throws<ObjectDisposedException>(() => pool.ActiveCount);
        Assert.Throws<ObjectDisposedException>(() => pool.Capture(0));
        Assert.Throws<ObjectDisposedException>(() => pool.CaptureAny(0));
        Assert.Throws<ObjectDisposedException>(() => pool.ChildAny(0, 0));
        Assert.Throws<ObjectDisposedException>(() => pool.ValidateRenderableCount(0));
        Assert.Throws<ObjectDisposedException>(() => pool.CopyPayload());
        Assert.Throws<ObjectDisposedException>(() => pool.CopyPfilHeader());
        Assert.Throws<ObjectDisposedException>(() => pool.CopyInputSha256());
    }

    [RequiresRustParticlePoolFact]
    public async Task Concurrent_readers_and_serialized_disposal_free_exactly_once()
    {
        long opened = RustParticlePool.OpenedHandleCount, freed = RustParticlePool.FreedHandleCount;
        for (int round = 0; round < 8; round++)
        {
            byte[] p = Fixture(); Emitter(p, 3, 7, 64, -1, int.MinValue);
            var pool = RustParticlePool.Open(p);
            using var start = new ManualResetEventSlim();
            var failures = new ConcurrentBag<Exception>();
            var readers = Enumerable.Range(0, 4).Select(_ => Task.Run(() =>
            {
                start.Wait();
                for (int i = 0; i < 100; i++)
                {
                    try { Assert.Equal(7u, pool.CaptureAny(3).ActiveRaw); Assert.Equal(35, pool.ChildAny(3, 63).Length); }
                    catch (ObjectDisposedException) { break; }
                    catch (Exception ex) { failures.Add(ex); break; }
                }
            })).ToArray();
            var disposers = Enumerable.Range(0, 3).Select(_ => Task.Run(() => { start.Wait(); pool.Dispose(); })).ToArray();
            start.Set(); await Task.WhenAll(readers.Concat(disposers));
            pool.Dispose(); Assert.Empty(failures); Assert.True(pool.IsDisposed);
        }
        Assert.Equal(opened + 8, RustParticlePool.OpenedHandleCount);
        Assert.Equal(freed + 8, RustParticlePool.FreedHandleCount);
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static void Abandon() => _ = RustParticlePool.Open(Fixture());
    [RequiresRustParticlePoolFact]
    public void SafeHandle_finalizer_frees_abandoned_pool_once()
    {
        long freed = RustParticlePool.FreedHandleCount;
        Abandon();
        for (int i = 0; i < 10 && RustParticlePool.FreedHandleCount == freed; i++)
        {
            GC.Collect(); GC.WaitForPendingFinalizers(); GC.Collect();
        }
        Assert.Equal(freed + 1, RustParticlePool.FreedHandleCount);
    }

    // Bind test-only delegates through the same loaded Api; no second resolver or NativeLibrary.Load.
    private static T Export<T>(string name) where T : Delegate
    {
        var getApi = typeof(RustNativeData).GetMethod("GetApi", BindingFlags.Static | BindingFlags.NonPublic)!;
        object api = getApi.Invoke(null, null)!;
        var bind = api.GetType().GetMethod("BindExport", BindingFlags.Instance | BindingFlags.NonPublic)!;
        return (T)bind.MakeGenericMethod(typeof(T)).Invoke(api, [name])!;
    }
    [StructLayout(LayoutKind.Sequential)]
    private struct Metadata
    {
        public uint Size, Slot, Active;
        public int Count;
        public short Owner;
        public ushort Reserved;
        public int Age;
    }
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate int OpenFn(byte[] bytes, nuint size, out IntPtr handle);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate void FreeFn(IntPtr handle);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate int SnapshotFn(IntPtr handle, uint slot,
        [In, Out] ref Metadata metadata, [In, Out] byte[] record, nuint recordCap, [In, Out] byte[] state, nuint stateCap);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate int ChildFn(IntPtr handle, uint slot, uint ordinal, [In, Out] byte[] bytes, nuint cap);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate int CopyFn(IntPtr handle, [In, Out] byte[]? bytes, nuint cap, out nuint size);

    [RequiresRustParticlePoolFact]
    public void Native_snapshot_and_child_errors_are_atomic_and_copy_caps_never_write_short_buffers()
    {
        byte[] p = Fixture();
        var open = Export<OpenFn>("ar_particle_pool_open"); var free = Export<FreeFn>("ar_particle_pool_free");
        Assert.Equal(0, open(p, (nuint)p.Length, out IntPtr handle));
        Assert.NotEqual(IntPtr.Zero, handle);
        try
        {
            var snapshot = Export<SnapshotFn>("ar_particle_pool_snapshot_any");
            var success = new Metadata { Size = 24 };
            byte[] successRecord = new byte[2252], successState = new byte[16];
            Assert.Equal(0, snapshot(handle, 1023, ref success, successRecord, 2252, successState, 16));
            Assert.Equal(24u, success.Size); Assert.Equal(1023u, success.Slot); Assert.Equal((ushort)0, success.Reserved);
            foreach (var (size, slot, recordCap, stateCap, expectedCode) in new[]
            { (23u, 0u, 2252, 16, 3), (24u, 1024u, 2252, 16, 4),
              (24u, 0u, 2251, 16, 2), (24u, 0u, 2252, 15, 2) })
            {
                var m = new Metadata { Size = size, Slot = 77, Active = 0xff, Count = -7, Owner = -9, Reserved = 9, Age = -11 };
                Metadata before = m;
                byte[] record = Enumerable.Repeat((byte)0xa5, 2252).ToArray(), state = Enumerable.Repeat((byte)0x5a, 16).ToArray();
                Assert.Equal(expectedCode, snapshot(handle, slot, ref m, record, (nuint)recordCap, state, (nuint)stateCap));
                Assert.Equal(before, m);
                Assert.All(record, b => Assert.Equal(0xa5, b)); Assert.All(state, b => Assert.Equal(0x5a, b));
            }
            var activeOnly = Export<SnapshotFn>("ar_particle_pool_snapshot");
            var inactive = new Metadata { Size = 24, Count = -123 };
            byte[] r = Enumerable.Repeat((byte)3, 2252).ToArray(), s = Enumerable.Repeat((byte)4, 16).ToArray();
            Assert.Equal(5, activeOnly(handle, 0, ref inactive, r, 2252, s, 16));
            Assert.Equal(-123, inactive.Count); Assert.All(r, b => Assert.Equal(3, b)); Assert.All(s, b => Assert.Equal(4, b));
            var child = Export<ChildFn>("ar_particle_pool_child_any");
            foreach (var (slot, ordinal, cap, code) in new[] { (1024u, 0u, 35, 4), (0u, 64u, 35, 11), (0u, 0u, 34, 2) })
            {
                byte[] b = Enumerable.Repeat((byte)0xa5, 35).ToArray();
                Assert.Equal(code, child(handle, slot, ordinal, b, (nuint)cap)); Assert.All(b, v => Assert.Equal(0xa5, v));
            }
            foreach (var (name, required) in new[] { ("ar_particle_pool_copy_payload", PayloadSize), ("ar_particle_pool_copy_input_sha256", 32) })
            {
                var copy = Export<CopyFn>(name);
                Assert.Equal(2, copy(handle, null, 0, out nuint size)); Assert.Equal((nuint)required, size);
                byte[] shortBuffer = Enumerable.Repeat((byte)0xa5, required - 1).ToArray();
                Assert.Equal(2, copy(handle, shortBuffer, (nuint)shortBuffer.Length, out size));
                Assert.Equal((nuint)required, size); Assert.True(shortBuffer.All(b => b == 0xa5), $"{name} mutated a short buffer.");
                byte[] oversized = Enumerable.Repeat((byte)0xa5, required + 8).ToArray();
                Assert.Equal(0, copy(handle, oversized, (nuint)oversized.Length, out size));
                Assert.Equal((nuint)required, size);
                Bytes(name.EndsWith("payload", StringComparison.Ordinal) ? p : SHA256.HashData(p),
                    oversized.AsSpan(0, required), name);
                Assert.All(oversized[required..], b => Assert.Equal(0xa5, b));
            }
            var headerCopy = Export<CopyFn>("ar_particle_pool_copy_pfil_header");
            Assert.Equal(0, headerCopy(handle, null, 0, out nuint headerSize)); Assert.Equal((nuint)0, headerSize);
        }
        finally { free(handle); }
        byte[] packed = Wrapped(p);
        Assert.Equal(0, open(packed, (nuint)packed.Length, out handle));
        try
        {
            var headerCopy = Export<CopyFn>("ar_particle_pool_copy_pfil_header");
            Assert.Equal(2, headerCopy(handle, null, 0, out nuint size)); Assert.Equal((nuint)64, size);
            byte[] header = Enumerable.Repeat((byte)0xa5, 72).ToArray();
            Assert.Equal(2, headerCopy(handle, header, 63, out size));
            Assert.Equal((nuint)64, size); Assert.All(header, b => Assert.Equal(0xa5, b));
            Assert.Equal(0, headerCopy(handle, header, (nuint)header.Length, out size));
            Assert.Equal((nuint)64, size); Bytes(packed.AsSpan(0, 64), header.AsSpan(0, 64), "native header copy");
            Assert.All(header[64..], b => Assert.Equal(0xa5, b));
        }
        finally { free(handle); }
    }

    private static void Bytes(ReadOnlySpan<byte> expected, ReadOnlySpan<byte> actual, string label)
        => Assert.True(expected.SequenceEqual(actual), $"{label}: byte mismatch (expected {expected.Length}, actual {actual.Length}).");

    private static void Differential(byte[] image, NativeParticlePool csharp, string label)
    {
        byte[] payload = GameLZSS.DecompressPfil(image);
        Bytes(payload, csharp.ToPayload(), $"{label}: C# payload");
        using var rust = RustParticlePool.Open(image);
        Bytes(payload, rust.CopyPayload(), $"{label}: Rust payload");
        Bytes(SHA256.HashData(image), rust.CopyInputSha256(), $"{label}: source digest");
        int active = 0;
        for (int slot = 0; slot < Capacity; slot++)
        {
            int o = RecordOffset(slot); var any = rust.CaptureAny(slot);
            Bytes(payload.AsSpan(o, RecordSize), any.RawRecord, $"{label}: slot {slot} record2252");
            Bytes(StateBytes(payload, slot), any.RawState, $"{label}: slot {slot} all four columns");
            Assert.Equal((uint)BinaryPrimitives.ReadUInt16LittleEndian(payload.AsSpan(o)), any.ActiveRaw);
            Assert.Equal(BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(o + 2)), any.CountRaw);
            Assert.Equal(BinaryPrimitives.ReadInt16LittleEndian(payload.AsSpan(o + 6)), any.Owner);
            Assert.Equal(BinaryPrimitives.ReadInt32LittleEndian(payload.AsSpan(o + 8)), any.AgeRaw);
            var native = csharp.CaptureSnapshot(slot); var captured = rust.Capture(slot);
            Assert.Equal(native is not null, captured is not null);
            if (native is null) continue;
            active++;
            Bytes(native.RecordBytes(), captured!.RawRecord, $"{label}: C# active record {slot}");
            Assert.Equal(native.OwnerObject, captured.Owner);
            int[] words = native.StateWords();
            for (int c = 0; c < 4; c++) Assert.Equal(words[c], BinaryPrimitives.ReadInt32LittleEndian(captured.RawState[(c * 4)..]));
        }
        Assert.Equal(active, rust.ActiveCount);
    }

    private sealed record SourceStat(string Path, long Length, long MtimeTicks, string Sha256);
    private static SourceStat Stat(string path)
    {
        var info = new FileInfo(path);
        return new(path, info.Length, info.LastWriteTimeUtc.Ticks, Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path))));
    }

    [RequiresInstalledRustParticlePoolFact]
    public void Installed_MAPS_and_SAVE_entire_payload_all1024_records_four_columns_and_sources_match()
    {
        string? root = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        Assert.False(string.IsNullOrWhiteSpace(root), "ARM_GAME_PATH is required for the installed differential gate.");
        Assert.True(Directory.Exists(root), $"Installed game root does not exist: {root}");
        List<string> Find(string folder) => !Directory.Exists(Path.Combine(root!, folder)) ? []
            : Directory.EnumerateDirectories(Path.Combine(root!, folder)).Select(d => Path.Combine(d, "DATA", "particle.dat"))
                .Where(File.Exists).OrderBy(p => p, StringComparer.OrdinalIgnoreCase).ToList();
        var maps = Find("MAPS"); var saves = Find("SAVE");
        Assert.Equal(74, maps.Count); Assert.Single(saves);
        string[] files = [.. maps, .. saves];
        var before = files.Select(Stat).ToArray();
        try
        {
            foreach (string path in files)
            {
                byte[] image = File.ReadAllBytes(path);
                Differential(image, NativeParticlePool.Load(path), path);
                using var loaded = RustParticlePool.Load(path);
                Bytes(GameLZSS.DecompressPfil(image), loaded.CopyPayload(), $"{path}: Load");
                Bytes(image.AsSpan().StartsWith("PFIL"u8) ? image.AsSpan(0, 64) : ReadOnlySpan<byte>.Empty,
                    loaded.CopyPfilHeader(), $"{path}: PFIL header");
            }
        }
        finally { Assert.Equal(before, files.Select(Stat).ToArray()); }
        Assert.Equal(75, files.Length);
    }
}
