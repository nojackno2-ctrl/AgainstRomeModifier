namespace AgainstRomeModifier
{
    public class BciLiteralPatch : IEndlessPatch
    {
        public string Id { get; }
        public string TargetPattern { get; }
        public int?[] Signature { get; }
        public int[] ValueWordIndices { get; }
        public int[] OriginalValues { get; }
        public int[] UltimateValues { get; }
        public int ExpectedSiteCount { get; }

        private readonly Func<byte[], int, PatchState>? _customDetectSite;

        public BciLiteralPatch(
            string id,
            string targetPattern,
            int?[] signature,
            int[] valueWordIndices,
            int[] originalValues,
            int[] ultimateValues,
            int expectedSiteCount,
            Func<byte[], int, PatchState>? customDetectSite = null)
        {
            Id = id;
            TargetPattern = targetPattern;
            Signature = signature;
            ValueWordIndices = valueWordIndices;
            OriginalValues = originalValues;
            UltimateValues = ultimateValues;
            ExpectedSiteCount = expectedSiteCount;
            _customDetectSite = customDetectSite;
        }

        public PatchState Detect(byte[] decompressed)
        {
            var sites = BciPattern.FindAllBciWordPatternSites(decompressed, Signature);
            if (sites.Count != ExpectedSiteCount)
            {
                return PatchState.Unknown;
            }

            bool allOriginal = true;
            bool allUltimate = true;

            for (int i = 0; i < sites.Count; i++)
            {
                int site = sites[i];
                if (_customDetectSite != null)
                {
                    var state = _customDetectSite(decompressed, site);
                    if (state == PatchState.Unknown) return PatchState.Legacy;
                    if (state == PatchState.Legacy)
                    {
                        allOriginal = false;
                        allUltimate = false;
                    }
                    else if (state == PatchState.Original)
                    {
                        allUltimate = false;
                    }
                    else if (state == PatchState.Ultimate)
                    {
                        allOriginal = false;
                    }
                }
                else
                {
                    for (int j = 0; j < ValueWordIndices.Length; j++)
                    {
                        int val = BitConverter.ToInt32(decompressed, site + ValueWordIndices[j] * 4);
                        if (val == UltimateValues[j])
                        {
                            allOriginal = false;
                        }
                        else if (val == OriginalValues[j])
                        {
                            allUltimate = false;
                        }
                        else
                        {
                            // Unrecognized value (e.g., from experimental builds);
                            // treat as Legacy so Apply can safely overwrite.
                            allOriginal = false;
                            allUltimate = false;
                        }
                    }
                }
            }

            if (allOriginal) return PatchState.Original;
            if (allUltimate) return PatchState.Ultimate;
            return PatchState.Legacy;
        }

        public bool Apply(ref byte[] decompressed, bool enabled)
        {
            var currentState = Detect(decompressed);
            if (currentState == PatchState.Unknown)
            {
                if (!enabled) return false;
                throw new InvalidOperationException($"Patch {Id} bytes do not match the original or supported patched pattern.");
            }
            var sites = BciPattern.FindAllBciWordPatternSites(decompressed, Signature);
            if (sites.Count != ExpectedSiteCount)
            {
                if (!enabled) return false;
                throw new InvalidOperationException($"Patch {Id} signature count mismatch. Expected {ExpectedSiteCount}, found {sites.Count}.");
            }

            bool changed = false;
            foreach (int site in sites)
            {
                for (int j = 0; j < ValueWordIndices.Length; j++)
                {
                    int targetVal = enabled ? UltimateValues[j] : OriginalValues[j];
                    int offset = site + ValueWordIndices[j] * 4;
                    int currentVal = BitConverter.ToInt32(decompressed, offset);
                    if (currentVal != targetVal)
                    {
                        BciPattern.WriteBciInt32(decompressed, offset, currentVal, targetVal, Id);
                        changed = true;
                    }
                }
            }
            return changed;
        }
    }
}
