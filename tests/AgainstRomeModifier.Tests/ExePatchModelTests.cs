using AgainstRomeModifier.Core.Patches;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 涵蓋 ExePatchModel 的狀態偵測與「state → 預期/取代位元組」選擇邏輯——
/// 這是先前重構後唯一沒有測試覆蓋的高風險決策點。全部使用合成 exe 緩衝區，
/// 不需要任何版權遊戲檔案。
/// </summary>
public sealed class ExePatchModelTests {
    // 需容納最大偏移（攝影機 code cave 0x1625c0 + 28）。
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

    private static void PlaceRomanEndless(byte[] exe, ExeRomanEndlessPatchState state) =>
        Place(exe, ExePatchModel.RomanEndlessPatchOffset,
            state == ExeRomanEndlessPatchState.Patched
                ? ExePatchModel.RomanEndlessPatchedBytes
                : ExePatchModel.RomanEndlessOriginalBytes);

    private static void PlaceNativeWidescreen(byte[] exe, ExeNativeWidescreenPatchState state) {
        bool modesPatched = state != ExeNativeWidescreenPatchState.Original;
        bool forcePatched = state is ExeNativeWidescreenPatchState.LegacyForcedStaleUi or ExeNativeWidescreenPatchState.Patched;
        bool uiPatched = state == ExeNativeWidescreenPatchState.Patched;
        Place(exe, ExePatchModel.NativeWidescreenIdentifyOffset, modesPatched ? ExePatchModel.NativeWidescreenIdentifyPatchedBytes : ExePatchModel.NativeWidescreenIdentifyOriginalBytes);
        Place(exe, ExePatchModel.NativeWidescreenCreateOffset, modesPatched ? ExePatchModel.NativeWidescreenCreatePatchedBytes : ExePatchModel.NativeWidescreenCreateOriginalBytes);
        Place(exe, ExePatchModel.NativeWidescreenModeTextOffset, modesPatched ? ExePatchModel.NativeWidescreenModeTextPatchedBytes : ExePatchModel.NativeWidescreenModeTextOriginalBytes);
        Place(exe, ExePatchModel.NativeWidescreenForceModeOffset, forcePatched ? ExePatchModel.NativeWidescreenForceModePatchedBytes : ExePatchModel.NativeWidescreenForceModeOriginalBytes);
        Place(exe, ExePatchModel.NativeWidescreenActiveModeGetterOffset, uiPatched ? ExePatchModel.NativeWidescreenActiveModeGetterPatchedBytes : ExePatchModel.NativeWidescreenActiveModeGetterOriginalBytes);
        Place(exe, ExePatchModel.NativeWidescreenIgmDialogModeOffset, uiPatched ? ExePatchModel.NativeWidescreenIgmDialogModePatchedBytes : ExePatchModel.NativeWidescreenIgmDialogModeOriginalBytes);
    }

    private static void PlaceCameraZoomOut(byte[] exe, ExeCameraZoomOutPatchState state) {
        bool patched = state is ExeCameraZoomOutPatchState.LegacyZoomHalf or ExeCameraZoomOutPatchState.Patched;
        Place(exe, ExePatchModel.CameraZoomInitCallOffset, patched ? ExePatchModel.CameraZoomInitCallPatchedBytes : ExePatchModel.CameraZoomInitCallOriginalBytes);
        Place(exe, ExePatchModel.CameraZoomLoadCallOffset, patched ? ExePatchModel.CameraZoomLoadCallPatchedBytes : ExePatchModel.CameraZoomLoadCallOriginalBytes);
        Place(exe, ExePatchModel.CameraZoomScriptCallOffset, patched ? ExePatchModel.CameraZoomScriptCallPatchedBytes : ExePatchModel.CameraZoomScriptCallOriginalBytes);
        byte[] cave = state switch {
            ExeCameraZoomOutPatchState.Original => ExePatchModel.CameraZoomCaveOriginalBytes,
            ExeCameraZoomOutPatchState.LegacyZoomHalf => ExePatchModel.CameraZoomCaveLegacyZoomHalfBytes,
            ExeCameraZoomOutPatchState.Patched => ExePatchModel.CameraZoomCavePatchedBytes,
            _ => throw new ArgumentOutOfRangeException(nameof(state)),
        };
        Place(exe, ExePatchModel.CameraZoomCaveOffset, cave);
    }

