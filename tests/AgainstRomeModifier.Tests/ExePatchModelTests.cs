using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 涵蓋 ExePatchModel 的狀態偵測與「state → 預期/取代位元組」選擇邏輯——
/// 這是先前重構後唯一沒有測試覆蓋的高風險決策點。全部使用合成 exe 緩衝區，
/// 不需要任何版權遊戲檔案。
/// </summary>
public sealed class ExePatchModelTests {
    // 需容納最大偏移（村落 setter cave 0x16258f + 39）。
    private const int ExeSize = 0x163000;

    private static byte[] NewExe() => new byte[ExeSize];

    private static void Place(byte[] exe, long offset, byte[] bytes) =>
        Buffer.BlockCopy(bytes, 0, exe, (int)offset, bytes.Length);

    private static void PlaceFocus(byte[] exe, ExePatchState state) =>
        Place(exe, ExePatchModel.FocusPatchOffset,
            state == ExePatchState.FocusPatched ? ExePatchModel.FocusPatchedBytes : ExePatchModel.FocusOriginalBytes);

    private static void PlaceSpellAltar(byte[] exe, ExeSpellAltarPatchState state) {
        foreach (var site in ExePatchModel.SpellAltarPatchSites) {
            Place(exe, site.Offset, state == ExeSpellAltarPatchState.Patched ? site.Patched : site.Original);
        }
    }

    private static void PlaceVillageRange(byte[] exe, ExeVillageRangePatchState state) {
        bool rangePatched = state is ExeVillageRangePatchState.LegacyLogicOnly or ExeVillageRangePatchState.Expanded;
        bool framePatched = state == ExeVillageRangePatchState.Expanded;
        Place(exe, ExePatchModel.VillageRangeXPatchOffset, rangePatched ? ExePatchModel.VillageRangeXPatchedBytes : ExePatchModel.VillageRangeXOriginalBytes);
        Place(exe, ExePatchModel.VillageRangeZPatchOffset, rangePatched ? ExePatchModel.VillageRangeZPatchedBytes : ExePatchModel.VillageRangeZOriginalBytes);
        Place(exe, ExePatchModel.VillageFrameXPatchOffset, framePatched ? ExePatchModel.VillageFrameXPatchedBytes : ExePatchModel.VillageFrameXOriginalBytes);
        Place(exe, ExePatchModel.VillageFrameZPatchOffset, framePatched ? ExePatchModel.VillageFrameZPatchedBytes : ExePatchModel.VillageFrameZOriginalBytes);
    }

    private static void PlaceVillageSetter(byte[] exe, ExeVillageSetterPatchState state) {
        byte[] hook = state == ExeVillageSetterPatchState.Original
            ? ExePatchModel.VillageSetterHookOriginalBytes
            : ExePatchModel.VillageSetterHookPatchedBytes;
        byte[] cave = state switch {
            ExeVillageSetterPatchState.Original => ExePatchModel.VillageSetterCaveOriginalBytes,
            ExeVillageSetterPatchState.Legacy2x => ExePatchModel.VillageSetterCaveLegacy2xBytes,
            ExeVillageSetterPatchState.Legacy2Point5x => ExePatchModel.VillageSetterCaveLegacy2Point5xBytes,
            ExeVillageSetterPatchState.Legacy3x => ExePatchModel.VillageSetterCaveLegacy3xBytes,
            ExeVillageSetterPatchState.Expanded5x => ExePatchModel.VillageSetterCavePatchedBytes,
            _ => throw new ArgumentOutOfRangeException(nameof(state)),
        };
        Place(exe, ExePatchModel.VillageSetterHookOffset, hook);
        Place(exe, ExePatchModel.VillageSetterCaveOffset, cave);
    }

    // ---- 狀態偵測 ----

    [Theory]
    [InlineData(ExePatchState.Original)]
    [InlineData(ExePatchState.FocusPatched)]
    public void Focus_state_is_detected(ExePatchState state) {
        byte[] exe = NewExe();
        PlaceFocus(exe, state);
        Assert.Equal(state, ExePatchModel.GetExePatchState(exe));
    }

