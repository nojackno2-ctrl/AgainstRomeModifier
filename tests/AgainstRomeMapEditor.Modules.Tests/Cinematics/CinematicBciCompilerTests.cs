using System.Buffers.Binary;
using System.Numerics;
using System.Security.Cryptography;
using AgainstRomeMapEditor.Modules.Cinematics;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Cinematics;

public sealed class CinematicBciCompilerTests
{
    [Fact]
    public void CameraStatements_UseDoubleWordsReverseArgumentsAndWordCleanup()
    {
        var image = BciImage.CreateIdleLevel();
        byte[] originalCode = image.Code.ToArray();
        int originalMain = image.MainAddress;
        byte[] code = CinematicBciCompiler.CompileCameraCalls(image, 1.25, -2.5, 3000, 2.75);
        // IEEE754 double words, as read by EXE opcode 67 (0x5b5b89), not float literals.
        int[] expected = [67, 0, 0x40a77000, 67, 0, unchecked((int)0xc0040000), 67, 0, 0x3ff40000,
            128, 0, 73, -6, 67, 0, 0x40060000, 128, 1, 73, -2];
        Assert.Equal(expected, Words(code));
        Assert.Equal("s_lgcSetEnginePos", image.Constant(0));
        Assert.Equal("s_lgcSetEngineZoom", image.Constant(1));
        Assert.Equal(originalCode, image.Code);
        Assert.Equal(originalMain, image.MainAddress);
        Assert.Equal(image.Serialize(), BciImage.Parse(image.Serialize()).Serialize());
    }

    [Fact]
    public void NativeIndices_AreResolvedPerImageAndReused()
    {
        var image = BciImage.CreateIdleLevel();
        image.AddConstant("unrelated"); image.AddConstant("s_lgcSetEngineZoom"); image.AddConstant("s_lgcSetEnginePos");
        int[] code = Words(CinematicBciCompiler.CompileCameraCalls(image, 0, 0, 0, 0));
        Assert.Equal(2, code[10]); Assert.Equal(1, code[17]); Assert.Equal(3, image.ConstOffsets.Count);
    }

    [Theory]
    [InlineData(-1)] [InlineData(82)] [InlineData(double.NaN)] [InlineData(double.PositiveInfinity)]
    public void PreviewDistanceOrInvalidZoom_IsRejectedBeforeChangingImage(double zoom)
    {
        var image = BciImage.CreateIdleLevel(); byte[] before = image.Serialize();
        Assert.Throws<ArgumentOutOfRangeException>(() => CinematicBciCompiler.CompileCameraCalls(image, 0, 0, 0, zoom));
        Assert.Equal(before, image.Serialize());
    }

    [Fact]
    public void InvalidPosition_IsRejectedBeforeChangingImage()
    {
        var image = BciImage.CreateIdleLevel(); byte[] before = image.Serialize();
        Assert.Throws<ArgumentOutOfRangeException>(() => CinematicBciCompiler.CompileCameraCalls(image, double.MaxValue, 0, 0, 1));
        Assert.Equal(before, image.Serialize());
    }

    [Fact]
    public void SubtitleEvents_UseAbsoluteDeadlinesAndDoNotPretendToTriggerEvents()
    {
        var seq = new CutsceneSequence
        {
            Name = "intro", Subtitles = [new(1, 3, "speaker", "first"), new(5, 2, "speaker", "second")],
            OnCompleteTriggerEvent = "SpawnReinforcements"
        };
        var events = CinematicBciCompiler.CompileToScenarioEvents(seq);
        Assert.Equal(new[] { 1, 5 }, events.Select(e => e.DelaySeconds));
        Assert.All(events, e => Assert.All(e.Actions, a => Assert.Equal(ScenarioActionKind.Message, a.Kind)));
        Assert.Equal("[speaker] second", events[1].Actions[0].Text);
        Assert.Empty(CinematicBciCompiler.CompileToScenarioEvents(new CutsceneSequence()));
    }

    [Fact]
    public void SequencePreview_IsExplicitlyUnwiredAndContainsNoInventedCalls()
    {
        var seq = new CutsceneSequence { Id = "intro_01", CameraWaypoints = [new(new Vector3(1, 2, 3))] };
        string script = CinematicBciCompiler.GenerateBciScriptText(seq, 4);
        Assert.True(CinematicBciCompiler.IsExperimental); Assert.False(CinematicBciCompiler.IsWiredToLevelScript);
        Assert.Contains("EXPERIMENTAL / UNWIRED", script);
        Assert.All(script.Split('\n', StringSplitOptions.RemoveEmptyEntries), line => Assert.StartsWith("//", line));
        Assert.DoesNotContain("s_disableGUI", script); Assert.DoesNotContain("s_conWaitTime", script);
    }

