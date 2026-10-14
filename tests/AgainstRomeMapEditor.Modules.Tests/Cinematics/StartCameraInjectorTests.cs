using System.Buffers.Binary;
using AgainstRomeMapEditor.Modules.Cinematics;
using AgainstRomeModifier;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Cinematics;

public sealed class StartCameraInjectorTests
{
    private static int[] Words(byte[] bytes)
    {
        int[] words = new int[bytes.Length / 4];
        for (int i = 0; i < words.Length; i++)
            words[i] = BinaryPrimitives.ReadInt32LittleEndian(bytes.AsSpan(i * 4, 4));
        return words;
    }

    [Fact]
    public void MapStartCamera_Defaults_AreValidAndEvidenced()
    {
        var camera = new MapStartCamera();
        Assert.True(camera.Enabled);
        Assert.Equal(8192.0, camera.X);
        Assert.Equal(8192.0, camera.Z);
        Assert.Equal(0.0, camera.Y);
        Assert.Equal(5.0, camera.Zoom);
        Assert.Equal(StartCameraPrimitive.CenterToPos, camera.Primitive);
        camera.Validate();
    }

    [Theory]
    [InlineData(-1, 8192, 0, 5)]
    [InlineData(16384, 8192, 0, 5)]
    [InlineData(8192, -1, 0, 5)]
    [InlineData(8192, 16384, 0, 5)]
    [InlineData(8192, 8192, double.NaN, 5)]
    [InlineData(8192, 8192, 0, -0.1)]
    [InlineData(8192, 8192, 0, 9.1)]
    [InlineData(8192, 8192, 0, double.NaN)]
    [InlineData(8192, 8192, 0, double.PositiveInfinity)]
    public void MapStartCamera_Validation_RejectsInvalidOrOutOfRangeValues(double x, double z, double y, double zoom)
    {
        var camera = new MapStartCamera { X = x, Z = z, Y = y, Zoom = zoom };
        Assert.Throws<ArgumentOutOfRangeException>(() => camera.Validate());
    }

