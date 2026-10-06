using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

/// <summary>地形高度／通行區域編輯狀態與 boden／emboss／collision.bmp 讀寫的回歸測試（純合成資料，不碰遊戲目錄）。</summary>
public sealed class MapEditorTerrainHeightTests : IDisposable
{
    private readonly string _root = Path.Combine(Path.GetTempPath(), "AgainstRomeTerrainHeightTests_" + Guid.NewGuid().ToString("N"));

    // ---------- 高度筆刷 ----------

    [Fact]
    public void Raise_peaks_at_center_falls_off_to_zero_at_radius_and_clamps_to_255()
    {
        const int size = 17;
        byte[] flat = Filled(size, 100);
        var session = new TerrainHeightEditSession(size, flat, null, 0, null);

        IReadOnlyList<TerrainSampleChange> changes = session.PaintHeight(8, 8, 4, TerrainHeightOperation.Raise, 10);

        Assert.NotEmpty(changes);
        byte[] after = session.Heights.ToArray();
        Assert.Equal(110, after[8 * size + 8]); // 中心 falloff = 1
        Assert.Equal(after.Max(), after[8 * size + 8]);
        for (int y = 0; y < size; y++)
        for (int x = 0; x < size; x++)
        {
            double distance = Math.Sqrt((x - 8) * (x - 8) + (y - 8) * (y - 8));
            int delta = after[y * size + x] - 100;
            Assert.True(delta >= 0);
            if (distance >= 4) Assert.Equal(0, delta); // 半徑上 falloff = 0，半徑外不處理
        }
        // 越靠近中心升高越多（沿 X 軸單調遞減）。
        for (int x = 8; x < 12; x++) Assert.True(after[8 * size + x] >= after[8 * size + x + 1]);
        foreach (TerrainSampleChange change in changes)
        {
            Assert.Equal(100, change.Before);
            Assert.Equal(after[change.Index], change.After);
        }

        var high = new TerrainHeightEditSession(size, Filled(size, 250), null, 0, null);
        high.PaintHeight(8, 8, 3, TerrainHeightOperation.Raise, 64);
        Assert.Equal(255, high.Heights[8 * size + 8]);
        Assert.All(high.Heights, value => Assert.True(value >= 250));

        var low = new TerrainHeightEditSession(size, Filled(size, 5), null, 0, null);
        low.PaintHeight(8, 8, 3, TerrainHeightOperation.Lower, 64);
        Assert.Equal(0, low.Heights[8 * size + 8]);
        Assert.All(low.Heights, value => Assert.True(value <= 5));
    }

    [Fact]
    public void Flatten_moves_samples_toward_center_height_and_Smooth_reduces_a_spike()
    {
        const int size = 17;
        var ramp = new byte[size * size];
        for (int y = 0; y < size; y++)
        for (int x = 0; x < size; x++) ramp[y * size + x] = (byte)(60 + 4 * x);
        byte centerHeight = ramp[8 * size + 8];
        var flatten = new TerrainHeightEditSession(size, ramp, null, 0, null);

        flatten.PaintHeight(8, 8, 4, TerrainHeightOperation.Flatten, 16);

        bool movedAny = false;
        for (int index = 0; index < ramp.Length; index++)
        {
            int before = Math.Abs(ramp[index] - centerHeight), after = Math.Abs(flatten.Heights[index] - centerHeight);
            Assert.True(after <= before, $"index {index} moved away from the flatten target");
            // 不越過目標高度。
            Assert.True(Math.Sign(flatten.Heights[index] - centerHeight) * Math.Sign(ramp[index] - centerHeight) >= 0);
            movedAny |= after < before;
        }
        Assert.True(movedAny);
        Assert.Equal(centerHeight, flatten.Heights[8 * size + 8]);

        byte[] spike = Filled(size, 100);
        spike[8 * size + 8] = 200;
        var smooth = new TerrainHeightEditSession(size, spike, null, 0, null);

        smooth.PaintHeight(8, 8, 2, TerrainHeightOperation.Smooth, 8);

        byte center = smooth.Heights[8 * size + 8];
        Assert.True(center < 200);
        Assert.True(center >= 100);
        Assert.All(smooth.Heights, value => Assert.InRange(value, (byte)100, center));
    }

