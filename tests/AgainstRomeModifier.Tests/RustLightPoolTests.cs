using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Security.Cryptography;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

/// <summary>Runs only when the real Rust DLL loads; with ARM_REQUIRE_NATIVE_DATA=1 it never skips (so it fails instead).</summary>
internal sealed class RequiresNativeDataFactAttribute : FactAttribute
{
    public RequiresNativeDataFactAttribute()
    {
        if (RustLightPoolTests.Required) return;
        if (!RustNativeData.TryEnsureLoaded(out string? error))
            Skip = $"Requires {RustNativeData.EnvironmentVariable} (absolute path to against_rome_data.dll): {error}";
    }
}

/// <summary>Additionally needs ARM_GAME_PATH (installed game root; read-only).</summary>
internal sealed class RequiresNativeInstalledFactAttribute : FactAttribute
{
    public RequiresNativeInstalledFactAttribute()
    {
        if (RustLightPoolTests.Required) return;
        if (!RustNativeData.TryEnsureLoaded(out string? error))
            Skip = $"Requires {RustNativeData.EnvironmentVariable}: {error}";
        else if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("ARM_GAME_PATH")))
            Skip = "Requires opt-in ARM_GAME_PATH; no proprietary files in Git.";
    }
}

/// <summary>Only meaningful as a gate: skipped unless ARM_REQUIRE_NATIVE_DATA=1.</summary>
internal sealed class RequiresNativeGateFactAttribute : FactAttribute
{
    public RequiresNativeGateFactAttribute()
    {
        if (!RustLightPoolTests.Required) Skip = "Gate only runs with ARM_REQUIRE_NATIVE_DATA=1.";
    }
}

/// <summary>Real FFI tests (no mocks) of the C# bridge against against_rome_data.dll. One class: handle counters are process-global.</summary>
[CollectionDefinition("Rust native interop", DisableParallelization = true)]
public sealed class RustNativeInteropTestGroup { }

[Collection("Rust native interop")]
public sealed class RustLightPoolTests
{
    internal static bool Required => Environment.GetEnvironmentVariable("ARM_REQUIRE_NATIVE_DATA") == "1";

    private static readonly int[] Widths = [4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 4, 4, 4, 2, 2, 2];
    private const int Capacity = 1024;
    private const int PayloadLength = 57352;

    private static int ColumnOffset(int column)
    {
        int offset = 8;
        for (int i = 0; i < column; i++) offset += Capacity * Widths[i];
        return offset;
    }

    private static byte[] EmptyPayload()
    {
        byte[] p = new byte[PayloadLength];
        BinaryPrimitives.WriteInt32LittleEndian(p, 1);
        BinaryPrimitives.WriteInt32LittleEndian(p.AsSpan(4), Capacity);
        for (int c = 13; c < 16; c++) p.AsSpan(ColumnOffset(c), Capacity * 2).Fill(0xFF);
        return p;
    }

    private static void Put(byte[] p, int column, int slot, ReadOnlySpan<byte> bytes)
        => bytes.CopyTo(p.AsSpan(ColumnOffset(column) + slot * Widths[column]));

    private static void PutF(byte[] p, int column, int slot, float v)
    {
        Span<byte> b = stackalloc byte[4]; BinaryPrimitives.WriteSingleLittleEndian(b, v); Put(p, column, slot, b);
    }

    private static void PutBits(byte[] p, int column, int slot, uint bits)
    {
        Span<byte> b = stackalloc byte[4]; BinaryPrimitives.WriteUInt32LittleEndian(b, bits); Put(p, column, slot, b);
    }

    private static void PutS(byte[] p, int column, int slot, ushort v)
    {
        Span<byte> b = stackalloc byte[2]; BinaryPrimitives.WriteUInt16LittleEndian(b, v); Put(p, column, slot, b);
    }

