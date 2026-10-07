using System.Drawing;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Object_palette_draws_sprite_only_with_catalog(bool nature)
    {
        string map = CreateFixture();
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Palette", "Test"));
            const string name = "LanGerNad00_Tanne_gross";
            void SetPaletteField(string field, object? value) => typeof(MapEditorForm)
                .GetField(field, BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, value);
            if (nature)
            {
                var template = (LevelObjectTemplate)Activator.CreateInstance(typeof(LevelObjectTemplate),
                    BindingFlags.Instance | BindingFlags.NonPublic, null,
                    new object[] { 42, new byte[79], new uint[18], Array.Empty<byte[]>(), new byte[17], new byte[17] }, null)!;
                SetPaletteField("_objdefNames", new Dictionary<int, string> { [42] = name });
                SetPaletteField("_natureTemplates", new Dictionary<int, LevelObjectTemplate> { [42] = template });
                SetPaletteField("_natureCatalogTask", Task.FromResult<IReadOnlyDictionary<int, LevelObjectTemplate>>(
                    new Dictionary<int, LevelObjectTemplate> { [42] = template }));
            }
            else
                SetPaletteField("_objectCatalog", new[] { new SdlObjectType(name, 42, SdlObjectCategory.Building,
                    "Ger", 1, new Dictionary<string, string>()) });
            string refresh = nature ? "RefreshNatureTypes" : "RefreshPlacementTypes";
            Invoke(form, refresh);
            var list = GetField<ListBox>(form, nature ? "_natureTypes" : "_placeTypes");
            // 放到獨立容器，讓隱藏分頁的 ListBox 也能真正收到 WM_DRAWITEM。
            using var host = new Form { ClientSize = new Size(320, 120), StartPosition = FormStartPosition.Manual,
                Location = new Point(-30000, -30000) };
            Control originalParent = list.Parent!;
            host.Controls.Add(list);
            host.Show(); Application.DoEvents();
            int RedPixels()
            {
                using var frame = new Bitmap(list.Width, list.Height);
                list.DrawToBitmap(frame, new Rectangle(Point.Empty, frame.Size));
                return Red(frame).Count;
            }
            Assert.Equal(DrawMode.OwnerDrawFixed, list.DrawMode);
            Assert.Single(list.Items.Cast<object>());
            Assert.Equal(0, RedPixels());
            using var catalog = NativeSpriteCatalog.FromText(
                string.Join(",", Enumerable.Range(0, 60).Select(i => i switch
                    { 0 => "42", 5 => "0", 8 => "-1", 14 => "-1", 17 => "0", 52 => name, _ => "   0" })),
                "0000,palette.alr", "", asset => asset == "palette.alr" ? SolidAlr(80, 120) : null, _ => null);
            Assert.NotNull(catalog.GetSprite(name));
            SetPaletteField("_spriteCatalog", catalog);
            Invoke(form, refresh); list.Invalidate(); Application.DoEvents();
            Assert.True(RedPixels() > 100, "素材庫存在時，清單應畫出原生 sprite 的紅色像素。");
            list.SelectedIndex = -1;
            Assert.True(RedPixels() > 100, "未選取項目仍應顯示縮圖。");
            using (var missing = NativeSpriteCatalog.FromText("", "", "", _ => null, _ => null))
            {
                SetPaletteField("_spriteCatalog", missing);
                Invoke(form, refresh); list.Invalidate(); Application.DoEvents();
                Assert.Equal(0, RedPixels());
            }
            SetPaletteField("_spriteCatalog", null);
            Invoke(form, refresh); list.Invalidate(); Application.DoEvents();
            Assert.Equal(0, RedPixels());
            originalParent.Controls.Add(list);
        });
    }

    [Fact]
    public void Sprite_thumbnail_cache_preserves_alpha_aspect_and_reference_identity()
    {
        RunInSta(() =>
        {
            var sprite = new NativeSprite(80, 40, Enumerable.Repeat(0x80FF0000u, 80 * 40).ToArray(), 0, 0, "test");
            using var cache = new SpriteThumbnailCache();
            Bitmap image = cache.Get(sprite, 40);
            Assert.Same(image, cache.Get(sprite, 40));
            Assert.Equal(0, image.GetPixel(20, 0).A);
            Assert.InRange(image.GetPixel(20, 20).A, 126, 130);
            Assert.InRange(image.GetPixel(20, 20).R, 250, 255);
            var copy = sprite with { AssetName = "test" };
            Assert.NotSame(image, cache.Get(copy, 40));
            Assert.Equal(new Size(80, 80), cache.Get(sprite, 80).Size);
        });
    }
}
