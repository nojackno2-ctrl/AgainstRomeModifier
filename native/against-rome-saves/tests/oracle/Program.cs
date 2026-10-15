using AgainstRomeModifier;

// Test-only oracle, compiling the actual unchanged C# patch implementations.
// Input: one decoded BCI file per case; output: patched BCI and compressed PFIL.
foreach (string path in Directory.GetFiles(args[0], "*.input"))
{
    byte[] bytes = File.ReadAllBytes(path);
    IEndlessPatch[] patches = { new P6_LoopDelayPatch(), new P8_ReinforcementUnitThresholdPatch(), new P9_RetreatQuotaPatch() };
    foreach (IEndlessPatch patch in patches)
        if (patch.Detect(bytes) == PatchState.Unknown) throw new InvalidDataException(patch.Id);
    foreach (IEndlessPatch patch in patches) patch.Apply(ref bytes, true);
    File.WriteAllBytes(path + ".output", bytes);
    byte[] header = new byte[64];
    "PFIL"u8.CopyTo(header);
    File.WriteAllBytes(path + ".pfil", GameLZSS.CompressPfil(bytes, header));
}
