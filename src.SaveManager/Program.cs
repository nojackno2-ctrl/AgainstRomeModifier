using System;
using System.Windows.Forms;

namespace AgainstRomeModifier {
    internal static class Program {
        [STAThread]
        static void Main(string[] args) {
            Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            string? gamePath = null;
            for (int i = 0; i < args.Length; i++) {
                if (args[i].Equals("--game", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length) {
                    gamePath = args[++i];
                }
            }

            Application.Run(new SaveManagerForm(gamePath));
        }
    }
}