    /// <summary>Slot 3: active=7, RGB&gt;1, effective != baseline, sNaN z, unusual type/age/links.</summary>
    private static byte[] Sample()
    {
        byte[] p = EmptyPayload();
        PutBits(p, 0, 3, 7); PutF(p, 1, 3, 1.5f); PutF(p, 2, 3, -2.0f); PutBits(p, 3, 3, 0x7FA00001);
        PutF(p, 4, 3, 2.5f); PutF(p, 5, 3, 0.25f); PutF(p, 6, 3, 0.5f);
        PutF(p, 7, 3, 300.0f); PutF(p, 8, 3, 450.0f);
        PutS(p, 9, 3, 0xFFFE); PutF(p, 10, 3, 0.15f); PutBits(p, 11, 3, 0xFFFFFFFF); PutBits(p, 12, 3, 0xDEADBEEF);
        PutS(p, 13, 3, unchecked((ushort)-5)); PutS(p, 14, 3, 12); PutS(p, 15, 3, 39);
        return p;
    }

    private static byte[] LiteralPfil(byte[] payload, byte headerMarker = 0)
    {
        var o = new List<byte>(new byte[64]);
        "PFIL"u8.CopyTo(System.Runtime.InteropServices.CollectionsMarshal.AsSpan(o));
        o[8] = headerMarker;
        BinaryPrimitives.WriteInt32LittleEndian(System.Runtime.InteropServices.CollectionsMarshal.AsSpan(o).Slice(16), payload.Length);
        for (int i = 0; i < payload.Length; i += 8)
        {
            int n = Math.Min(8, payload.Length - i);
            o.Add((byte)((1 << n) - 1)); // low bits set = literals
            for (int k = 0; k < n; k++) o.Add(payload[i + k]);
        }
        return o.ToArray();
    }

    private static byte[] ColumnRaw(byte[] payload, int slot)
    {
        byte[] raw = new byte[56]; int at = 0;
        for (int c = 0; c < 16; c++)
        {
            payload.AsSpan(ColumnOffset(c) + slot * Widths[c], Widths[c]).CopyTo(raw.AsSpan(at));
            at += Widths[c];
        }
        return raw;
    }

    private static void AssertBytes(ReadOnlySpan<byte> expected, ReadOnlySpan<byte> actual, string label)
        => Assert.True(expected.SequenceEqual(actual), $"bytes differ: {label}");

    private static uint Bits(float f) => BitConverter.SingleToUInt32Bits(f);

    private static void AssertSnapshotMatchesRaw(RustLightSnapshot s, byte[] raw, int slot)
    {
        uint U32(int o) => BinaryPrimitives.ReadUInt32LittleEndian(raw.AsSpan(o));
        Assert.Equal(slot, s.Slot);
        AssertBytes(raw, s.RawFields, $"slot {slot} raw56");
        Assert.Equal(U32(0), s.ActiveRaw);
        Assert.Equal(U32(4), Bits(s.Position.X)); Assert.Equal(U32(8), Bits(s.Position.Y)); Assert.Equal(U32(12), Bits(s.Position.Z));
        Assert.Equal(U32(16), Bits(s.Color.X)); Assert.Equal(U32(20), Bits(s.Color.Y)); Assert.Equal(U32(24), Bits(s.Color.Z));
        Assert.Equal(U32(28), Bits(s.EffectiveRadius)); Assert.Equal(U32(32), Bits(s.BaselineRadius));
        Assert.Equal(BinaryPrimitives.ReadUInt16LittleEndian(raw.AsSpan(36)), s.TypeRaw);
        Assert.Equal(U32(38), Bits(s.TypeParameter));
        Assert.Equal(U32(42), s.AgeRaw); Assert.Equal(unchecked((int)U32(42)), s.AgeSigned);
        Assert.Equal(U32(46), s.SpecialFxRaw);
        Assert.Equal(BinaryPrimitives.ReadInt16LittleEndian(raw.AsSpan(50)), s.OwnerObject);
        Assert.Equal(BinaryPrimitives.ReadInt16LittleEndian(raw.AsSpan(52)), s.ParentObject);
        Assert.Equal(BinaryPrimitives.ReadInt16LittleEndian(raw.AsSpan(54)), s.Definition);
        Assert.Equal((U32(4), U32(8), U32(12)), s.PositionBits);
        Assert.Equal((U32(16), U32(20), U32(24)), s.ColorBits);
        Assert.Equal(U32(28), s.EffectiveRadiusBits); Assert.Equal(U32(32), s.BaselineRadiusBits);
        Assert.Equal(U32(38), s.TypeParameterBits);
    }

