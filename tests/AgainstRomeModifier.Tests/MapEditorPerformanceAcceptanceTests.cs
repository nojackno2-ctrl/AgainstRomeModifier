using System.Diagnostics;
using System.Drawing;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;
using OpenTK.Graphics.OpenGL4;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Real_large_map_performance_records_load_decode_memory_animation_lighting_and_pan()
    {
        if (Environment.GetEnvironmentVariable("ARM_MAP_PERFORMANCE") != "1") return;
        string source = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_COMPARE_GAME"));
        string output = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_PERFORMANCE_OUTPUT"));
        Assert.False(Directory.Exists(output), "Use a new evidence directory.");
        Assert.False(output.StartsWith(source + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase));
        var hashes = PerformanceHashes(source);
        Directory.CreateDirectory(output);
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            GC.Collect(); GC.WaitForPendingFinalizers(); GC.Collect();
            var stages = new List<PerformanceStage>();
            var memory = new List<PerformanceMemory> { PerformanceMemory.Read("before") };
            void Stage(string name, Action action)
            {
                long allocation = GC.GetTotalAllocatedBytes(true);
                var clock = Stopwatch.StartNew(); action(); clock.Stop();
                stages.Add(new(name, clock.Elapsed.TotalMilliseconds, GC.GetTotalAllocatedBytes(true) - allocation));
                memory.Add(PerformanceMemory.Read(name));
                File.WriteAllText(Path.Combine(output, "stages.json"), JsonSerializer.Serialize(stages, PerformanceJson));
            }
            MapEditorForm? loaded = null;
            Stage("construct_catalogs_and_ui", () => loaded = new MapEditorForm(source,
                new GameMapInfo("ENDL_000", Path.Combine(source, "ENDL_000"), false, "Performance", "Read-only")));
            using var form = loaded!;
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.Size = new Size(1280, 900); form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            Stage("show_load_first_decode_atlas_and_first_paint", () => { form.Show(); Application.DoEvents(); Invoke(form, "SetActiveView", true); Application.DoEvents(); });
            var view = GetField<Map3DViewControl>(form, "_view3d");
            Assert.True(view.IsReady, view.LastFailureReason);
            var objects = (IReadOnlyList<MapSceneObject>)typeof(Map3DViewControl).GetField("_objects", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!;
            var level = GetField<IReadOnlyList<LevelWorldObject>>(form, "_levelObjects");
            Assert.True(level.Count >= 1000, $"Large-map acceptance requires actual source objects, got {level.Count}.");
            Assert.True(view.SpriteObjectCount > level.Count / 2);
            Assert.True(view.ShadowObjectCount > 0); Assert.True(view.AnimatedObjectCount > 0); Assert.True(view.SceneLightCount > 0);
            MapSceneObject anchor = ((System.Collections.IEnumerable)typeof(Map3DViewControl).GetField("_animationGroups", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!).Cast<object>()
                .SelectMany(group => (int[])group.GetType().GetProperty("ObjectIndices")!.GetValue(group)!).Select(index => objects[index])
                .First(item => item.WorldX is >= 0 and < 16384 && item.WorldZ is >= 0 and < 16384);
            view.ShowGrid = false; view.AnimationTimeMs = 0;
            view.MakeCurrent();
            using var renderTarget = new PerformanceRenderTarget(view.ClientSize.Width, view.ClientSize.Height);
            var down = typeof(Map3DViewControl).GetMethod("OnMouseDown", BindingFlags.Instance | BindingFlags.NonPublic)!.CreateDelegate<Action<MouseEventArgs>>(view);
            var move = typeof(Map3DViewControl).GetMethod("OnMouseMove", BindingFlags.Instance | BindingFlags.NonPublic)!.CreateDelegate<Action<MouseEventArgs>>(view);
            var up = typeof(Map3DViewControl).GetMethod("OnMouseUp", BindingFlags.Instance | BindingFlags.NonPublic)!.CreateDelegate<Action<MouseEventArgs>>(view);
            var cases = new List<PerformanceCase>();
            foreach (var settings in new[] { ("terrain", false, false, false), ("static", true, false, false),
                ("animation", true, true, false), ("lighting", true, false, true), ("animation_lighting", true, true, true) })
            {
                foreach (bool pan in new[] { false, true })
                {
                    view.ShowObjects = settings.Item2; view.AnimationsEnabled = settings.Item3; view.GameLightingEnabled = settings.Item4; view.GameHour = 12;
                    view.FocusTile(anchor.WorldX / 256f, anchor.WorldZ / 256f);
                    var camera = (EditorCamera)typeof(Map3DViewControl).GetField("_camera", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!; camera.ZoomToGameScale(view.ClientSize.Height);
                    Point center = new(view.ClientSize.Width / 2, view.ClientSize.Height / 2);
                    if (pan) down(new MouseEventArgs(MouseButtons.Middle, 1, center.X, center.Y, 0));
                    void Frame(int frame)
                    {
                        if (settings.Item3) view.AnimationTimeMs = frame * (1000d / 30);
                        if (pan)
                        {
                            double angle = frame * Math.PI / 60;
                            move(new MouseEventArgs(MouseButtons.Middle, 0, center.X + (int)(40 * Math.Sin(angle)), center.Y + (int)(20 * Math.Cos(angle)), 0));
                        }
                        renderTarget.Bind(); view.Refresh(); GL.Finish();
                    }
                    for (int i = 0; i < 12; i++) Frame(i);
                    long painted = view.PaintFrameCount, allocated = GC.GetAllocatedBytesForCurrentThread();
                    int gen0 = GC.CollectionCount(0), gen1 = GC.CollectionCount(1), gen2 = GC.CollectionCount(2);
                    var samples = new double[120]; var total = Stopwatch.StartNew();
                    for (int i = 0; i < samples.Length; i++)
                    {
                        long start = Stopwatch.GetTimestamp(); Frame(i + 12); samples[i] = Stopwatch.GetElapsedTime(start).TotalMilliseconds;
                    }
                    total.Stop(); long bytes = GC.GetAllocatedBytesForCurrentThread() - allocated;
                    if (pan) up(new MouseEventArgs(MouseButtons.Middle, 1, center.X, center.Y, 0));
                    Assert.Equal(samples.Length, view.PaintFrameCount - painted);
                    Array.Sort(samples);
                    cases.Add(new(settings.Item1, pan, samples.Length, total.Elapsed.TotalMilliseconds / samples.Length, samples[samples.Length / 2], samples[(int)(samples.Length * .95)],
                        samples[^1], samples.Length / total.Elapsed.TotalSeconds, bytes / samples.Length, GC.CollectionCount(0) - gen0, GC.CollectionCount(1) - gen1, GC.CollectionCount(2) - gen2));
                    memory.Add(PerformanceMemory.Read(settings.Item1 + (pan ? "_pan" : "_still")));
                    File.WriteAllText(Path.Combine(output, "frames.json"), JsonSerializer.Serialize(cases, PerformanceJson));
                    renderTarget.Save(Path.Combine(output, settings.Item1 + (pan ? "_pan.png" : "_still.png")));
                }
            }
            view.ShowObjects = true; view.AnimationsEnabled = true; view.GameLightingEnabled = true; view.FocusTile(32, 32);
            using (var image = view.CaptureFrame(1280, 800)) { Assert.NotNull(image); image.Save(Path.Combine(output, "overview.png")); }
            Stage("warm_reload_cached_assets", () => { Invoke(form, "LoadSelectedMap"); view.Refresh(); GL.Finish(); });
            int decoded = 0, animations = 0;
            var unique = objects.DistinctBy(item => (item.Name, item.Team, item.Angle)).ToArray();
            NativeSpriteCatalog? opened = null;
            Stage("independent_catalog_open", () => opened = NativeSpriteCatalog.Open(source));
            using (var catalog = opened)
            {
                Assert.NotNull(catalog);
                Stage("first_decode_unique_sprites_and_idle_animations_new_catalog_os_cache_warm", () =>
                {
                    foreach (var item in unique)
                    {
                        if (catalog.GetSprite(item.Name, item.Team, angleDegrees: item.Angle) is not null) decoded++;
                        if (catalog.GetAnimation(item.Name, item.Team, item.Angle) is not null) animations++;
                    }
                });
            }
            Assert.True(decoded > 0); Assert.True(animations > 0);
            File.WriteAllText(Path.Combine(output, "report.json"), JsonSerializer.Serialize(new
            {
                Revision = Environment.GetEnvironmentVariable("ARM_PERFORMANCE_REVISION") ?? "unspecified",
                Map = "ENDL_000", ActualDataObjects = level.Count, SceneObjects = objects.Count, view.SpriteObjectCount, view.ShadowObjectCount, view.AnimatedObjectCount, view.SceneLightCount,
                UniqueVariants = unique.Length, DecodedVariants = decoded, AnimatedVariants = animations, view.ContextDescription,
                BenchmarkAnchor = new { anchor.Name, anchor.WorldX, anchor.WorldZ },
                Viewport = new { view.ClientSize.Width, view.ClientSize.Height }, ProcessorCount = Environment.ProcessorCount,
                Runtime = System.Runtime.InteropServices.RuntimeInformation.FrameworkDescription, OS = System.Runtime.InteropServices.RuntimeInformation.OSDescription,
                Stages = stages, Memory = memory, Frames = cases,
                Method = "Existing TEMP native assets; new catalogs, OS file cache not flushed. 12 warmup + 120 synchronous Refresh/SwapBuffers + GL.Finish frames per case; persistent color/depth FBO at actual viewport size ensures GPU rasterization even with offscreen window; saves measured final frame. Pan uses actual OnMouseDown/Move/Up; includes hover/picking/navigation callbacks. Animation fixed clock advances 30Hz. FPS is completed render throughput, not monitor presentation or sustained UI timer FPS. Memory is process/managed, excludes GPU VRAM. One process, cases share caches."
            }, PerformanceJson));
            Assert.False(GetProperty<bool>(form, "IsDirty"));
        }, TimeSpan.FromMinutes(8));
        Assert.Equal(hashes, PerformanceHashes(source));
        File.WriteAllText(Path.Combine(output, "source-hashes.json"), JsonSerializer.Serialize(hashes, PerformanceJson));
    }

    private static readonly JsonSerializerOptions PerformanceJson = new() { WriteIndented = true };
    private sealed record PerformanceStage(string Name, double Milliseconds, long AllocatedBytes);
    private sealed record PerformanceCase(string Mode, bool Pan, int Frames, double MeanMs, double MedianMs, double P95Ms, double MaxMs, double RenderThroughputFps, long ThreadAllocatedBytesPerFrame, int Gen0, int Gen1, int Gen2);
    private sealed record PerformanceMemory(string Stage, long ManagedBytes, long WorkingSetBytes, long PrivateBytes, long PeakWorkingSetBytes)
    {
        internal static PerformanceMemory Read(string stage)
        {
            using var process = Process.GetCurrentProcess(); process.Refresh();
            return new(stage, GC.GetTotalMemory(false), process.WorkingSet64, process.PrivateMemorySize64, process.PeakWorkingSet64);
        }
    }
    private sealed class PerformanceRenderTarget : IDisposable
    {
        private readonly int _framebuffer = GL.GenFramebuffer(), _color = GL.GenRenderbuffer(), _depth = GL.GenRenderbuffer();
        private readonly int _width, _height;
        internal PerformanceRenderTarget(int width, int height)
        {
            _width = width; _height = height; Bind();
            GL.BindRenderbuffer(RenderbufferTarget.Renderbuffer, _color); GL.RenderbufferStorage(RenderbufferTarget.Renderbuffer, RenderbufferStorage.Rgba8, width, height);
            GL.FramebufferRenderbuffer(FramebufferTarget.Framebuffer, FramebufferAttachment.ColorAttachment0, RenderbufferTarget.Renderbuffer, _color);
            GL.BindRenderbuffer(RenderbufferTarget.Renderbuffer, _depth); GL.RenderbufferStorage(RenderbufferTarget.Renderbuffer, RenderbufferStorage.DepthComponent24, width, height);
            GL.FramebufferRenderbuffer(FramebufferTarget.Framebuffer, FramebufferAttachment.DepthAttachment, RenderbufferTarget.Renderbuffer, _depth);
            Assert.Equal(FramebufferErrorCode.FramebufferComplete, GL.CheckFramebufferStatus(FramebufferTarget.Framebuffer));
        }
        internal void Bind() => GL.BindFramebuffer(FramebufferTarget.Framebuffer, _framebuffer);
        internal void Save(string path)
        {
            Bind(); var pixels = new int[_width * _height]; GL.ReadPixels(0, 0, _width, _height, OpenTK.Graphics.OpenGL4.PixelFormat.Bgra, PixelType.UnsignedByte, pixels);
            Assert.Contains(pixels, pixel => pixel != pixels[0]); // a counted frame must contain rendered scene pixels
            using var bitmap = BitmapPixels.Write(_width, _height, pixels); bitmap.RotateFlip(RotateFlipType.RotateNoneFlipY); bitmap.Save(path);
            GL.BindFramebuffer(FramebufferTarget.Framebuffer, 0);
        }
        public void Dispose()
        {
            GL.BindFramebuffer(FramebufferTarget.Framebuffer, 0); GL.DeleteRenderbuffer(_color); GL.DeleteRenderbuffer(_depth); GL.DeleteFramebuffer(_framebuffer);
        }
    }
    private static Dictionary<string, string> PerformanceHashes(string source)
    {
        var result = new Dictionary<string, string>();
        void Visit(string directory)
        {
            Assert.False(File.GetAttributes(directory).HasFlag(FileAttributes.ReparsePoint));
            foreach (string file in Directory.GetFiles(directory).Order(StringComparer.Ordinal))
            {
                Assert.False(File.GetAttributes(file).HasFlag(FileAttributes.ReparsePoint));
                using var stream = File.OpenRead(file); result.Add(Path.GetRelativePath(source, file), Convert.ToHexString(SHA256.HashData(stream)));
            }
            foreach (string child in Directory.GetDirectories(directory).Order(StringComparer.Ordinal)) Visit(child);
        }
        Visit(source); return result;
    }
}
