using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

namespace AgainstRomeModifier.Cli;

internal static class ConsoleHelper
{
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AttachConsole(int dwProcessId);
    private const int ATTACH_PARENT_PROCESS = -1;

    internal static void EnsureConsoleAttached()
    {
        if (!RuntimeInformation.IsOSPlatform(OSPlatform.Windows)) return;

        try
        {
            if (!Console.IsOutputRedirected)
            {
                if (AttachConsole(ATTACH_PARENT_PROCESS))
                {
                    var stdout = Console.OpenStandardOutput();
                    var stderr = Console.OpenStandardError();
                    Console.OutputEncoding = Encoding.UTF8;
                    Console.SetOut(new StreamWriter(stdout, Encoding.UTF8) { AutoFlush = true });
                    Console.SetError(new StreamWriter(stderr, Encoding.UTF8) { AutoFlush = true });
                }
            }
            else
            {
                Console.OutputEncoding = Encoding.UTF8;
            }
        }
        catch
        {
            // Ignore attachment failures
        }
    }
}