    [Fact]
    public void MapStartCamera_Serialization_RoundTripsAccurately()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ArmStartCameraTest_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(tempDir);
        try
        {
            var camera = new MapStartCamera
            {
                Enabled = true,
                X = 12512,
                Z = 14816,
                Y = 32.5,
                Zoom = 7.5,
                Primitive = StartCameraPrimitive.DirectEnginePos
            };
            camera.Save(tempDir);

            var loaded = MapStartCamera.Load(tempDir);
            Assert.True(loaded.Enabled);
            Assert.Equal(12512, loaded.X);
            Assert.Equal(14816, loaded.Z);
            Assert.Equal(32.5, loaded.Y);
            Assert.Equal(7.5, loaded.Zoom);
            Assert.Equal(StartCameraPrimitive.DirectEnginePos, loaded.Primitive);
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, recursive: true);
        }
    }

    [Fact]
    public void CompileCenterToPosCall_EmitsCorrectOpcodesAndStackOrder()
    {
        var image = BciImage.CreateIdleLevel();
        byte[] code = CinematicBciCompiler.CompileCenterToPosCall(image, 12512, 14816);
        int[] words = Words(code);

        // Expected: pushlit 14816 (Z), pushlit 12512 (X), pushsym s_centerToPos, argc -2
        int[] expected = [
            66, 14816,
            66, 12512,
            128, 0,
            73, -2
        ];
        Assert.Equal(expected, words);
        Assert.Equal("s_centerToPos", image.Constant(0));
    }

    [Fact]
    public void CompileStartCameraCalls_CenterToPos_EmitsBothPositionAndZoom()
    {
        var image = BciImage.CreateIdleLevel();
        var camera = new MapStartCamera
        {
            X = 8000,
            Z = 9000,
            Zoom = 4.0,
            Primitive = StartCameraPrimitive.CenterToPos
        };

        byte[] code = CinematicBciCompiler.CompileStartCameraCalls(image, camera);
        int[] words = Words(code);

        // Expected:
        // 66, 9000 (Z), 66, 8000 (X), 128, <s_centerToPos>, 73, -2
        // 67, zoomLow, zoomHigh, 128, <s_lgcSetEngineZoom>, 73, -2
        Assert.Equal(66, words[0]);
        Assert.Equal(9000, words[1]);
        Assert.Equal(66, words[2]);
        Assert.Equal(8000, words[3]);
        Assert.Equal(128, words[4]);
        Assert.Equal(73, words[6]);
        Assert.Equal(-2, words[7]);

        Assert.Equal(67, words[8]); // Double immediate
        Assert.Equal(128, words[11]); // s_lgcSetEngineZoom call
        Assert.Equal(73, words[13]);
        Assert.Equal(-2, words[14]);

        Assert.Equal("s_centerToPos", image.Constant(words[5]));
        Assert.Equal("s_lgcSetEngineZoom", image.Constant(words[12]));
    }

    [Fact]
    public void CompileStartCameraCalls_DirectEnginePos_EmitsDirectPositionAndZoom()
    {
        var image = BciImage.CreateIdleLevel();
        var camera = new MapStartCamera
        {
            X = 1000,
            Y = 50,
            Z = 2000,
            Zoom = 3.0,
            Primitive = StartCameraPrimitive.DirectEnginePos
        };

        byte[] code = CinematicBciCompiler.CompileStartCameraCalls(image, camera);
        int[] words = Words(code);

        // s_lgcSetEnginePos takes 3 doubles, cleanup -6; then s_lgcSetEngineZoom takes 1 double, cleanup -2
        Assert.Equal(128, words[9]);
        Assert.Equal(73, words[11]);
        Assert.Equal(-6, words[12]);
        Assert.Equal("s_lgcSetEnginePos", image.Constant(words[10]));

        Assert.Equal(128, words[16]);
        Assert.Equal(73, words[18]);
        Assert.Equal(-2, words[19]);
        Assert.Equal("s_lgcSetEngineZoom", image.Constant(words[17]));
    }

    [Fact]
    public void Inject_PrependsShimAndJumpsToOriginalMain()
    {
        var image = BciImage.CreateIdleLevel();
        int originalMain = image.MainAddress;
        int originalCodeLength = image.Code.Length;

        var camera = new MapStartCamera
        {
            X = 8192,
            Z = 8192,
            Zoom = 5.0,
            Primitive = StartCameraPrimitive.CenterToPos
        };

        MapStartCameraInjector.Inject(image, camera);

        // MainAddress should now point to start of injected code
        Assert.Equal(originalCodeLength, image.MainAddress);
        Assert.True(image.Code.Length > originalCodeLength);

        // The end of the injected code must be OpJump (112) jumping back to originalMain
        int[] allWords = Words(image.Code);
        int jumpOpcodeIndex = allWords.Length - 2;
        int jumpOperandIndex = allWords.Length - 1;

        Assert.Equal(112, allWords[jumpOpcodeIndex]);
        int jumpOffset = jumpOpcodeIndex * 4;
        int operand = allWords[jumpOperandIndex];
        // OpJump formula: target = jumpOffset + 8 + operand
        Assert.Equal(originalMain, jumpOffset + 8 + operand);

        // BCI roundtrip serialization must be valid
        byte[] serialized = image.Serialize();
        var reparsed = BciImage.Parse(serialized);
        Assert.Equal(image.MainAddress, reparsed.MainAddress);
        Assert.Equal(image.Code.Length, reparsed.Code.Length);
    }

    [Fact]
    public void Inject_WhenDisabled_DoesNotModifyImage()
    {
        var image = BciImage.CreateIdleLevel();
        byte[] originalCode = image.Code.ToArray();
        int originalMain = image.MainAddress;

        var camera = new MapStartCamera { Enabled = false };
        MapStartCameraInjector.Inject(image, camera);

        Assert.Equal(originalMain, image.MainAddress);
        Assert.Equal(originalCode, image.Code);
    }

    [Fact]
    public void Apply_FullFileWorkflow_CreatesBackupAndInjectsScript()
    {
        string tempDir = Path.Combine(Path.GetTempPath(), "ArmStartCameraApplyTest_" + Guid.NewGuid().ToString("N"));
        string scriptDir = Path.Combine(tempDir, "SCRIPT");
        Directory.CreateDirectory(scriptDir);

        try
        {
            var idleImage = BciImage.CreateIdleLevel();
            string scriptPath = Path.Combine(scriptDir, LevelScriptInjector.ScriptFile);
            File.WriteAllBytes(scriptPath, idleImage.Serialize());

            var camera = new MapStartCamera
            {
                Enabled = true,
                X = 4000,
                Z = 6000,
                Zoom = 2.5,
                Primitive = StartCameraPrimitive.CenterToPos
            };

            using (var rollback = new FileRollbackScope())
            {
                MapStartCameraInjector.Apply(tempDir, camera, rollback);
                rollback.Commit();
            }

            // Backup file ak_level.arm_original must exist
            string backupPath = Path.Combine(scriptDir, LevelScriptInjector.OriginalBackupFile);
            Assert.True(File.Exists(backupPath));

            // Injected ak_level.bci must contain s_centerToPos and s_lgcSetEngineZoom
            byte[] injectedBytes = File.ReadAllBytes(scriptPath);
            var injectedImage = BciImage.Parse(injectedBytes);
            Assert.Contains(injectedImage.ConstOffsets, off =>
            {
                int end = Array.IndexOf(injectedImage.ConstBlob, (byte)0, off);
                string name = System.Text.Encoding.Latin1.GetString(injectedImage.ConstBlob, off, (end < 0 ? injectedImage.ConstBlob.Length : end) - off);
                return name == "s_centerToPos";
            });

            // Metadata file arm_start_camera.json must be written
            string metaPath = Path.Combine(tempDir, MapStartCamera.FileName);
            Assert.True(File.Exists(metaPath));
            var loaded = MapStartCamera.Load(tempDir);
            Assert.True(loaded.Enabled);
            Assert.Equal(4000, loaded.X);
            Assert.Equal(6000, loaded.Z);
            Assert.Equal(2.5, loaded.Zoom);
        }
        finally
        {
            if (Directory.Exists(tempDir)) Directory.Delete(tempDir, recursive: true);
        }
    }
}