    // ---------- 復原／重做 ----------

    [Fact]
    public void Multiple_paints_commit_as_one_undo_step_and_undo_redo_round_trip()
    {
        const int size = 17;
        byte[] original = Filled(size, 100);
        var session = new TerrainHeightEditSession(size, original, null, 8, Filled(8, 0));

        session.PaintHeight(5, 5, 3, TerrainHeightOperation.Raise, 10);
        session.PaintHeight(6, 5, 3, TerrainHeightOperation.Raise, 10);
        session.PaintHeight(11, 11, 2, TerrainHeightOperation.Lower, 10);
        byte[] painted = session.Heights.ToArray();
        Assert.True(session.IsDirty);
        Assert.True(session.HeightsDirty);
        Assert.True(session.CanUndo); // 未提交的筆劃也可復原

        Assert.True(session.CommitStroke());
        Assert.False(session.CommitStroke()); // 沒有待提交變更

        TerrainLayerStroke? undone = session.Undo();

        Assert.NotNull(undone);
        Assert.Equal(original, session.Heights.ToArray());
        Assert.False(session.IsDirty);
        Assert.False(session.HeightsDirty);
        Assert.False(session.CanUndo); // 三次 PaintHeight 只算一步
        Assert.True(session.CanRedo);
        Assert.Empty(undone!.Collision);
        Assert.Equal(painted.Count((value) => value != 100), undone.Heights.Count);
        foreach (TerrainSampleChange change in undone.Heights)
        {
            // 回傳的是反向筆劃：Before = 繪製後值、After = 復原後值。
            Assert.Equal(painted[change.Index], change.Before);
            Assert.Equal(original[change.Index], change.After);
        }
        Assert.Null(session.Undo());

        TerrainLayerStroke? redone = session.Redo();

        Assert.NotNull(redone);
        Assert.Equal(painted, session.Heights.ToArray());
        Assert.True(session.IsDirty);
        Assert.False(session.CanRedo);
        foreach (TerrainSampleChange change in redone!.Heights)
        {
            Assert.Equal(original[change.Index], change.Before);
            Assert.Equal(painted[change.Index], change.After);
        }
        Assert.Null(session.Redo());

        session.Undo();
        Assert.True(session.CanRedo);
        session.PaintHeight(2, 2, 1, TerrainHeightOperation.Raise, 5);
        Assert.False(session.CanRedo); // 新筆劃清除重做堆疊
    }

    [Fact]
    public void Painting_back_to_original_within_one_stroke_leaves_no_pending_change()
    {
        const int size = 9;
        var session = new TerrainHeightEditSession(size, Filled(size, 100), null, 0, null);

        // 半徑 0.5 只涵蓋中心頂點。
        IReadOnlyList<TerrainSampleChange> raised = session.PaintHeight(4, 4, .5f, TerrainHeightOperation.Raise, 10);
        IReadOnlyList<TerrainSampleChange> lowered = session.PaintHeight(4, 4, .5f, TerrainHeightOperation.Lower, 10);

        Assert.Equal(new TerrainSampleChange(4 * size + 4, 100, 110), Assert.Single(raised));
        Assert.Equal(new TerrainSampleChange(4 * size + 4, 110, 100), Assert.Single(lowered));
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        Assert.False(session.CommitStroke());
        Assert.Null(session.Undo());
    }

    // ---------- 通行區域 ----------