    private static void PlaceIdleSelect999(byte[] exe, ExeIdleSelect999PatchState state) =>
        Place(exe, ExePatchModel.IdleSelect999PatchOffset,
            state == ExeIdleSelect999PatchState.Patched
                ? ExePatchModel.IdleSelect999PatchedBytes
                : ExePatchModel.IdleSelect999OriginalBytes);

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
            ExeVillageSetterPatchState.Legacy5x => ExePatchModel.VillageSetterCaveLegacy5xBytes,
            ExeVillageSetterPatchState.EntireMap => ExePatchModel.VillageSetterCavePatchedBytes,
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
    [InlineData(ExeRomanEndlessPatchState.Original)]
    [InlineData(ExeRomanEndlessPatchState.Patched)]
    public void Roman_endless_state_is_detected(ExeRomanEndlessPatchState state) {
        byte[] exe = NewExe();
        PlaceRomanEndless(exe, state);
        Assert.Equal(state, ExePatchModel.GetRomanEndlessPatchState(exe));
    }

    [Fact]
    public void Roman_endless_unknown_bytes_and_short_buffer_report_unknown() {
        byte[] exe = NewExe();
        Place(exe, ExePatchModel.RomanEndlessPatchOffset, Enumerable.Repeat((byte)0xCC, 21).ToArray());
        Assert.Equal(ExeRomanEndlessPatchState.Unknown, ExePatchModel.GetRomanEndlessPatchState(exe));
        Assert.Equal(ExeRomanEndlessPatchState.Unknown, ExePatchModel.GetRomanEndlessPatchState(new byte[8]));
        Assert.Empty(ExePatchModel.PlanRomanEndless(true, ExeRomanEndlessPatchState.Unknown));
        Assert.Empty(ExePatchModel.PlanRomanEndless(false, ExeRomanEndlessPatchState.Unknown));
    }

    [Theory]
    [InlineData(ExeNativeWidescreenPatchState.Original)]
    [InlineData(ExeNativeWidescreenPatchState.Patched)]
    public void Native_widescreen_state_is_detected(ExeNativeWidescreenPatchState state) {
        byte[] exe = new byte[0x1DD000];
        PlaceNativeWidescreen(exe, state);
        Assert.Equal(state, ExePatchModel.GetNativeWidescreenPatchState(exe));
    }

    [Fact]
    public void Native_widescreen_round_trip_changes_all_six_verified_sites() {
        byte[] exe = new byte[0x1DD000];
        PlaceNativeWidescreen(exe, ExeNativeWidescreenPatchState.Original);

        ExePatchModel.Apply(exe, ExePatchModel.PlanNativeWidescreen(true,
            ExePatchModel.GetNativeWidescreenPatchState(exe)));
        Assert.Equal(ExeNativeWidescreenPatchState.Patched,
            ExePatchModel.GetNativeWidescreenPatchState(exe));
        Assert.Contains("1920x1080", System.Text.Encoding.ASCII.GetString(
            exe, (int)ExePatchModel.NativeWidescreenModeTextOffset,
            ExePatchModel.NativeWidescreenModeTextPatchedBytes.Length));

        ExePatchModel.Apply(exe, ExePatchModel.PlanNativeWidescreen(false,
            ExePatchModel.GetNativeWidescreenPatchState(exe)));
        Assert.Equal(ExeNativeWidescreenPatchState.Original,
            ExePatchModel.GetNativeWidescreenPatchState(exe));
    }

