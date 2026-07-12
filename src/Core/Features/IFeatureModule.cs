namespace AgainstRomeModifier.Core.Features;

public interface IFeatureModule
{
    string Id { get; }
    FeatureCategory Category { get; }
    FeatureValue DisabledValue { get; }
    void Plan(PatchContext context, FeatureValue value);
    FeatureValue Detect(DetectContext context);
}