    [Fact]
    public void PaintCollision_sets_block_and_clear_only_inside_radius()
    {
        const int size = 8;
        var session = new TerrainHeightEditSession(9, Filled(9, 100), null, size, Filled(size, 0));
        int[] expected = [3 * size + 3, 3 * size + 4, 4 * size + 3, 4 * size + 4]; // 像素中心距 (4,4) ≤ 1

        IReadOnlyList<TerrainSampleChange> blocked = session.PaintCollision(4, 4, 1, TerrainCollisionOperation.Block);

        Assert.Equal(expected, blocked.Select(change => change.Index).OrderBy(index => index).ToArray());
        Assert.All(blocked, change => { Assert.Equal(0, change.Before); Assert.Equal(255, change.After); });
        for (int index = 0; index < size * size; index++)
            Assert.Equal(expected.Contains(index) ? 255 : 0, session.Collision![index]);
        Assert.True(session.HasCollision);
        Assert.True(session.CollisionDirty);
        Assert.False(session.HeightsDirty);
        Assert.True(session.IsDirty);
        Assert.Empty(session.PaintCollision(4, 4, 1, TerrainCollisionOperation.Block)); // 已是目標值

        var clearSession = new TerrainHeightEditSession(9, Filled(9, 100), null, size, Filled(size, 255));
        IReadOnlyList<TerrainSampleChange> cleared = clearSession.PaintCollision(4, 4, 1, TerrainCollisionOperation.Clear);
        Assert.Equal(expected, cleared.Select(change => change.Index).OrderBy(index => index).ToArray());
        for (int index = 0; index < size * size; index++)
            Assert.Equal(expected.Contains(index) ? 0 : 255, clearSession.Collision![index]);
        Assert.True(clearSession.CollisionDirty);

        var noCollision = new TerrainHeightEditSession(9, Filled(9, 100), null, 0, null);
        Assert.False(noCollision.HasCollision);
        Assert.Null(noCollision.Collision);
        Assert.Empty(noCollision.PaintCollision(4, 4, 2, TerrainCollisionOperation.Block));
        Assert.False(noCollision.CollisionDirty);
        Assert.False(noCollision.IsDirty);
    }

    // ---------- 基準 ----------

    [Fact]
    public void ResetToBaseline_restores_everything_and_CommitBaseline_sets_new_baseline()
    {
        const int size = 9, collisionSize = 8;
        byte[] heights = Filled(size, 100), collision = Filled(collisionSize, 0);
        var session = new TerrainHeightEditSession(size, heights, null, collisionSize, collision);

        session.PaintHeight(4, 4, 2, TerrainHeightOperation.Raise, 20);
        session.PaintCollision(2, 2, 1, TerrainCollisionOperation.Block);
        session.CommitStroke();
        session.PaintHeight(2, 2, 2, TerrainHeightOperation.Lower, 20); // 未提交
        session.Undo();
        session.PaintCollision(6, 6, 1, TerrainCollisionOperation.Block);
        Assert.True(session.IsDirty);

        session.ResetToBaseline();

        Assert.Equal(heights, session.Heights.ToArray());
        Assert.Equal(collision, session.Collision!.ToArray());
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        Assert.False(session.CanRedo);

        session.PaintHeight(4, 4, 2, TerrainHeightOperation.Raise, 20);
        session.PaintCollision(2, 2, 1, TerrainCollisionOperation.Block);
        byte[] savedHeights = session.Heights.ToArray(), savedCollision = session.Collision!.ToArray();

        session.CommitBaseline(null);

        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        Assert.False(session.CanRedo);
        Assert.Equal(savedHeights, session.Heights.ToArray());

        session.PaintHeight(6, 6, 2, TerrainHeightOperation.Lower, 20);
        session.PaintCollision(6, 6, 1, TerrainCollisionOperation.Block);
        Assert.True(session.IsDirty);
        session.ResetToBaseline();
        Assert.Equal(savedHeights, session.Heights.ToArray());
        Assert.Equal(savedCollision, session.Collision!.ToArray());
        Assert.False(session.IsDirty);
    }

    // ---------- emboss 光照 ----------

