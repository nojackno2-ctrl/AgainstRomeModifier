using System.Drawing.Imaging;
using System.Runtime.InteropServices;

namespace AgainstRomeMapEditor;

/// <summary>以單次 LockBits 讀寫整張點陣圖，取代逐像素 GetPixel/SetPixel（快一個數量級）。</summary>
internal static class BitmapPixels
{
    // 回傳的 int 為原生小端序 ARGB：A=(px>>24)&0xff、R=(px>>16)&0xff、G=(px>>8)&0xff、B=px&0xff。
    public static int[] Read(Bitmap bitmap)
    {
        var rectangle = new Rectangle(0, 0, bitmap.Width, bitmap.Height);
        BitmapData data = bitmap.LockBits(rectangle, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        try
        {
            var pixels = new int[bitmap.Width * bitmap.Height];
            if (data.Stride == bitmap.Width * 4)
                Marshal.Copy(data.Scan0, pixels, 0, pixels.Length);
            else
                for (int y = 0; y < bitmap.Height; y++)
                    Marshal.Copy(IntPtr.Add(data.Scan0, y * data.Stride), pixels, y * bitmap.Width, bitmap.Width);
            return pixels;
        }
        finally { bitmap.UnlockBits(data); }
    }

    public static Bitmap Write(int width, int height, int[] argb)
    {
        var bitmap = new Bitmap(width, height, PixelFormat.Format32bppArgb);
        var rectangle = new Rectangle(0, 0, width, height);
        BitmapData data = bitmap.LockBits(rectangle, ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
        try
        {
            if (data.Stride == width * 4)
                Marshal.Copy(argb, 0, data.Scan0, argb.Length);
            else
                for (int y = 0; y < height; y++)
                    Marshal.Copy(argb, y * width, IntPtr.Add(data.Scan0, y * data.Stride), width);
        }
        finally { bitmap.UnlockBits(data); }
        return bitmap;
    }
}