    // ---- gate / ABI ----

    [RequiresNativeGateFact]
    public void Required_mode_demands_an_explicit_existing_native_dll()
    {
        string? env = Environment.GetEnvironmentVariable(RustNativeData.EnvironmentVariable);
        Assert.False(string.IsNullOrWhiteSpace(env), $"{RustNativeData.EnvironmentVariable} must be set explicitly.");
        Assert.True(Path.IsPathFullyQualified(env!), "must be an absolute path");
        Assert.True(File.Exists(env), $"missing {env}");
        Assert.True(RustNativeData.TryEnsureLoaded(out string? error), error);
        Assert.Equal(Path.GetFullPath(env!), RustNativeData.LoadedLibraryPath);
    }

    [RequiresNativeDataFact]
    public void Abi_version_struct_size_and_field_offsets_match_the_header()
    {
        Assert.Equal(1u, RustNativeData.NativeAbiVersion);
        Assert.Equal(120u, RustNativeData.NativeSnapshotStructSize);
        Assert.Equal(120, RustNativeData.ManagedSnapshotStructSize);
        var header = new Dictionary<string, int> // natural C alignment of ArLightSnapshot
        {
            ["struct_size"] = 0, ["slot"] = 4, ["active_raw"] = 8, ["position"] = 12, ["color"] = 24,
            ["effective_radius"] = 36, ["baseline_radius"] = 40, ["type_parameter"] = 44, ["age_raw"] = 48,
            ["special_fx_raw"] = 52, ["type_raw"] = 56, ["owner"] = 58, ["parent"] = 60, ["definition"] = 62,
            ["raw_fields"] = 64,
        };
        Assert.Equal(header.OrderBy(p => p.Key), RustNativeData.ManagedFieldOffsets.OrderBy(p => p.Key));
        Assert.Equal(header.OrderBy(p => p.Key), RustNativeData.ExpectedFieldOffsets.OrderBy(p => p.Key));
        Assert.True(Path.IsPathFullyQualified(RustNativeData.LoadedLibraryPath));
    }

    // ---- data ----

    [RequiresNativeDataFact]
    public void Raw_payload_exposes_every_field_and_all_56_bytes_bit_for_bit()
    {
        byte[] payload = Sample();
        using var pool = RustLightPool.Open(payload);
        Assert.Equal(1, pool.ActiveCount);
        var s = pool.Capture(3)!;
        Assert.NotNull(s);
        Assert.Equal(7u, s.ActiveRaw);
        Assert.True(s.IsActive);
        Assert.Equal(1.5f, s.Position.X); Assert.Equal(-2.0f, s.Position.Y);
        Assert.Equal(0x7FA00001u, s.PositionBits.Z);
        Assert.Equal(new System.Numerics.Vector3(2.5f, 0.25f, 0.5f), s.Color); // RGB > 1 not clamped
        Assert.Equal(300.0f, s.EffectiveRadius); Assert.Equal(450.0f, s.BaselineRadius); // unequal radii
        Assert.Equal(0xFFFE, s.TypeRaw); Assert.Equal(0.15f, s.TypeParameter);
        Assert.Equal(0xFFFFFFFFu, s.AgeRaw); Assert.Equal(-1, s.AgeSigned);
        Assert.Equal(0xDEADBEEFu, s.SpecialFxRaw);
        Assert.Equal((short)-5, s.OwnerObject); Assert.Equal((short)12, s.ParentObject); Assert.Equal((short)39, s.Definition);
        AssertSnapshotMatchesRaw(s, ColumnRaw(payload, 3), 3);
        AssertBytes(payload, pool.CopyPayload(), "payload");
        Assert.Empty(pool.CopyPfilHeader());
    }