    [Fact]
    public void EmbossLightModel_Fit_recovers_linear_lighting_and_rejects_constant_emboss()
    {
        const int size = 33;
        byte[] heights = SmoothField(size);
        byte[] emboss = SyntheticEmboss(size, heights);

        EmbossLightModel model = EmbossLightModel.Fit(size, heights, emboss);

        Assert.True(model.IsUsable);
        Assert.InRange(model.Intercept, 119.9f, 120.1f);
        Assert.InRange(model.SlopeX, 2.99f, 3.01f);
        Assert.InRange(model.SlopeY, -2.01f, -1.99f);
        Assert.True(model.RSquared > .95, $"R² = {model.RSquared}");

        EmbossLightModel constant = EmbossLightModel.Fit(size, heights, Filled(size, 180));
        Assert.False(constant.IsUsable);
        Assert.Equal(0, constant.RSquared);

        var session = new TerrainHeightEditSession(size, heights, emboss, 0, null);
        Assert.True(session.LightFitQuality > .95);
        Assert.Equal(0, new TerrainHeightEditSession(size, heights, null, 0, null).LightFitQuality);
    }

    [Fact]
    public void BuildEmboss_keeps_untouched_vertices_and_adjusts_changed_slopes_by_fitted_light()
    {
        const int size = 33;
        byte[] heights = SmoothField(size);
        byte[] emboss = SyntheticEmboss(size, heights);
        var session = new TerrainHeightEditSession(size, heights, emboss, 0, null);
        Assert.True(session.HasEmboss);

        byte[]? unchanged = session.BuildEmboss();
        Assert.NotNull(unchanged);
        Assert.NotSame(emboss, unchanged);
        Assert.Equal(emboss, unchanged);

        session.PaintHeight(16, 16, 5, TerrainHeightOperation.Raise, 20);
        byte[] edited = session.Heights.ToArray();
        byte[]? rebuilt = session.BuildEmboss();

        Assert.NotNull(rebuilt);
        int adjusted = 0;
        for (int y = 0; y < size; y++)
        for (int x = 0; x < size; x++)
        {
            int index = y * size + x;
            (float gx, float gy) = TerrainHeightEditSession.Gradient(edited, size, x, y);
            (float bx, float by) = TerrainHeightEditSession.Gradient(heights, size, x, y);
            if (gx == bx && gy == by)
            {
                Assert.Equal(emboss[index], rebuilt![index]); // 坡度未變：保留原版烘焙值
                continue;
            }
            double predicted = emboss[index] + 3 * (gx - bx) - 2 * (gy - by);
            int expected = Math.Clamp((int)Math.Round(predicted), 0, 255);
            Assert.InRange(rebuilt![index], expected - 1, expected + 1);
            double shift = predicted - emboss[index];
            if (Math.Abs(shift) >= 2 && expected is > 0 and < 255)
                Assert.Equal(Math.Sign(shift), Math.Sign(rebuilt[index] - emboss[index]));
            if (rebuilt[index] != emboss[index]) adjusted++;
        }
        Assert.True(adjusted > 0);

        var noEmboss = new TerrainHeightEditSession(size, heights, null, 0, null);
        noEmboss.PaintHeight(16, 16, 5, TerrainHeightOperation.Raise, 20);
        Assert.False(noEmboss.HasEmboss);
        Assert.Null(noEmboss.BuildEmboss());
    }

    // ---------- BMP 讀寫 ----------

