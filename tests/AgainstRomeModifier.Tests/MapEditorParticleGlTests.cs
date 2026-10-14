using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using Xunit;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void ParticleGl_mode0_executes_and_mode3_is_reported_without_draw()
    {
        string map = CreateFixture("ENDL_012"); SetupFamilyParticleFrameFixture(map);
        string data = Path.Combine(map, "DATA");
        byte[] objects = File.ReadAllBytes(Path.Combine(data, "objects.dat"));
        int link = System.Buffers.Binary.BinaryPrimitives.ReadUInt16LittleEndian(objects.AsSpan(16 + 73));
        byte[] gfx = File.ReadAllBytes(Path.Combine(data, "gfxtype.dat"));
        int count = System.Buffers.Binary.BinaryPrimitives.ReadInt32LittleEndian(gfx.AsSpan(4));
        System.Buffers.Binary.BinaryPrimitives.WriteInt16LittleEndian(gfx.AsSpan(8 + count * 15 + link * 2), 99);
        File.WriteAllBytes(Path.Combine(data, "gfxtype.dat"), gfx);
        File.WriteAllText(Path.Combine(_root, "SelectedDefs", "objdef.d09"), "[ObjectDefaults]\n227,1,1000,0,0,258,0,0\n");
        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = FamilyPreviewForm(map);
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new System.Drawing.Point(-30000, -30000);
            form.Show(); Application.DoEvents(); Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            AdoptFamilyParticleTestAsset(form);
            var images = form.DecodeSavedParticlePreviewImages(0);
            var heights = ElevationViewField<TerrainHeightField>(view, "_heights");
            var camera = ElevationViewField<EditorCamera>(view, "_camera");
            var draw = Assert.Single(SceneParticleRenderer.Build(images, heights, camera.GetViewMatrix()).Draws);
            camera.Target = new System.Numerics.Vector3(draw.Vertices[0], draw.Vertices[1], draw.Vertices[2]);
            camera.Zoom(.001f);
            view.UpdateSceneObjects(Array.Empty<AgainstRomeModifier.Maps.MapSceneObject>());
            view.ShowGrid = false;
            view.UpdateSavedParticleImages(Array.Empty<SavedParticlePreviewImage>());
            using var baseline = view.CaptureFrame(1024, 768);
            Assert.NotNull(baseline);
            view.UpdateSavedParticleImages(images);
            using var frame = view.CaptureFrame(1024, 768);
            Assert.NotNull(frame); Assert.Equal(1, view.LastParticleDrawCount);
            Assert.Empty(view.ParticleDrawDiagnostics);
            int PalettePixels(System.Drawing.Bitmap bitmap)
            {
                int count = 0;
                for (int y = 0; y < bitmap.Height; y++)
                    for (int x = 0; x < bitmap.Width; x++)
                        if (bitmap.GetPixel(x, y).ToArgb() == unchecked((int)0xff605040)) count++;
                return count;
            }
            Assert.True(PalettePixels(frame!) > PalettePixels(baseline!), "Mode0 must write exact decoded palette RGB into the framebuffer.");
            int builds = view.ParticleBatchBuildCount, uploads = view.ParticleTextureUploadCount;
            using var repeated = view.CaptureFrame(1024, 768);
            Assert.NotNull(repeated);
            Assert.Equal(builds, view.ParticleBatchBuildCount);
            Assert.Equal(uploads, view.ParticleTextureUploadCount);
            Assert.Equal(PalettePixels(frame!), PalettePixels(repeated!));
            view.UpdateSavedParticleImages(images);
            using var refreshed = view.CaptureFrame(1024, 768);
            Assert.NotNull(refreshed);
            Assert.True(view.ParticleBatchBuildCount > builds);
            Assert.True(view.ParticleTextureUploadCount > uploads);
            Assert.Equal(PalettePixels(frame!), PalettePixels(refreshed!));
            view.UpdateSavedParticleImages(images.Concat(images).ToArray());
            using var duplicated = view.CaptureFrame(1024, 768);
            Assert.NotNull(duplicated); Assert.Equal(2, view.LastParticleDrawCount);
            Assert.Equal(1, view.LastParticleSubmissionCount);
            Assert.Equal(PalettePixels(frame!), PalettePixels(duplicated!));
            Assert.Equal(OpenTK.Graphics.OpenGL4.ErrorCode.NoError, OpenTK.Graphics.OpenGL4.GL.GetError());
            Assert.True(OpenTK.Graphics.OpenGL4.GL.IsEnabled(OpenTK.Graphics.OpenGL4.EnableCap.DepthTest));
            Assert.True(OpenTK.Graphics.OpenGL4.GL.IsEnabled(OpenTK.Graphics.OpenGL4.EnableCap.CullFace));
            Assert.True(OpenTK.Graphics.OpenGL4.GL.IsEnabled(OpenTK.Graphics.OpenGL4.EnableCap.Blend));
            view.UpdateSavedParticleImages(Array.Empty<SavedParticlePreviewImage>());
            using var cleared = view.CaptureFrame(1024, 768);
            Assert.NotNull(cleared); Assert.Equal(0, view.LastParticleDrawCount);
            Assert.Equal(0, view.LastParticleSubmissionCount);
            Assert.Equal(PalettePixels(baseline!), PalettePixels(cleared!));
            File.WriteAllText(Path.Combine(_root, "SelectedDefs", "objdef.d09"), "[ObjectDefaults]\n227,1,1000,0,0,258,0,3\n");
            Invoke(form, "LoadSelectedMap"); AdoptFamilyParticleTestAsset(form);
            view.UpdateSavedParticleImages(form.DecodeSavedParticlePreviewImages(0));
            using var unsupported = view.CaptureFrame(320, 240);
            Assert.NotNull(unsupported); Assert.Equal(0, view.LastParticleDrawCount);
            Assert.Contains(view.ParticleDrawDiagnostics, reason => reason.Contains("mode 3"));
            view.UpdateSavedParticleImages(Array.Empty<SavedParticlePreviewImage>());
            Assert.Empty(view.ParticleDrawDiagnostics); Assert.Equal(0, view.LastParticleDrawCount);
        }, TimeSpan.FromMinutes(2));
    }
}