    [RequiresNativeDataFact]
    public void Random_full_pool_returns_exact_raw_bytes_for_all_1024_slots_including_inactive()
    {
        byte[] payload = EmptyPayload();
        var rng = new Random(20261012);
        int end = ColumnOffset(15) + Capacity * 2;
        rng.NextBytes(payload.AsSpan(8, end - 8));
        BinaryPrimitives.WriteInt32LittleEndian(payload, 1);
        BinaryPrimitives.WriteInt32LittleEndian(payload.AsSpan(4), Capacity);
        for (int slot = 0; slot < Capacity; slot += 5) PutBits(payload, 0, slot, 0); // sprinkle inactive slots
        using var pool = RustLightPool.Open(payload);
        int active = 0;
        for (int slot = 0; slot < Capacity; slot++)
        {
            byte[] raw = ColumnRaw(payload, slot);
            AssertSnapshotMatchesRaw(pool.CaptureAny(slot), raw, slot);
            var active1 = pool.Capture(slot);
            if (BinaryPrimitives.ReadUInt32LittleEndian(raw) == 0) Assert.Null(active1);
            else { Assert.NotNull(active1); AssertSnapshotMatchesRaw(active1!, raw, slot); active++; }
        }
        Assert.Equal(active, pool.ActiveCount);
        AssertBytes(payload, pool.CopyPayload(), "payload");
    }

    [RequiresNativeDataFact]
    public void Inactive_slot_is_null_for_Capture_but_preserved_by_CaptureAny_and_bad_slots_throw()
    {
        byte[] payload = EmptyPayload();
        PutF(payload, 7, 9, 123.0f);
        using var pool = RustLightPool.Open(payload);
        Assert.Null(pool.Capture(9));
        Assert.Equal(123.0f, pool.CaptureAny(9).EffectiveRadius);
        Assert.False(pool.CaptureAny(9).IsActive);
        foreach (int bad in new[] { -1, Capacity, Capacity + 1, int.MaxValue, int.MinValue })
        {
            Assert.Throws<ArgumentOutOfRangeException>(() => pool.Capture(bad));
            Assert.Throws<ArgumentOutOfRangeException>(() => pool.CaptureAny(bad));
        }
    }

    [RequiresNativeDataFact]
    public void Pfil_keeps_the_original_header_decoded_payload_and_matches_GameLZSS()
    {
        byte[] payload = Sample();
        byte[] file = LiteralPfil(payload, 0xAB);
        using var pool = RustLightPool.Open(file);
        AssertBytes(payload, pool.CopyPayload(), "payload");
        AssertBytes(file.AsSpan(0, 64), pool.CopyPfilHeader(), "pfil header");
        AssertBytes(GameLZSS.DecompressPfil(file), pool.CopyPayload(), "GameLZSS vs rust");
        AssertSnapshotMatchesRaw(pool.Capture(3)!, ColumnRaw(payload, 3), 3);
        AssertRustMatchesCSharp(file, NativeLightPool.ParsePayload(GameLZSS.DecompressPfil(file)), "synthetic pfil");
    }

    [RequiresNativeDataFact]
    public void Open_copies_the_input_so_later_caller_mutation_cannot_change_the_pool()
    {
        byte[] payload = Sample();
        using var pool = RustLightPool.Open(payload);
        payload.AsSpan().Clear();
        Assert.Equal(1, pool.ActiveCount);
        Assert.Equal(7u, pool.Capture(3)!.ActiveRaw);
        byte[] copy = pool.CopyPayload(); copy[0] = 99; // returned buffers are caller-owned copies
        Assert.Equal(1, pool.CopyPayload()[0]);
    }

    [RequiresNativeDataFact]
    public void Snapshots_are_immutable_detached_copies()
    {
        using var pool = RustLightPool.Open(Sample());
        var s = pool.Capture(3)!;
        Assert.All(typeof(RustLightSnapshot).GetProperties(BindingFlags.Public | BindingFlags.Instance),
            p => Assert.True(p.GetSetMethod(nonPublic: true) is null, $"{p.Name} has a setter"));
        byte[] copy = s.ToRawBytes(); copy[0] ^= 0xFF;
        Assert.Equal(7u, s.ActiveRaw);
        Assert.Equal((byte)7, s.RawFields[0]);
        byte[] before = s.ToRawBytes();
        pool.Dispose(); // snapshot outlives the pool
        AssertBytes(before, s.RawFields, "snapshot after dispose");
    }

