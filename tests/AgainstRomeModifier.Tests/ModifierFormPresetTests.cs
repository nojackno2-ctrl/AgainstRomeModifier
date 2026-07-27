using System.Reflection;
using System.Windows.Forms;
using AgainstRomeModifier.Core.Features;

namespace AgainstRomeModifier.Tests;

public sealed class ModifierFormPresetTests
{
    [Fact]
    public void Enable_all_includes_runtime_verified_centering_and_entire_map_vision()
    {
        using var form = NewForm();

        Invoke(form, "BtnEnableAll_Click");

        Assert.True(GetToggle(form, "chkNativeWidescreen1920x1080").Checked);
        Assert.True(GetToggle(form, "chkAllUnitsEntireMapVision").Checked);
        Assert.False(GetToggle(form, "chkVillageGarrisonQuota3x").Checked);
    }

    [Fact]
    public void Enable_all_leaves_verified_opt_in_features_off()
    {
        using var form = NewForm();

        Invoke(form, "BtnEnableAll_Click");

        Assert.False(GetToggle(form, "chkGameSpeed").Checked);
        Assert.False(GetToggle(form, "chkCameraZoomOut1").Checked);
    }

    [Fact]
    public void Game_speed_profile_uses_selected_multiplier()
    {
        using var form = NewForm();
        var combo = GetField<ComboBox>(form, "cboGameSpeedMultiplier");

        // 預設選擇最高 10 倍；未勾選時輸出 1（原版速度）。
        Assert.Equal("×10", combo.SelectedItem?.ToString());
        Assert.Equal(1, BuildProfile(form).GameSpeed);

        GetToggle(form, "chkGameSpeed").Checked = true;
        Assert.Equal(10, BuildProfile(form).GameSpeed);

        combo.SelectedIndex = combo.Items.IndexOf("×4");
        Assert.Equal(4, BuildProfile(form).GameSpeed);

        Invoke(form, "BtnDisableAll_Click");
        Assert.False(GetToggle(form, "chkGameSpeed").Checked);
        Assert.Equal(1, BuildProfile(form).GameSpeed);
    }

    [Fact]
    public void Enable_all_covers_every_toggle_except_the_documented_exclusions()
    {
        using var form = NewForm();

        Invoke(form, "BtnEnableAll_Click");

        var toggles = GetFeatureToggles(form);
        var excluded = new HashSet<string>(
            ModifierForm.EnableAllExcludedFeatureIds, StringComparer.OrdinalIgnoreCase);

        // 排除清單本身必須全部是真實存在的開關，避免打錯字時默默失效。
        Assert.All(excluded, id => Assert.True(toggles.ContainsKey(id), $"未知的排除項: {id}"));
        foreach (var (id, toggle) in toggles)
        {
            Assert.Equal(!excluded.Contains(id), toggle.Checked);
        }
    }

    [Fact]
    public void Disable_all_resets_every_registered_toggle()
    {
        using var form = NewForm();

        Invoke(form, "BtnEnableAll_Click");
        // 一鍵全開不會碰排除清單，這裡手動打開它們，確保「全部關閉」真的涵蓋全部。
        var toggles = GetFeatureToggles(form);
        foreach (var toggle in toggles.Values) toggle.Checked = true;
        GetToggle(form, "chkVillageGarrisonQuota3x").Checked = true;
        GetToggle(form, "chkGameSpeed").Checked = true;

        Invoke(form, "BtnDisableAll_Click");

        Assert.All(toggles.Values, toggle => Assert.False(toggle.Checked));
        Assert.False(GetToggle(form, "chkVillageGarrisonQuota3x").Checked);
        Assert.False(GetToggle(form, "chkGameSpeed").Checked);
    }

    private static Dictionary<string, ModernToggle> GetFeatureToggles(ModifierForm form) =>
        GetField<Dictionary<string, ModernToggle>>(form, "featureToggles");

    private static ModifierForm NewForm()
    {
        string unavailableGamePath = Path.Combine(
            Path.GetTempPath(),
            "AgainstRomeModifierTests",
            Guid.NewGuid().ToString("N"));
        return new ModifierForm(unavailableGamePath);
    }

    private static PatchProfile BuildProfile(ModifierForm form)
    {
        MethodInfo method = typeof(ModifierForm).GetMethod(
            "BuildCurrentPatchProfile",
            BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException("找不到 BuildCurrentPatchProfile。");
        return Assert.IsType<PatchProfile>(method.Invoke(form, [false]));
    }

    private static void Invoke(ModifierForm form, string methodName)
    {
        MethodInfo method = typeof(ModifierForm).GetMethod(
            methodName,
            BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException($"找不到 {methodName}。");
        method.Invoke(form, [null, EventArgs.Empty]);
    }

    private static ModernToggle GetToggle(ModifierForm form, string fieldName) =>
        GetField<ModernToggle>(form, fieldName);

    private static T GetField<T>(ModifierForm form, string fieldName)
    {
        FieldInfo field = typeof(ModifierForm).GetField(
            fieldName,
            BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException($"找不到 {fieldName}。");
        return Assert.IsType<T>(field.GetValue(form));
    }
}