    [Fact]
    public void Native_widescreen_legacy_unforced_patch_migrates_and_restores() {
        byte[] exe = new byte[0x1DD000];
        PlaceNativeWidescreen(exe, ExeNativeWidescreenPatchState.LegacyUnforced);

        Assert.Equal(ExeNativeWidescreenPatchState.LegacyUnforced,
            ExePatchModel.GetNativeWidescreenPatchState(exe));
        IReadOnlyList<ExeWriteOp> migration = ExePatchModel.PlanNativeWidescreen(
            true, ExeNativeWidescreenPatchState.LegacyUnforced);
        Assert.Equal(3, migration.Count);
        ExePatchModel.Apply(exe, migration);
        Assert.Equal(ExeNativeWidescreenPatchState.Patched,
            ExePatchModel.GetNativeWidescreenPatchState(exe));

        ExePatchModel.Apply(exe, ExePatchModel.PlanNativeWidescreen(
            false, ExePatchModel.GetNativeWidescreenPatchState(exe)));
        Assert.Equal(ExeNativeWidescreenPatchState.Original,
            ExePatchModel.GetNativeWidescreenPatchState(exe));
    }

    [Fact]
    public void Native_widescreen_forced_stale_ui_patch_migrates_and_restores() {
        byte[] exe = new byte[0x1DD000];
        PlaceNativeWidescreen(exe, ExeNativeWidescreenPatchState.LegacyForcedStaleUi);

        Assert.Equal(ExeNativeWidescreenPatchState.LegacyForcedStaleUi,
            ExePatchModel.GetNativeWidescreenPatchState(exe));
        IReadOnlyList<ExeWriteOp> migration = ExePatchModel.PlanNativeWidescreen(
            true, ExeNativeWidescreenPatchState.LegacyForcedStaleUi);
        Assert.Equal(2, migration.Count);
        ExePatchModel.Apply(exe, migration);
        Assert.Equal(ExeNativeWidescreenPatchState.Patched,
            ExePatchModel.GetNativeWidescreenPatchState(exe));

        ExePatchModel.Apply(exe, ExePatchModel.PlanNativeWidescreen(
            false, ExePatchModel.GetNativeWidescreenPatchState(exe)));
        Assert.Equal(ExeNativeWidescreenPatchState.Original,
            ExePatchModel.GetNativeWidescreenPatchState(exe));
    }

    [Fact]
    public void Native_widescreen_mixed_or_short_state_is_unknown_and_not_planned() {
        byte[] exe = new byte[0x1DD000];
        PlaceNativeWidescreen(exe, ExeNativeWidescreenPatchState.Original);
        Place(exe, ExePatchModel.NativeWidescreenCreateOffset, ExePatchModel.NativeWidescreenCreatePatchedBytes);

        Assert.Equal(ExeNativeWidescreenPatchState.Unknown,
            ExePatchModel.GetNativeWidescreenPatchState(exe));
        Assert.Equal(ExeNativeWidescreenPatchState.Unknown,
            ExePatchModel.GetNativeWidescreenPatchState(new byte[8]));
        Assert.Empty(ExePatchModel.PlanNativeWidescreen(true, ExeNativeWidescreenPatchState.Unknown));
        Assert.Empty(ExePatchModel.PlanNativeWidescreen(false, ExeNativeWidescreenPatchState.Unknown));
    }

    [Theory]
    [InlineData(ExeCameraZoomOutPatchState.Original)]
    [InlineData(ExeCameraZoomOutPatchState.LegacyZoomHalf)]
    [InlineData(ExeCameraZoomOutPatchState.Patched)]
    public void Camera_zoom_out_state_is_detected(ExeCameraZoomOutPatchState state) {
        byte[] exe = NewExe();
        PlaceCameraZoomOut(exe, state);
        Assert.Equal(state, ExePatchModel.GetCameraZoomOutPatchState(exe));
    }

