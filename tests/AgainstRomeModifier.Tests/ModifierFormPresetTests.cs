using System.Reflection;

namespace AgainstRomeModifier.Tests;

public sealed class ModifierFormPresetTests
{
    [Fact]
    public void Enable_all_includes_runtime_verified_centering_and_entire_map_vision()
    {
        string unavailableGamePath = Path.Combine(
            Path.GetTempPath(),
            "AgainstRomeModifierTests",
            Guid.NewGuid().ToString("N"));
        using var form = new ModifierForm(unavailableGamePath);

        Invoke(form, "BtnEnableAll_Click");

        Assert.True(GetToggle(form, "chkNativeWidescreen1920x1080").Checked);
        Assert.True(GetToggle(form, "chkAllUnitsEntireMapVision").Checked);
        Assert.False(GetToggle(form, "chkVillageGarrisonQuota3x").Checked);
    }

    private static void Invoke(ModifierForm form, string methodName)
    {
        MethodInfo method = typeof(ModifierForm).GetMethod(
            methodName,
            BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException($"找不到 {methodName}。");
        method.Invoke(form, [null, EventArgs.Empty]);
    }

    private static ModernToggle GetToggle(ModifierForm form, string fieldName)
    {
        FieldInfo field = typeof(ModifierForm).GetField(
            fieldName,
            BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException($"找不到 {fieldName}。");
        return Assert.IsType<ModernToggle>(field.GetValue(form));
    }
}