    [Fact]
    public void Focus_unknown_bytes_and_short_buffer_report_unknown() {
        byte[] exe = NewExe();
        Place(exe, ExePatchModel.FocusPatchOffset, new byte[] { 0x11, 0x22, 0x33, 0x44, 0x55, 0x66 });
        Assert.Equal(ExePatchState.Unknown, ExePatchModel.GetExePatchState(exe));
        Assert.Equal(ExePatchState.Unknown, ExePatchModel.GetExePatchState(new byte[8]));
    }

    [Theory]
    [InlineData(ExeVillageRangePatchState.Original)]
    [InlineData(ExeVillageRangePatchState.LegacyLogicOnly)]
    [InlineData(ExeVillageRangePatchState.Expanded)]
    public void Village_range_state_is_detected(ExeVillageRangePatchState state) {
        byte[] exe = NewExe();
        PlaceVillageRange(exe, state);
        Assert.Equal(state, ExePatchModel.GetVillageBuildRangePatchState(exe));
    }

    [Theory]
    [InlineData(ExeVillageSetterPatchState.Original)]
    [InlineData(ExeVillageSetterPatchState.Legacy2x)]
    [InlineData(ExeVillageSetterPatchState.Legacy2Point5x)]
    [InlineData(ExeVillageSetterPatchState.Legacy3x)]
    [InlineData(ExeVillageSetterPatchState.Expanded5x)]
    public void Village_setter_state_is_detected(ExeVillageSetterPatchState state) {
        byte[] exe = NewExe();
        PlaceVillageSetter(exe, state);
        Assert.Equal(state, ExePatchModel.GetVillageSetterPatchState(exe));
    }

    // ---- Focus 套用/還原 round-trip ----

    [Fact]
    public void Focus_enable_then_disable_round_trips() {
        byte[] exe = NewExe();
        PlaceFocus(exe, ExePatchState.Original);
        byte[] pristine = exe.ToArray();

        ExePatchModel.Apply(exe, ExePatchModel.PlanFocus(true, ExePatchModel.GetExePatchState(exe)));
        Assert.Equal(ExePatchState.FocusPatched, ExePatchModel.GetExePatchState(exe));

        ExePatchModel.Apply(exe, ExePatchModel.PlanFocus(false, ExePatchModel.GetExePatchState(exe)));
        Assert.Equal(ExePatchState.Original, ExePatchModel.GetExePatchState(exe));
        Assert.Equal(pristine, exe);
    }

    [Fact]
    public void Focus_plan_is_empty_when_already_in_target_state() {
        byte[] exe = NewExe();
        PlaceFocus(exe, ExePatchState.FocusPatched);
        Assert.Empty(ExePatchModel.PlanFocus(true, ExePatchModel.GetExePatchState(exe)));
        PlaceFocus(exe, ExePatchState.Original);
        Assert.Empty(ExePatchModel.PlanFocus(false, ExePatchModel.GetExePatchState(exe)));
    }

    // ---- 法術祭壇套用/還原 round-trip ----

    [Fact]
    public void Spell_altar_enable_then_disable_round_trips() {
        byte[] exe = NewExe();
        PlaceSpellAltar(exe, ExeSpellAltarPatchState.Original);
        byte[] pristine = exe.ToArray();

        ExePatchModel.Apply(exe, ExePatchModel.PlanSpellAltar(true, ExePatchModel.GetSpellAltarPatchState(exe)));
        Assert.Equal(ExeSpellAltarPatchState.Patched, ExePatchModel.GetSpellAltarPatchState(exe));

        ExePatchModel.Apply(exe, ExePatchModel.PlanSpellAltar(false, ExePatchModel.GetSpellAltarPatchState(exe)));
        Assert.Equal(ExeSpellAltarPatchState.Original, ExePatchModel.GetSpellAltarPatchState(exe));
        Assert.Equal(pristine, exe);
    }

    // ---- 村落 setter：每個舊版狀態都能安全遷移到 3x，並可還原回原版 ----

