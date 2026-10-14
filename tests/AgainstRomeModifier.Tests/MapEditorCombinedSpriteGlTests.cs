using System.Numerics;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Combined_gl_orders_overlapping_object_and_particle_by_view_depth()
    {
        string map = CreateFixture("ENDL_012"); SetupFamilyParticleFrameFixture(map);
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
            var particle = Assert.Single(SceneParticleRenderer.Build(images, heights, camera.GetViewMatrix()).Draws);
            var anchor = new Vector3(particle.Vertices[0], particle.Vertices[1], particle.Vertices[2]);
            camera.Target = anchor; camera.Zoom(.001f);
            var towardsEye = Vector3.Normalize(camera.Position - camera.Target);
            string definition = string.Join(',', Enumerable.Range(0, 60).Select(i => i switch
                { 0 => "555", 2 => "1000", 5 => "0", 8 => "-1", 14 => "-1", 17 => "1", 52 => "BauGerTest", _ => "0" }));
            byte[] red = SolidAlr(64, 40);
            using var catalog = NativeSpriteCatalog.FromText(definition, "0000,red.alr", "", _ => red, _ => null);
            view.SpriteCatalog = catalog; view.GameLightingEnabled = false; view.ShowGrid = false;
            MapSceneObject ObjectAt(Vector3 position) => new("BauGerTest", position.X * 256,
                position.Y * heights.NativeHeightMapStep * 255 / heights.HeightScale, position.Z * 256, 0, "")
                { HasNativeSavedPosition = true };
            int ParticlePixels(System.Drawing.Bitmap frame)
            {
                int pixels = 0;
                for (int y = 0; y < frame.Height; y++)
                    for (int x = 0; x < frame.Width; x++)
                        if (frame.GetPixel(x, y).ToArgb() == unchecked((int)0xff605040)) pixels++;
                return pixels;
            }
            view.UpdateSceneObjects(new[] { ObjectAt(anchor - towardsEye * .5f) });
            view.UpdateSavedParticleImages(images);
            using var behind = view.CaptureFrame(1024, 768);
            Assert.NotNull(behind); Assert.True(Red(behind!).Count > 100);
            Assert.True(ParticlePixels(behind!) > 0); Assert.Equal(1, view.LastParticleDrawCount);
            view.UpdateSavedParticleImages(images.Concat(images).ToArray());
            using var doubled = view.CaptureFrame(1024, 768);
            Assert.NotNull(doubled); Assert.Equal(ParticlePixels(behind!), ParticlePixels(doubled!));
            Assert.Equal(2, view.LastParticleDrawCount); Assert.Equal(1, view.LastParticleSubmissionCount);
            view.UpdateSavedParticleImages(images);
            view.UpdateSceneObjects(new[] { ObjectAt(anchor + towardsEye * .5f) });
            using var inFront = view.CaptureFrame(1024, 768);
            Assert.NotNull(inFront); Assert.True(Red(inFront!).Count > 100);
            Assert.Equal(0, ParticlePixels(inFront!)); Assert.Equal(1, view.LastParticleDrawCount);
            Assert.Equal(OpenTK.Graphics.OpenGL4.ErrorCode.NoError, OpenTK.Graphics.OpenGL4.GL.GetError());
            view.SpriteCatalog = null;
        }, TimeSpan.FromMinutes(2));
    }
}