    [Fact]
    public void Camera_zoom_out_current_cave_clamps_to_one_while_legacy_clamps_to_half() {
        Assert.Equal(1.0f, BitConverter.ToSingle(ExePatchModel.CameraZoomCavePatchedBytes, 9));
        Assert.Equal(1.0f, BitConverter.ToSingle(ExePatchModel.CameraZoomCavePatchedBytes, 19));
        Assert.Equal(0.5f, BitConverter.ToSingle(ExePatchModel.CameraZoomCaveLegacyZoomHalfBytes, 9));
        Assert.Equal(0.5f, BitConverter.ToSingle(ExePatchModel.CameraZoomCaveLegacyZoomHalfBytes, 19));
    }

    [Fact]
    public void Camera_zoom_out_round_trip_changes_three_calls_and_shared_cave_safely() {
        byte[] exe = NewExe();
        PlaceCameraZoomOut(exe, ExeCameraZoomOutPatchState.Original);
        byte[] pristine = exe.ToArray();

        IReadOnlyList<ExeWriteOp> enable = ExePatchModel.PlanCameraZoomOut(
            true, ExePatchModel.GetCameraZoomOutPatchState(exe));
        Assert.Equal(ExePatchModel.CameraZoomCaveOffset, enable[0].Offset);
        ExePatchModel.Apply(exe, enable);
        Assert.Equal(ExeCameraZoomOutPatchState.Patched,
            ExePatchModel.GetCameraZoomOutPatchState(exe));
        foreach (long callOffset in new[] {
            ExePatchModel.CameraZoomInitCallOffset,
            ExePatchModel.CameraZoomLoadCallOffset,
            ExePatchModel.CameraZoomScriptCallOffset,
        }) {
            int relative = BitConverter.ToInt32(exe, (int)callOffset + 1);
            Assert.Equal(ExePatchModel.CameraZoomCaveOffset, callOffset + 5 + relative);
        }
        int tailRelative = BitConverter.ToInt32(
            exe, (int)ExePatchModel.CameraZoomCaveOffset + ExePatchModel.CameraZoomCavePatchedBytes.Length - 4);
        Assert.Equal(0x98A30,
            ExePatchModel.CameraZoomCaveOffset + ExePatchModel.CameraZoomCavePatchedBytes.Length + tailRelative);

        IReadOnlyList<ExeWriteOp> disable = ExePatchModel.PlanCameraZoomOut(
            false, ExePatchModel.GetCameraZoomOutPatchState(exe));
        Assert.Equal(ExePatchModel.CameraZoomCaveOffset, disable[^1].Offset);
        ExePatchModel.Apply(exe, disable);
        Assert.Equal(ExeCameraZoomOutPatchState.Original,
            ExePatchModel.GetCameraZoomOutPatchState(exe));
        Assert.Equal(pristine, exe);
    }

    [Fact]
    public void Camera_zoom_out_legacy_zoom_half_migrates_by_replacing_only_the_cave() {
        byte[] exe = NewExe();
        PlaceCameraZoomOut(exe, ExeCameraZoomOutPatchState.LegacyZoomHalf);

        IReadOnlyList<ExeWriteOp> migration = ExePatchModel.PlanCameraZoomOut(
            true, ExePatchModel.GetCameraZoomOutPatchState(exe));
        ExeWriteOp op = Assert.Single(migration);
        Assert.Equal(ExePatchModel.CameraZoomCaveOffset, op.Offset);
        ExePatchModel.Apply(exe, migration);
        Assert.Equal(ExeCameraZoomOutPatchState.Patched,
            ExePatchModel.GetCameraZoomOutPatchState(exe));

        ExePatchModel.Apply(exe, ExePatchModel.PlanCameraZoomOut(
            false, ExePatchModel.GetCameraZoomOutPatchState(exe)));
        Assert.Equal(ExeCameraZoomOutPatchState.Original,
            ExePatchModel.GetCameraZoomOutPatchState(exe));
    }