    // ---- invalid input ----

    [RequiresNativeDataFact]
    public void Invalid_inputs_throw_InvalidDataException_and_never_leave_handles()
    {
        byte[] good = Sample();
        byte[] version = good.ToArray(); version[0] = 2;
        byte[] capacity = good.ToArray(); capacity[5] = 8;
        byte[] pfil = LiteralPfil(good);
        byte[] declared = pfil.ToArray(); BinaryPrimitives.WriteInt32LittleEndian(declared.AsSpan(16), good.Length - 1);
        var cases = new Dictionary<string, byte[]>
        {
            ["empty"] = [],
            ["one byte"] = [1],
            ["short payload"] = good[..^1],
            ["long payload"] = [.. good, 0],
            ["version 2"] = version,
            ["capacity 2048"] = capacity,
            ["pfil magic only"] = "PFIL"u8.ToArray(),
            ["pfil header truncated"] = pfil[..63],
            ["pfil declared size"] = declared,
            ["lzss truncated"] = pfil[..^1],
            ["lzss trailing input"] = [.. pfil, 0],
        };
        long opened = RustLightPool.OpenedHandleCount, freed = RustLightPool.FreedHandleCount;
        foreach (var (_, bytes) in cases)
        {
            var ex = Assert.Throws<InvalidDataException>(() => RustLightPool.Open(bytes));
            Assert.Contains("ar_light_pool_open", ex.Message);
        }
        Assert.Equal(opened, RustLightPool.OpenedHandleCount);
        Assert.Equal(freed, RustLightPool.FreedHandleCount);
    }

    // ---- lifetime ----

    [RequiresNativeDataFact]
    public void After_dispose_every_member_throws_and_dispose_is_idempotent()
    {
        long opened = RustLightPool.OpenedHandleCount, freed = RustLightPool.FreedHandleCount;
        var pool = RustLightPool.Open(LiteralPfil(Sample()));
        Assert.False(pool.IsDisposed);
        pool.Dispose(); pool.Dispose();
        Assert.True(pool.IsDisposed);
        Assert.Throws<ObjectDisposedException>(() => pool.ActiveCount);
        Assert.Throws<ObjectDisposedException>(() => pool.Capture(3));
        Assert.Throws<ObjectDisposedException>(() => pool.CaptureAny(3));
        Assert.Throws<ObjectDisposedException>(() => pool.CopyPayload());
        Assert.Throws<ObjectDisposedException>(() => pool.CopyPfilHeader());
        pool.Dispose();
        Assert.Equal(opened + 1, RustLightPool.OpenedHandleCount);
        Assert.Equal(freed + 1, RustLightPool.FreedHandleCount); // freed exactly once
    }