    [Theory]
    [InlineData(ExeVillageSetterPatchState.Original)]
    [InlineData(ExeVillageSetterPatchState.Legacy2x)]
    [InlineData(ExeVillageSetterPatchState.Legacy2Point5x)]
    [InlineData(ExeVillageSetterPatchState.Legacy3x)]
    public void Village_setter_enable_from_any_prior_state_reaches_expanded5x(ExeVillageSetterPatchState start) {
        byte[] exe = NewExe();
        PlaceVillageSetter(exe, start);

        ExePatchModel.Apply(exe, ExePatchModel.PlanVillageSetter(true, ExePatchModel.GetVillageSetterPatchState(exe)));

        Assert.Equal(ExeVillageSetterPatchState.Expanded5x, ExePatchModel.GetVillageSetterPatchState(exe));
    }

    [Theory]
    [InlineData(ExeVillageSetterPatchState.Legacy2x)]
    [InlineData(ExeVillageSetterPatchState.Legacy2Point5x)]
    [InlineData(ExeVillageSetterPatchState.Legacy3x)]
    [InlineData(ExeVillageSetterPatchState.Expanded5x)]
    public void Village_setter_disable_from_any_patched_state_restores_original(ExeVillageSetterPatchState start) {
        byte[] exe = NewExe();
        PlaceVillageSetter(exe, ExeVillageSetterPatchState.Original);
        byte[] pristine = exe.ToArray();
        PlaceVillageSetter(exe, start);

        ExePatchModel.Apply(exe, ExePatchModel.PlanVillageSetter(false, ExePatchModel.GetVillageSetterPatchState(exe)));

        Assert.Equal(ExeVillageSetterPatchState.Original, ExePatchModel.GetVillageSetterPatchState(exe));
        Assert.Equal(pristine, exe);
    }

    [Fact]
    public void Village_setter_enable_noop_when_already_expanded5x() {
        byte[] exe = NewExe();
        PlaceVillageSetter(exe, ExeVillageSetterPatchState.Expanded5x);
        Assert.Empty(ExePatchModel.PlanVillageSetter(true, ExePatchModel.GetVillageSetterPatchState(exe)));
    }

    // ---- 舊版村落範圍候選補丁：偵測到就還原成四處原版 bytes ----

    [Theory]
    [InlineData(ExeVillageRangePatchState.LegacyLogicOnly)]
    [InlineData(ExeVillageRangePatchState.Expanded)]
    public void Village_range_restore_returns_to_original(ExeVillageRangePatchState start) {
        byte[] exe = NewExe();
        PlaceVillageRange(exe, ExeVillageRangePatchState.Original);
        byte[] pristine = exe.ToArray();
        PlaceVillageRange(exe, start);

        ExePatchModel.Apply(exe, ExePatchModel.PlanVillageRangeRestore(ExePatchModel.GetVillageBuildRangePatchState(exe)));

        Assert.Equal(ExeVillageRangePatchState.Original, ExePatchModel.GetVillageBuildRangePatchState(exe));
        Assert.Equal(pristine, exe);
    }

    [Fact]
    public void Village_range_restore_is_empty_when_already_original() {
        byte[] exe = NewExe();
        PlaceVillageRange(exe, ExeVillageRangePatchState.Original);
        Assert.Empty(ExePatchModel.PlanVillageRangeRestore(ExePatchModel.GetVillageBuildRangePatchState(exe)));
    }

    // ---- 版本不符防護：預期位元組不符時中止且不破壞緩衝區 ----

    [Fact]
    public void Apply_aborts_and_preserves_buffer_when_expected_bytes_mismatch() {
        byte[] exe = NewExe();
        // 目前是「已修補」狀態，但計畫誤以為是「原版」→ 預期位元組不符，必須中止。
        PlaceFocus(exe, ExePatchState.FocusPatched);
        byte[] before = exe.ToArray();

        IReadOnlyList<ExeWriteOp> wrongPlan = ExePatchModel.PlanFocus(true, ExePatchState.Original);
        Assert.Throws<InvalidDataException>(() => ExePatchModel.Apply(exe, wrongPlan));
        Assert.Equal(before, exe);
    }
}