    [CinematicEvidenceFact]
    public void TempMessageStatements_MatchRealEncodingsAfterTextConstantRelocation()
    {
        string root = Environment.GetEnvironmentVariable("ARM_CINEMATIC_SAMPLES")!;
        // The fixture must be the authorized TEMP copy, never an installed game directory.
        Assert.Equal(Path.GetFullPath(Path.Combine(Path.GetTempPath(), "ArmGameCompare_20261007")), Path.GetFullPath(root), ignoreCase: true);
        foreach (string map in new[] { "ENDL_000", "ENDL_005" })
        {
            byte[] raw = File.ReadAllBytes(Path.Combine(root, map, "SCRIPT", "ak_level.bci"));
            Assert.Equal(map == "ENDL_000" ? "336e3a5cb808e7f2ef6aaa77ac2ef54a0fb00de04cb00d1abb4fbd5b937d5f27" :
                "faaba1e6653327c04610ac0ab4a728d1faadde4cf750eb9598f33e0504fd769e", Convert.ToHexString(SHA256.HashData(raw)).ToLowerInvariant());
            var image = BciImage.Parse(GameLZSS.DecompressPfil(raw));
            int start = image.ConstOffsets[240]; int end = Array.IndexOf(image.ConstBlob, (byte)0, start);
            string text = MapTextEncoding.Game.GetString(image.ConstBlob, start, end - start);
            byte[] emitted = CinematicBciCompiler.CompileMessageCall(image, text);
            int relocated = BinaryPrimitives.ReadInt32LittleEndian(emitted.AsSpan(4));
            Assert.Equal(image.ConstBlob.AsSpan(start, end - start).ToArray(),
                image.ConstBlob.AsSpan(image.ConstOffsets[relocated], end - start).ToArray());
            BinaryPrimitives.WriteInt32LittleEndian(emitted.AsSpan(4), 240);
            Assert.Equal(image.Code.AsSpan(0x1b9f4, 32).ToArray(), emitted);
            Assert.DoesNotContain(Enumerable.Range(0, image.ConstOffsets.Count).Select(image.Constant), s => s == "s_lgcSetEnginePos");
        }
    }

    [CinematicEvidenceFact]
    public void RepoExe_ContainsVerifiedRegistrationsAndDoublePushHandler()
    {
        byte[] exe = File.ReadAllBytes(Environment.GetEnvironmentVariable("ARM_CINEMATIC_EXE")!);
        Assert.Equal("6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf", Convert.ToHexString(SHA256.HashData(exe)).ToLowerInvariant());
        Assert.True(exe.AsSpan().IndexOf(Convert.FromHexString("6a0068f32e60006800c4540068fa2e6000e8294b0600")) >= 0);
        Assert.True(exe.AsSpan().IndexOf(Convert.FromHexString("6a0068a72f60006820c6540068ac2f6000e86c4a0600")) >= 0);
        Assert.True(exe.AsSpan().IndexOf("v(ddd)\0s_lgcSetEnginePos\0"u8) >= 0);
        Assert.True(exe.AsSpan().IndexOf("v(d)\0s_lgcSetEngineZoom\0"u8) >= 0);
        // Opcode 67: read two immediate words and advance PC by 8.
        Assert.True(exe.AsSpan().IndexOf(Convert.FromHexString("8b432c8b140783c708")) >= 0);
        // Signature marshalling: double consumes two stack words.
        Assert.True(exe.AsSpan().IndexOf(Convert.FromHexString("8b52fc83c002895104")) >= 0);
    }

    private static int[] Words(byte[] code) => Enumerable.Range(0, code.Length / 4)
        .Select(i => BinaryPrimitives.ReadInt32LittleEndian(code.AsSpan(i * 4))).ToArray();
}

public sealed class CinematicEvidenceFactAttribute : FactAttribute
{
    public CinematicEvidenceFactAttribute()
    {
        if (Environment.GetEnvironmentVariable("ARM_CINEMATIC_SAMPLES") is null || Environment.GetEnvironmentVariable("ARM_CINEMATIC_EXE") is null)
            Skip = "Requires authorized TEMP cinematic samples and repository EXE copy; no game assets are bundled.";
    }
}