    [Fact]
    public void EncodeWithGreen_writes_padded_bottom_up_24bit_bmp_and_round_trips_through_Read()
    {
        const int width = 9, height = 5, stride = 28; // 9×3 = 27 → 補齊至 4 的倍數
        TerrainLayer original = ColorfulLayer(width, height);
        byte[] values = original.Green.ToArray();
        int changedA = 0 * width + 1, changedB = 4 * width + 8;
        values[changedA] = (byte)(original.Green[changedA] ^ 0x5A);
        values[changedB] = (byte)(original.Green[changedB] ^ 0xA5);

        byte[] bmp = TerrainLayerFiles.EncodeWithGreen(original, values);

        Assert.Equal((byte)'B', bmp[0]);
        Assert.Equal((byte)'M', bmp[1]);
        Assert.Equal(54 + stride * height, bmp.Length);
        Assert.Equal(bmp.Length, BitConverter.ToInt32(bmp, 2));
        Assert.Equal(54, BitConverter.ToInt32(bmp, 10));
        Assert.Equal(40, BitConverter.ToInt32(bmp, 14));
        Assert.Equal(width, BitConverter.ToInt32(bmp, 18));
        Assert.Equal(height, BitConverter.ToInt32(bmp, 22)); // 正值 = bottom-up
        Assert.Equal(1, BitConverter.ToInt16(bmp, 26));
        Assert.Equal(24, BitConverter.ToInt16(bmp, 28));
        Assert.Equal(stride * height, BitConverter.ToInt32(bmp, 34));
        for (int y = 0; y < height; y++)
        {
            int row = 54 + (height - 1 - y) * stride;
            Assert.Equal(0, bmp[row + width * 3]); // 補齊位元組
            for (int x = 0; x < width; x++)
            {
                int index = y * width + x, offset = row + x * 3, pixel = original.Argb[index];
                if (index == changedA || index == changedB)
                {
                    Assert.Equal(values[index], bmp[offset]);
                    Assert.Equal(values[index], bmp[offset + 1]);
                    Assert.Equal(values[index], bmp[offset + 2]);
                }
                else
                {
                    Assert.Equal((byte)pixel, bmp[offset]);
                    Assert.Equal((byte)(pixel >> 8), bmp[offset + 1]);
                    Assert.Equal((byte)(pixel >> 16), bmp[offset + 2]);
                }
            }
        }
        Assert.Throws<ArgumentException>(() => TerrainLayerFiles.EncodeWithGreen(original, new byte[width * height - 1]));

        Directory.CreateDirectory(_root);
        string path = Path.Combine(_root, "boden.bmp");
        using (var rollback = new FileRollbackScope())
        {
            TerrainLayerFiles.Write(path, original, values, rollback);
            rollback.Commit();
        }
        Assert.Equal(bmp, File.ReadAllBytes(path));

        TerrainLayer? read = TerrainLayerFiles.Read(path);

        Assert.NotNull(read);
        Assert.Equal(width, read!.Width);
        Assert.Equal(height, read.Height);
        Assert.Equal(values, read.Green);
        for (int index = 0; index < values.Length; index++)
        {
            if (index == changedA || index == changedB)
                Assert.Equal(Argb(values[index], values[index], values[index]), read.Argb[index]);
            else
                Assert.Equal(original.Argb[index], read.Argb[index]);
        }
        Assert.Null(TerrainLayerFiles.Read(Path.Combine(_root, "missing.bmp")));
    }

    // ---------- 高度相依快取 ----------

