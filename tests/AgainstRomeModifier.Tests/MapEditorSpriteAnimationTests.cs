using System.Reflection;
using System.Drawing;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Shared_animation_selection_for_7000_objects_allocates_nothing_per_tick()
    {
        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            byte[] asset = SolidIdleAlr(8);
            string row = string.Join(",", Enumerable.Range(0, 60).Select(i => i switch
            {
                0 => "0", 2 => "1000", 5 => "0", 6 => "0", 8 => "-1", 10 => "-1", 14 => "-1",
                17 => "1", 45 => "00008008", 52 => "Unit", _ => "   0"
            }));
            using var catalog = NativeSpriteCatalog.FromText(row, "0,idle.alr", "", _ => asset, _ => null);
            using var view = new Map3DViewControl { SpriteCatalog = catalog, AnimationTimeMs = 0 };
            view.UpdateSceneObjects(Enumerable.Range(0, 7000).Select(i => new MapSceneObject("Unit", i, 0, i, 0, "fixture.sdl")).ToArray());
            Assert.Equal(7000, view.AnimatedObjectCount);
            Assert.False(view.AnimationTimerRunning); // 沒有 native handle，不啟動 Timer
            var select = typeof(Map3DViewControl).GetMethod("SelectAnimationFrames", BindingFlags.Instance | BindingFlags.NonPublic)!
                .CreateDelegate<Action<double>>(view);
            for (int i = 0; i < 30; i++) select(i * 33d); // 暖身排除首次 JIT
            long before = GC.GetAllocatedBytesForCurrentThread();
            for (int i = 0; i < 300; i++) select(i * 33d);
            long allocated = GC.GetAllocatedBytesForCurrentThread() - before;
            Assert.Equal(0, allocated);
            select(500);
            var sprites = (NativeSprite?[])typeof(Map3DViewControl).GetField("_objectSprites", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!;
            NativeSprite selected = catalog.GetAnimation("Unit")!.Frames[1];
            Assert.All(sprites, sprite => Assert.Same(selected, sprite));
        });
    }

    [Theory]
    [InlineData(true, 200, true)]
    [InlineData(false, 200, false)]
    [InlineData(true, 1023, false)] // 1025px including gutters: ten frames exceed the atlas's 3x3 shelves
    public void Real_opengl_animation_clock_changes_only_animated_objects_that_fit(bool unit, int size, bool plays)
    {
        string map = CreateFixture();
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Idle", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            const string name = "IdleFixture";
            byte[] asset = SolidIdleAlr(size);
            using var catalog = NativeSpriteCatalog.FromText(
                string.Join(",", Enumerable.Range(0, 60).Select(i => i switch
                {
                    0 => "42", 2 => "1000", 5 => "0", 6 => unit ? "0" : "-1", 8 => "-1", 10 => "-1",
                    14 => "-1", 17 => unit ? "1" : "0", 45 => unit ? "00008008" : "00000000", 52 => name, _ => "   0"
                })), "0,idle.alr", "", _ => asset, _ => null);
            view.AnimationTimeMs = 0;
            view.SpriteCatalog = catalog;
            // 同一序列可由大量物件共享；tick 的索引清單不重新配置。
            view.UpdateSceneObjects([new MapSceneObject(name, 10000, 0, 6000, 0, "fixture.sdl")]);
            Assert.Equal(1, view.SpriteObjectCount);
            Assert.Equal(plays ? 1 : 0, view.AnimatedObjectCount);
            Assert.False(view.AnimationTimerRunning); // 固定時鐘不依賴 WinForms 訊息時序
            view.ShowGrid = false;
            view.FocusTile(10000 / 256f, 6000 / 256f);
            ((EditorCamera)typeof(Map3DViewControl).GetField("_camera", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!).Zoom(.05f);
            using Bitmap first = view.CaptureFrame(640, 480)!;
            Assert.True(Red(first).Count > 500, "即使動畫容量不足，也必須保留可見首格。");
            view.AnimationTimeMs = 500;
            using Bitmap second = view.CaptureFrame(640, 480)!;
            int[] firstPixels = BitmapPixels.Read(first), secondPixels = BitmapPixels.Read(second);
            if (plays) Assert.False(firstPixels.SequenceEqual(secondPixels));
            else Assert.Equal(firstPixels, secondPixels);
            view.AnimationTimeMs = 1000;
            using Bitmap looped = view.CaptureFrame(640, 480)!;
            Assert.Equal(firstPixels, BitmapPixels.Read(looped));
            view.AnimationTimeMs = 500;
            view.AnimationsEnabled = false;
            using Bitmap disabled = view.CaptureFrame(640, 480)!;
            Assert.Equal(firstPixels, BitmapPixels.Read(disabled));
            view.AnimationsEnabled = true;
            view.AnimationTimeMs = null;
            Assert.Equal(plays, view.AnimationTimerRunning);
            view.Visible = false;
            Assert.False(view.AnimationTimerRunning);
            view.Visible = true;
            Assert.Equal(plays, view.AnimationTimerRunning);
            view.UpdateSceneObjects([]);
            Assert.False(view.AnimationTimerRunning);
            view.SpriteCatalog = null;
        }, TimeSpan.FromMinutes(2));
    }

    private static byte[] SolidIdleAlr(int size)
    {
        using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream);
        int canvas = size * 2;
        int count = size > 1000 ? 10 : 2; // ALR 行 offset 僅 20 bits，單格 raw payload 必須小於 1MiB。
        foreach (uint word in new uint[] { 0x41524C41, 6, 0, (uint)count, 8, 0, (uint)count, 1, 1, 0, 0, 0, 0, 0, 0,
            (uint)canvas, (uint)canvas, 0, 0, 0, 0, 0, 0 }) writer.Write(word);
        int pixels = size * size, padded = (pixels + 3) & ~3;
        for (int frame = 0; frame < count; frame++)
        {
            writer.Write(-1); writer.Write((uint)(12 + padded));
            writer.Write((uint)(size - size / 2)); // ground point at sprite bottom centre
            writer.Write((uint)(size | size << 11 | 3 << 22)); writer.Write((uint)pixels);
            writer.Write(0u); writer.Write(0x0000FFu); writer.Write(0x00FF00u);
            writer.Write(Enumerable.Repeat((byte)(frame % 2 + 1), pixels).ToArray()); writer.Write(new byte[padded - pixels]);
            for (int y = 0; y <= size; y++) writer.Write((uint)(y * size));
        }
        return stream.ToArray();
    }
}
