namespace AgainstRomeModifier.Core.Features;

public sealed class DetectContext
{
    public DetectContext(PatchProfile detectedProfile) => DetectedProfile = detectedProfile;
    public PatchProfile DetectedProfile { get; }
}