    [Fact]
    public void InvalidateHeightCaches_deletes_only_existing_caches_and_rollback_restores_them()
    {
        Directory.CreateDirectory(_root);
        var skydens = new byte[] { 1, 2, 3, 4, 5 };
        var shadows = new byte[] { 9, 8, 7 };
        File.WriteAllBytes(Path.Combine(_root, "skydens.dat"), skydens);
        File.WriteAllBytes(Path.Combine(_root, "shadows.dat"), shadows);
        File.SetAttributes(Path.Combine(_root, "shadows.dat"), FileAttributes.ReadOnly);
        File.WriteAllBytes(Path.Combine(_root, "boden.bmp"), [42]);
        File.WriteAllBytes(Path.Combine(_root, "other.dat"), [7]);

        using (var rollback = new FileRollbackScope())
        {
            IReadOnlyList<string> removed = TerrainLayerFiles.InvalidateHeightCaches(_root, rollback);

            Assert.Equal(new[] { "skydens.dat", "shadows.dat" }, removed);
            Assert.False(File.Exists(Path.Combine(_root, "skydens.dat")));
            Assert.False(File.Exists(Path.Combine(_root, "shadows.dat")));
            Assert.False(File.Exists(Path.Combine(_root, "visible.dat")));
            Assert.False(File.Exists(Path.Combine(_root, "cliprect.dat")));
            Assert.True(File.Exists(Path.Combine(_root, "boden.bmp")));
            Assert.True(File.Exists(Path.Combine(_root, "other.dat")));
            // 未 Commit 即 Dispose → 還原。
        }

        Assert.Equal(skydens, File.ReadAllBytes(Path.Combine(_root, "skydens.dat")));
        Assert.Equal(shadows, File.ReadAllBytes(Path.Combine(_root, "shadows.dat")));
        Assert.False(File.Exists(Path.Combine(_root, "visible.dat")));
        Assert.False(File.Exists(Path.Combine(_root, "cliprect.dat")));

        using (var rollback = new FileRollbackScope())
        {
            Assert.Equal(new[] { "skydens.dat", "shadows.dat" }, TerrainLayerFiles.InvalidateHeightCaches(_root, rollback));
            rollback.Commit();
        }

        Assert.False(File.Exists(Path.Combine(_root, "skydens.dat")));
        Assert.False(File.Exists(Path.Combine(_root, "shadows.dat")));
        Assert.Equal(new byte[] { 42 }, File.ReadAllBytes(Path.Combine(_root, "boden.bmp")));
        Assert.Equal(new byte[] { 7 }, File.ReadAllBytes(Path.Combine(_root, "other.dat")));

        using (var rollback = new FileRollbackScope())
            Assert.Empty(TerrainLayerFiles.InvalidateHeightCaches(_root, rollback));
    }

    public void Dispose()
    {
        if (!Directory.Exists(_root)) return;
        foreach (string file in Directory.EnumerateFiles(_root, "*", SearchOption.AllDirectories))
            File.SetAttributes(file, FileAttributes.Normal);
        Directory.Delete(_root, true);
    }

    // ---------- 合成資料 ----------

    private static byte[] Filled(int size, byte value) => Enumerable.Repeat(value, size * size).ToArray();

    /// <summary>平滑、非對稱的高度場，讓 X／Y 坡度都有變化且彼此不共線。</summary>
    private static byte[] SmoothField(int size)
    {
        var heights = new byte[size * size];
        for (int y = 0; y < size; y++)
        for (int x = 0; x < size; x++)
            heights[y * size + x] = (byte)Math.Round(128 + 30 * Math.Sin(x * .3) * Math.Cos(y * .25) + 15 * Math.Sin(.17 * x + .11 * y));
        return heights;
    }

    /// <summary>emboss = clamp(120 + 3·gx − 2·gy)，gx/gy 取自與編輯器相同的中央差分。</summary>
    private static byte[] SyntheticEmboss(int size, byte[] heights)
    {
        var emboss = new byte[size * size];
        for (int y = 0; y < size; y++)
        for (int x = 0; x < size; x++)
        {
            (float gx, float gy) = TerrainHeightEditSession.Gradient(heights, size, x, y);
            emboss[y * size + x] = (byte)Math.Clamp((int)Math.Round(120 + 3 * gx - 2 * gy), 0, 255);
        }
        return emboss;
    }

    private static TerrainLayer ColorfulLayer(int width, int height)
    {
        var argb = new int[width * height];
        var green = new byte[argb.Length];
        for (int y = 0; y < height; y++)
        for (int x = 0; x < width; x++)
        {
            int index = y * width + x;
            argb[index] = Argb((byte)(x * 20 + y), (byte)(x * 7 + y * 13), (byte)(255 - x * 3 - y));
            green[index] = (byte)(argb[index] >> 8);
        }
        return new TerrainLayer(width, height, argb, green);
    }

    private static int Argb(byte r, byte g, byte b) => unchecked((int)(0xFF000000u | (uint)r << 16 | (uint)g << 8 | b));
}