    [Fact]
    public void Camera_zoom_out_legacy_zoom_half_can_restore_without_migration() {
        byte[] exe = NewExe();
        PlaceCameraZoomOut(exe, ExeCameraZoomOutPatchState.LegacyZoomHalf);

        ExePatchModel.Apply(exe, ExePatchModel.PlanCameraZoomOut(
            false, ExePatchModel.GetCameraZoomOutPatchState(exe)));

        Assert.Equal(ExeCameraZoomOutPatchState.Original,
            ExePatchModel.GetCameraZoomOutPatchState(exe));
    }

    [Fact]
    public void Camera_zoom_out_mixed_or_short_state_is_unknown_and_not_planned() {
        byte[] exe = NewExe();
        PlaceCameraZoomOut(exe, ExeCameraZoomOutPatchState.Original);
        Place(exe, ExePatchModel.CameraZoomLoadCallOffset, ExePatchModel.CameraZoomLoadCallPatchedBytes);

        Assert.Equal(ExeCameraZoomOutPatchState.Unknown,
            ExePatchModel.GetCameraZoomOutPatchState(exe));
        Assert.Equal(ExeCameraZoomOutPatchState.Unknown,
            ExePatchModel.GetCameraZoomOutPatchState(new byte[8]));
        Assert.Empty(ExePatchModel.PlanCameraZoomOut(true, ExeCameraZoomOutPatchState.Unknown));
        Assert.Empty(ExePatchModel.PlanCameraZoomOut(false, ExeCameraZoomOutPatchState.Unknown));
    }

    [Theory]
    [InlineData(ExeIdleSelect999PatchState.Original)]
    [InlineData(ExeIdleSelect999PatchState.Patched)]
    public void Idle_select_999_state_is_detected(ExeIdleSelect999PatchState state) {
        byte[] exe = NewExe();
        PlaceIdleSelect999(exe, state);
        Assert.Equal(state, ExePatchModel.GetIdleSelect999PatchState(exe));
    }

    [Fact]
    public void Idle_select_999_bytes_rewrite_the_whole_function_region_in_place() {
        // 原版函式 0xA0 位元組 + 0x10 對齊填充；補丁版 0x9F + NOP 填充。
        // 兩者必須等長且覆蓋同一 0xB0 區域，否則會破壞下一個函式 0x451E70。
        Assert.Equal(0xB0, ExePatchModel.IdleSelect999OriginalBytes.Length);
        Assert.Equal(0xB0, ExePatchModel.IdleSelect999PatchedBytes.Length);
        // 補丁版的收集上限 push 0x3E7（999）與堆疊框架 0x1F38 必須成對出現。
        Assert.Contains("68E7030000", Convert.ToHexString(ExePatchModel.IdleSelect999PatchedBytes));
        Assert.Contains("81EC381F0000", Convert.ToHexString(ExePatchModel.IdleSelect999PatchedBytes));
        // 原版特徵：push 0x28（40）與框架 0x140。
        Assert.Contains("6A28", Convert.ToHexString(ExePatchModel.IdleSelect999OriginalBytes));
        Assert.Contains("81EC40010000", Convert.ToHexString(ExePatchModel.IdleSelect999OriginalBytes));
    }

    [Fact]
    public void Idle_select_999_round_trip_restores_pristine_bytes() {
        byte[] exe = NewExe();
        PlaceIdleSelect999(exe, ExeIdleSelect999PatchState.Original);
        byte[] pristine = exe.ToArray();

        IReadOnlyList<ExeWriteOp> enable = ExePatchModel.PlanIdleSelect999(
            true, ExePatchModel.GetIdleSelect999PatchState(exe));
        ExeWriteOp op = Assert.Single(enable);
        Assert.Equal(ExePatchModel.IdleSelect999PatchOffset, op.Offset);
        ExePatchModel.Apply(exe, enable);
        Assert.Equal(ExeIdleSelect999PatchState.Patched,
            ExePatchModel.GetIdleSelect999PatchState(exe));

        IReadOnlyList<ExeWriteOp> disable = ExePatchModel.PlanIdleSelect999(
            false, ExePatchModel.GetIdleSelect999PatchState(exe));
        ExePatchModel.Apply(exe, disable);
        Assert.Equal(ExeIdleSelect999PatchState.Original,
            ExePatchModel.GetIdleSelect999PatchState(exe));
        Assert.Equal(pristine, exe);
    }

