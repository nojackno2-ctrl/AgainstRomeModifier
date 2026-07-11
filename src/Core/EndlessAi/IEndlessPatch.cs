namespace AgainstRomeModifier
{
    public interface IEndlessPatch
    {
        string Id { get; }
        string TargetPattern { get; }
        PatchState Detect(byte[] decompressed);
        bool Apply(ref byte[] decompressed, bool enabled);
    }
}
