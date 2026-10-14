using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;

namespace AgainstRomeMapEditor.NativeAssets;

internal sealed record NativeParticleRenderDefinition(int OwnerType, int ParameterId, int GeometryId,
    int GrafikId, int GrafikObject, int AlrId, int Lifetime, int TypeDivisor, int VelocityBranchRaw, int SkipFirstRaw);

/// <summary>Rust definition catalogs from caller base images or checked native loose source order.</summary>
internal sealed partial class RustParticleBaseDefinitions : IDisposable
{
    [StructLayout(LayoutKind.Sequential)]
    private struct NativeDefinition
    {
        public uint StructSize;
        public int OwnerType, ParameterId, GeometryId, GrafikId, GrafikObject, AlrId,
            Lifetime, TypeDivisor, VelocityBranchRaw, SkipFirstRaw;
    }
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate uint SizeFn();
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate int OpenFn(
        [In] byte[] parameters, nuint parameterLength, [In] byte[] geometry, nuint geometryLength,
        [In] byte[] grafik, nuint grafikLength, [In] byte[] objects, nuint objectLength, out IntPtr handle);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate void FreeFn(IntPtr handle);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate int LoadFn([In] byte[] root, nuint length, out IntPtr handle);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate int CountFn(IntPtr handle, out uint count);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate int FrameFn(IntPtr handle, int ownerType,
        int childLifetime, int alrId, int columns, int directions, uint frameCount, ref uint output);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] private delegate int QueryFn(IntPtr handle, int ownerType,
        [In, Out] ref NativeDefinition output);
    private readonly object _gate = new();
    private readonly DefinitionHandle _handle;
    private readonly QueryFn _query;
    private FrameFn? _frame;
    private readonly IntPtr _library;
    internal uint SourceFilesRead { get; }

    internal static RustParticleBaseDefinitions LoadGameSources(string gameRoot)
    {
        ArgumentNullException.ThrowIfNull(gameRoot);
        if (!Path.IsPathFullyQualified(gameRoot)) throw new ArgumentException("Game root must be absolute.", nameof(gameRoot));
        return new(null, null, null, null, Encoding.UTF8.GetBytes(gameRoot));
    }

    internal RustParticleBaseDefinitions(byte[] parameters, byte[] geometry, byte[] grafik, byte[] objects)
        : this(parameters, geometry, grafik, objects, null) { }

    private RustParticleBaseDefinitions(byte[]? parameters, byte[]? geometry, byte[]? grafik, byte[]? objects, byte[]? root)
    {
        if (root is null)
        {
            ArgumentNullException.ThrowIfNull(parameters); ArgumentNullException.ThrowIfNull(geometry);
            ArgumentNullException.ThrowIfNull(grafik); ArgumentNullException.ThrowIfNull(objects);
        }
        string? configured = Environment.GetEnvironmentVariable("ARM_NATIVE_DATA_DLL");
        if (!string.IsNullOrWhiteSpace(configured) && !Path.IsPathFullyQualified(configured))
            throw new InvalidDataException("ARM_NATIVE_DATA_DLL must be an absolute path.");
        string path = string.IsNullOrWhiteSpace(configured)
            ? Path.Combine(AppContext.BaseDirectory, "against_rome_data.dll") : Path.GetFullPath(configured);
        IntPtr library = NativeLibrary.Load(path), native = IntPtr.Zero;
        FreeFn? free = null;
        try
        {
            T Bind<T>(string name) where T : Delegate => Marshal.GetDelegateForFunctionPointer<T>(NativeLibrary.GetExport(library, name));
            if (Bind<SizeFn>("ar_particle_definition_abi_version")() != 1
                || Bind<SizeFn>("ar_particle_definition_struct_size")() != 44 || Marshal.SizeOf<NativeDefinition>() != 44)
                throw new InvalidDataException("Rust particle definitions ABI mismatch.");
            free = Bind<FreeFn>("ar_particle_definition_free");
            _query = Bind<QueryFn>("ar_particle_definition_query");
            int code;
            if (root is null)
            {
                ArgumentNullException.ThrowIfNull(parameters); ArgumentNullException.ThrowIfNull(geometry);
                ArgumentNullException.ThrowIfNull(grafik); ArgumentNullException.ThrowIfNull(objects);
                code = Bind<OpenFn>("ar_particle_definition_open")(parameters, (nuint)parameters.Length,
                    geometry, (nuint)geometry.Length, grafik, (nuint)grafik.Length, objects, (nuint)objects.Length, out native);
            }
            else code = Bind<LoadFn>("ar_particle_definition_load_utf8")(root, (nuint)root.Length, out native);
            if (code != 0) throw new InvalidDataException($"Rust base particle definitions rejected (code={code}).");
            if (Bind<CountFn>("ar_particle_definition_source_count")(native, out uint count) != 0)
                throw new InvalidDataException("Rust definition source count unavailable.");
            SourceFilesRead = count;
            _handle = new DefinitionHandle(native, library, free);
            _library = library;
        }
        catch
        {
            if (native != IntPtr.Zero && free is not null) free(native);
            NativeLibrary.Free(library);
            throw;
        }
    }

    internal bool TryResolveLifetimeFrame(int ownerType, int childLifetime, int alrId, int columns,
        int directions, uint frameCount, out uint? frame, out string? diagnostic)
    {
        lock (_gate)
        {
            ObjectDisposedException.ThrowIf(_handle.IsClosed, this);
            _frame ??= Marshal.GetDelegateForFunctionPointer<FrameFn>(NativeLibrary.GetExport(_library, "ar_particle_definition_lifetime_frame"));
            uint result = uint.MaxValue;
            int code = _frame(_handle.DangerousGetHandle(), ownerType, childLifetime, alrId, columns, directions, frameCount, ref result);
            frame = null; diagnostic = null;
            if (code == 0) { frame = result; return true; }
            diagnostic = code switch
            {
                224 => "Particle definition chain is missing or inactive.",
                225 => "Particle grafik declares no primary object.",
                240 => "Particle asset frame layout is invalid.",
                241 => "Particle velocity frame rounding is not verified.",
                242 => "Particle direction frame rounding is not verified.",
                243 => "Particle frame divisor is zero.",
                244 => "Particle frame division overflows signed32.",
                245 => "Particle frame index is negative.",
                246 => "Particle frame index exceeds the asset.",
                247 => "Particle primary object does not use a supported ALR route.",
                248 => "Particle asset ID does not match the grafik object.",
                _ => throw new InvalidDataException($"Rust particle frame query rejected (code={code}).")
            };
            return false;
        }
    }

    internal bool TryResolve(int ownerType, out NativeParticleRenderDefinition? definition, out string? diagnostic)
    {
        lock (_gate)
        {
            ObjectDisposedException.ThrowIf(_handle.IsClosed, this);
            var output = new NativeDefinition { StructSize = 44 };
            int code = _query(_handle.DangerousGetHandle(), ownerType, ref output);
            definition = null;
            diagnostic = null;
            if (code == 225) { diagnostic = "Particle grafik declares no primary object."; return false; }
            if (code == 224) { diagnostic = "Particle primary definition chain is missing or inactive."; return false; }
            if (code != 0) throw new InvalidDataException($"Rust particle definition query rejected (code={code}).");
            definition = new(output.OwnerType, output.ParameterId, output.GeometryId, output.GrafikId,
                output.GrafikObject, output.AlrId, output.Lifetime, output.TypeDivisor,
                output.VelocityBranchRaw, output.SkipFirstRaw);
            return true;
        }
    }

    public void Dispose() { lock (_gate) _handle.Dispose(); }
    private sealed class DefinitionHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        private readonly IntPtr _library;
        private readonly FreeFn _free;
        internal DefinitionHandle(IntPtr handle, IntPtr library, FreeFn free) : base(true)
        { _library = library; _free = free; SetHandle(handle); }
        protected override bool ReleaseHandle()
        { _free(handle); NativeLibrary.Free(_library); return true; }
    }
}
