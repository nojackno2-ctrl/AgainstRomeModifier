namespace AgainstRomeModifier.Core.Features;

public readonly struct FeatureValue
{
    private readonly bool _bool;
    private readonly int _int;
    private readonly object? _object;

    private FeatureValue(bool value) { _bool = value; _int = default; _object = null; }
    private FeatureValue(int value) { _bool = default; _int = value; _object = null; }
    private FeatureValue(object? value) { _bool = default; _int = default; _object = value; }

    public bool AsBool => _bool;
    public int AsInt => _int;
    public object? AsObject => _object;

    public static FeatureValue Of(bool value) => new(value);
    public static FeatureValue Of(int value) => new(value);
    public static FeatureValue Of(object? value) => new(value);
    public static FeatureValue Disabled(IFeatureModule module) => module.DisabledValue;
}