    [RequiresNativeDataFact]
    public void Concurrent_dispose_frees_once_and_in_flight_calls_never_crash()
    {
        long opened = RustLightPool.OpenedHandleCount, freed = RustLightPool.FreedHandleCount;
        for (int round = 0; round < 20; round++)
        {
            var pool = RustLightPool.Open(Sample());
            var errors = new ConcurrentBag<Exception>();
            Task[] readers = Enumerable.Range(0, 4).Select(_ => Task.Run(() =>
            {
                for (int i = 0; i < 500; i++)
                {
                    try { Assert.Equal(7u, pool.Capture(3)!.ActiveRaw); pool.CopyPayload(); }
                    catch (ObjectDisposedException) { break; }
                    catch (Exception e) { errors.Add(e); break; }
                }
            })).ToArray();
            Task[] disposers = Enumerable.Range(0, 3).Select(_ => Task.Run(pool.Dispose)).ToArray();
            Task.WaitAll(readers.Concat(disposers).ToArray());
            Assert.Empty(errors);
            Assert.True(pool.IsDisposed);
        }
        Assert.Equal(opened + 20, RustLightPool.OpenedHandleCount);
        Assert.Equal(freed + 20, RustLightPool.FreedHandleCount);
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static void OpenAndAbandon(byte[] image) => _ = RustLightPool.Open(image);

    [RequiresNativeDataFact]
    public void Abandoned_pool_is_freed_once_by_the_safehandle_finalizer()
    {
        long freed = RustLightPool.FreedHandleCount;
        OpenAndAbandon(Sample());
        for (int i = 0; i < 10 && RustLightPool.FreedHandleCount == freed; i++)
        {
            GC.Collect(); GC.WaitForPendingFinalizers(); GC.Collect();
        }
        Assert.Equal(freed + 1, RustLightPool.FreedHandleCount);
    }

    // ---- differential: Rust vs GameLZSS / NativeLightPool ----

    private static void AssertRustMatchesCSharp(byte[] file, NativeLightPool csharp, string label)
    {
        byte[] expected = GameLZSS.DecompressPfil(file);
        AssertBytes(expected, csharp.ToPayload(), $"{label}: C# pool payload");
        using var rust = RustLightPool.Open(file);
        AssertBytes(expected, rust.CopyPayload(), $"{label}: rust vs GameLZSS payload");
        bool wrapped = file.Length >= 4 && file.AsSpan(0, 4).SequenceEqual("PFIL"u8);
        AssertBytes(wrapped ? file.AsSpan(0, 64) : ReadOnlySpan<byte>.Empty, rust.CopyPfilHeader(), $"{label}: header");
        var lights = csharp.ActiveLights().ToDictionary(l => l.Slot);
        Assert.Equal(lights.Count, rust.ActiveCount);
        for (int slot = 0; slot < Capacity; slot++)
        {
            byte[] raw = ColumnRaw(expected, slot);
            var any = rust.CaptureAny(slot);
            AssertBytes(raw, any.RawFields, $"{label}: slot {slot} raw56");
            AssertSnapshotMatchesRaw(any, raw, slot);
            var active = rust.Capture(slot);
            Assert.Equal(csharp.Capture(slot) is not null, active is not null);
            Assert.Equal(lights.ContainsKey(slot), active is not null);
            if (active is null) continue;
            var l = lights[slot];
            Assert.Equal(l.Active, active.ActiveRaw);
            Assert.Equal((Bits(l.Position.X), Bits(l.Position.Y), Bits(l.Position.Z)), active.PositionBits);
            Assert.Equal(l.OwnerObject, active.OwnerObject);
            Assert.Equal(l.ParentObject, active.ParentObject);
            Assert.Equal(l.Definition, active.Definition);
        }
    }

    private sealed record SourceStat(string Path, long Length, long LastWriteTicks, string Sha256);

    private static SourceStat Stat(string path)
    {
        var info = new FileInfo(path);
        return new(path, info.Length, info.LastWriteTimeUtc.Ticks, Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path))));
    }

    [RequiresNativeInstalledFact]
    public void Installed_74_maps_and_save_match_GameLZSS_and_CSharp_pool_slot_by_slot_with_sources_unchanged()
    {
        string root = Environment.GetEnvironmentVariable("ARM_GAME_PATH")!;
        List<string> Find(string folder) => !Directory.Exists(Path.Combine(root, folder)) ? new List<string>()
            : Directory.EnumerateDirectories(Path.Combine(root, folder)).Select(d => Path.Combine(d, "DATA", "light.dat"))
                .Where(File.Exists).OrderBy(p => p, StringComparer.OrdinalIgnoreCase).ToList();
        var maps = Find("MAPS"); var saves = Find("SAVE");
        Assert.Equal(74, maps.Count);
        Assert.Single(saves);
        string[] files = [.. maps, .. saves];

        var before = files.Select(Stat).ToList();
        foreach (string path in files)
        {
            byte[] file = File.ReadAllBytes(path);
            AssertRustMatchesCSharp(file, NativeLightPool.Load(path), path);
        }
        var after = files.Select(Stat).ToList();
        Assert.Equal(before, after); // length, mtime and SHA-256 unchanged
        Assert.Equal(75, after.Count);
    }
}
