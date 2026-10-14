using System.Security.Cryptography;
using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class RustParticleBaseDefinitionTests
{
    [Fact]
    public void Installed_base_definitions_use_grafik_object_and_keep_source_files_unchanged()
    {
        string? root = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (root is null) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_REQUIRE_NATIVE_DATA")); return; }
        string[] paths = ["partpar.dau", "partgeo.dau", "partgfx.dau", "objdef.dau"];
        paths = paths.Select(p => Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS", p)).ToArray();
        byte[][] sources = paths.Select(File.ReadAllBytes).ToArray();
        byte[][] hashes = sources.Select(s => SHA256.HashData(s)).ToArray();
        var modified = paths.Select(File.GetLastWriteTimeUtc).ToArray();
        using var definitions = new RustParticleBaseDefinitions(sources[0], sources[1], sources[2], sources[3]);
        Assert.Equal(0u, definitions.SourceFilesRead);
        Assert.True(definitions.TryResolve(830, out var arrow, out var diagnostic));
        Assert.Null(diagnostic); Assert.NotNull(arrow);
        Assert.Equal(new NativeParticleRenderDefinition(830, 22, 23, 24, 227, 258, 15000, 1000, 1, 0), arrow);
        Assert.False(definitions.TryResolveLifetimeFrame(830, 14001, 258, 19, 32, 608, out var unavailable, out var frameReason));
        Assert.Null(unavailable); Assert.Contains("velocity", frameReason);
        Assert.False(definitions.TryResolveLifetimeFrame(830, 14001, 777, 19, 32, 608, out unavailable, out frameReason));
        Assert.Null(unavailable); Assert.Contains("match", frameReason);
        using (var effective = RustParticleBaseDefinitions.LoadGameSources(root))
        {
            Assert.True(effective.SourceFilesRead >= 5);
            Assert.True(effective.TryResolve(830, out var current, out _));
            Assert.Equal(arrow, current);
        }
        int resolved = 0, noPrimary = 0;
        for (int type = 0; type < 2500; type++)
        {
            if (definitions.TryResolve(type, out _, out var reason)) resolved++;
            else if (reason == "Particle grafik declares no primary object.") noPrimary++;
        }
        Assert.Equal(149, resolved); Assert.Equal(20, noPrimary);
        Assert.False(definitions.TryResolve(878, out var noPrimaryObject, out var noPrimaryDiagnostic));
        Assert.Null(noPrimaryObject); Assert.Equal("Particle grafik declares no primary object.", noPrimaryDiagnostic);
        foreach (byte[] source in sources) Array.Fill(source, (byte)0);
        Assert.True(definitions.TryResolve(830, out var second, out _)); Assert.Equal(arrow, second);
        Assert.False(definitions.TryResolve(-1, out var absent, out diagnostic)); Assert.Null(absent); Assert.NotNull(diagnostic);
        for (int i = 0; i < paths.Length; i++)
        {
            byte[] after = File.ReadAllBytes(paths[i]);
            Assert.Equal(hashes[i], SHA256.HashData(after));
            Assert.Equal(modified[i], File.GetLastWriteTimeUtc(paths[i]));
        }
        definitions.Dispose(); definitions.Dispose();
        Assert.Throws<ObjectDisposedException>(() => definitions.TryResolve(830, out _, out _));
    }
    [Fact]
    public void Missing_base_header_is_rejected_by_real_native_parser()
        => Assert.Throws<InvalidDataException>(() => new RustParticleBaseDefinitions([], [], [], []));

    [Fact]
    public void Effective_sources_reject_corrupt_present_layer_and_keep_existing_handle_valid()
    {
        string root = Path.Combine(Path.GetTempPath(), "ArmDefSources_" + Guid.NewGuid().ToString("N"));
        try
        {
            Directory.CreateDirectory(Path.Combine(root, "SYSTEM"));
            Directory.CreateDirectory(Path.Combine(root, "Selected"));
            File.WriteAllText(Path.Combine(root, "SYSTEM", "clengine.ini"), "[DefaultPath]\nSelected/\n");
            File.WriteAllText(Path.Combine(root, "Selected", "partpar.dau"), "[ParticleParameterDefault]\n22,1,Arrow,64,15000\n");
            using var first = RustParticleBaseDefinitions.LoadGameSources(root);
            Assert.Equal(2u, first.SourceFilesRead);
            File.WriteAllText(Path.Combine(root, "Selected", "partpar.d09"), "[ParticleParameterDefault]\n22,2,N,64,not_an_integer\n");
            Assert.Throws<InvalidDataException>(() => RustParticleBaseDefinitions.LoadGameSources(root));
            Assert.False(first.TryResolve(830, out _, out _));
        }
        finally
        {
            string temp = Path.GetFullPath(Path.GetTempPath());
            Assert.StartsWith(temp, Path.GetFullPath(root));
            if (Directory.Exists(root)) Directory.Delete(root, recursive: true);
        }
    }
}