    [Fact]
    public void Idle_select_999_mixed_or_short_state_is_unknown_and_not_planned() {
        byte[] exe = NewExe();
        PlaceIdleSelect999(exe, ExeIdleSelect999PatchState.Original);
        exe[ExePatchModel.IdleSelect999PatchOffset + 0x31] = 0xCC;

        Assert.Equal(ExeIdleSelect999PatchState.Unknown,
            ExePatchModel.GetIdleSelect999PatchState(exe));
        Assert.Equal(ExeIdleSelect999PatchState.Unknown,
            ExePatchModel.GetIdleSelect999PatchState(new byte[8]));
        Assert.Empty(ExePatchModel.PlanIdleSelect999(true, ExeIdleSelect999PatchState.Unknown));
        Assert.Empty(ExePatchModel.PlanIdleSelect999(false, ExeIdleSelect999PatchState.Unknown));
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
    [InlineData(ExeVillageSetterPatchState.Legacy5x)]
    [InlineData(ExeVillageSetterPatchState.EntireMap)]
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

    [Fact]
    public void Roman_endless_enable_then_disable_round_trips() {
        byte[] exe = NewExe();
        PlaceRomanEndless(exe, ExeRomanEndlessPatchState.Original);
        byte[] pristine = exe.ToArray();

        ExePatchModel.Apply(exe, ExePatchModel.PlanRomanEndless(true,
            ExePatchModel.GetRomanEndlessPatchState(exe)));
        Assert.Equal(ExeRomanEndlessPatchState.Patched, ExePatchModel.GetRomanEndlessPatchState(exe));

        ExePatchModel.Apply(exe, ExePatchModel.PlanRomanEndless(false,
            ExePatchModel.GetRomanEndlessPatchState(exe)));
        Assert.Equal(ExeRomanEndlessPatchState.Original, ExePatchModel.GetRomanEndlessPatchState(exe));
        Assert.Equal(pristine, exe);
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

    // ---- 村落 setter：每個舊版狀態都能安全遷移到 EntireMap，並可還原回原版 ----

    [Theory]
    [InlineData(ExeVillageSetterPatchState.Original)]
    [InlineData(ExeVillageSetterPatchState.Legacy2x)]
    [InlineData(ExeVillageSetterPatchState.Legacy2Point5x)]
    [InlineData(ExeVillageSetterPatchState.Legacy3x)]
    [InlineData(ExeVillageSetterPatchState.Legacy5x)]
    public void Village_setter_enable_from_any_prior_state_reaches_entire_map(ExeVillageSetterPatchState start) {
        byte[] exe = NewExe();
        PlaceVillageSetter(exe, start);

        ExePatchModel.Apply(exe, ExePatchModel.PlanVillageSetter(true, ExePatchModel.GetVillageSetterPatchState(exe)));

        Assert.Equal(ExeVillageSetterPatchState.EntireMap, ExePatchModel.GetVillageSetterPatchState(exe));
    }

    [Theory]
    [InlineData(ExeVillageSetterPatchState.Legacy2x)]
    [InlineData(ExeVillageSetterPatchState.Legacy2Point5x)]
    [InlineData(ExeVillageSetterPatchState.Legacy3x)]
    [InlineData(ExeVillageSetterPatchState.Legacy5x)]
    [InlineData(ExeVillageSetterPatchState.EntireMap)]
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
    public void Village_setter_enable_noop_when_already_entire_map() {
        byte[] exe = NewExe();
        PlaceVillageSetter(exe, ExeVillageSetterPatchState.EntireMap);
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

    // ---- 遊戲整體時脈加速（主時脈常數縮放）----
    // 需容納時脈常數偏移 0x20424c + 8。
    private const int SpeedExeSize = 0x205000;

    private static byte[] NewSpeedExe(int multiplier) {
        byte[] exe = new byte[SpeedExeSize];
        Place(exe, ExePatchModel.GameSpeedQpcConstOffset, BitConverter.GetBytes(1_000_000_000.0 * multiplier));
        Place(exe, ExePatchModel.GameSpeedTgtConstOffset, BitConverter.GetBytes(1_000_000.0 * multiplier));
        return exe;
    }

    [Theory]
    [InlineData(1)]
    [InlineData(2)]
    [InlineData(3)]
    [InlineData(4)]
    [InlineData(5)]
    [InlineData(6)]
    [InlineData(7)]
    [InlineData(8)]
    [InlineData(9)]
    [InlineData(10)]
    public void Game_speed_multiplier_is_detected(int multiplier) {
        Assert.Equal(multiplier, ExePatchModel.GetGameSpeedMultiplier(NewSpeedExe(multiplier)));
    }

    [Fact]
    public void Game_speed_unknown_when_two_paths_disagree() {
        byte[] exe = new byte[SpeedExeSize];
        Place(exe, ExePatchModel.GameSpeedQpcConstOffset, BitConverter.GetBytes(2_000_000_000.0)); // 2×
        Place(exe, ExePatchModel.GameSpeedTgtConstOffset, BitConverter.GetBytes(3_000_000.0));     // 3×
        Assert.Equal(0, ExePatchModel.GetGameSpeedMultiplier(exe));
    }

    [Fact]
    public void Game_speed_unknown_when_not_integer_multiple() {
        byte[] exe = new byte[SpeedExeSize];
        Place(exe, ExePatchModel.GameSpeedQpcConstOffset, BitConverter.GetBytes(1_500_000_000.0));
        Place(exe, ExePatchModel.GameSpeedTgtConstOffset, BitConverter.GetBytes(1_500_000.0));
        Assert.Equal(0, ExePatchModel.GetGameSpeedMultiplier(exe));
    }

    [Fact]
    public void Game_speed_plan_is_empty_when_state_unknown() {
        Assert.Empty(ExePatchModel.PlanGameSpeed(3, 0));
    }

    [Fact]
    public void Game_speed_plan_is_empty_when_already_at_target() {
        Assert.Empty(ExePatchModel.PlanGameSpeed(3, 3));
    }

    [Theory]
    [InlineData(1, 3)] // 原版 → 3×
    [InlineData(2, 4)] // 2× → 4×
    [InlineData(4, 1)] // 4× → 還原
    [InlineData(1, 10)] // 原版 → 10×（最高倍率）
    [InlineData(10, 1)] // 10× → 還原
    public void Game_speed_apply_moves_between_multipliers(int from, int to) {
        byte[] exe = NewSpeedExe(from);
        IReadOnlyList<ExeWriteOp> ops = ExePatchModel.PlanGameSpeed(to, from);
        ExePatchModel.Apply(exe, ops);
        Assert.Equal(to, ExePatchModel.GetGameSpeedMultiplier(exe));
    }

    [Fact]
    public void Game_speed_apply_aborts_when_current_state_misdetected() {
        // 檔案實際是 3×，但計畫誤以為是原版(1×) → 預期位元組不符，必須中止且不破壞緩衝區。
        byte[] exe = NewSpeedExe(3);
        byte[] before = exe.ToArray();
        IReadOnlyList<ExeWriteOp> wrongPlan = ExePatchModel.PlanGameSpeed(2, 1);
        Assert.Throws<InvalidDataException>(() => ExePatchModel.Apply(exe, wrongPlan));
        Assert.Equal(before, exe);
    }
}
