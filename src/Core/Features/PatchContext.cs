namespace AgainstRomeModifier.Core.Features;

/// <summary>
/// Logical planning context. Per-file composers consume the resulting profile once,
/// preserving the existing single rewrite per shared game file.
/// </summary>
public sealed class PatchContext
{
    public PatchProfile Profile { get; } = new();
    public void Set(string id, FeatureValue value) => Profile.Set(id, value);
}
