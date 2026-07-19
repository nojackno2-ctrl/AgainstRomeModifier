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
