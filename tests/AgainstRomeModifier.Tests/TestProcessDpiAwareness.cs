using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// The test host starts DPI-unaware, but GLFW (used by the OpenGL view) switches the whole process to
/// per-monitor awareness the first time an OpenGL test runs. WinForms layout measurements (notably the
/// high-DPI emulation test, which was written for the unaware host) then depended on test order. Pin the
/// host's initial mode explicitly before any test runs; Windows rejects later process-wide changes.
/// </summary>
internal static class TestProcessDpiAwareness
{
    private static readonly IntPtr Unaware = new(-1);

    [ModuleInitializer]
    internal static void Initialize()
    {
        if (OperatingSystem.IsWindowsVersionAtLeast(10, 0, 15063)) SetProcessDpiAwarenessContext(Unaware);
    }

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetProcessDpiAwarenessContext(IntPtr value);
}
