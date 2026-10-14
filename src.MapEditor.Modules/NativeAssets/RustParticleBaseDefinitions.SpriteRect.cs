using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor.NativeAssets;

internal sealed record NativeParticleSpriteRect(int Left, int Top, int Right, int Bottom);

internal sealed partial class RustParticleBaseDefinitions
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate int SpriteRectFn(uint anchorWidth, uint anchorHeight, int offsetX, int offsetY,
        int width, int height, nuint pixels, [Out] int[] edges);
    private SpriteRectFn? _spriteRect;

    internal bool TryResolveSpriteRect(NativeParticleAssetFrame frame,
        out NativeParticleSpriteRect? rect, out string? diagnostic)
    {
        ArgumentNullException.ThrowIfNull(frame);
        rect = null; diagnostic = null;
        lock (_gate)
        {
            ObjectDisposedException.ThrowIf(_handle.IsClosed, this);
            _spriteRect ??= Marshal.GetDelegateForFunctionPointer<SpriteRectFn>(NativeLibrary.GetExport(_library,
                "ar_particle_sprite_rect"));
            int[] edges = new int[4];
            if (frame.Image.Width != frame.Info.Width || frame.Image.Height != frame.Info.Height)
            { diagnostic = "Particle decoded dimensions do not match ALR frame metadata."; return false; }
            int code = _spriteRect(frame.AnchorWidth, frame.AnchorHeight, frame.Info.OffsetX, frame.Info.OffsetY,
                frame.Info.Width, frame.Info.Height, (nuint)frame.Image.ArgbPixels.Count, edges);
            if (code == 0) { rect = new(edges[0], edges[1], edges[2], edges[3]); return true; }
            if (code != 253) throw new InvalidDataException($"Rust particle geometry rejected (code={code}).");
            diagnostic = "Particle sprite dimensions, pixel count or anchor-relative bounds are invalid.";
            return false;
        }
    }
}
