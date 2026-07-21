using System;
using System.IO;
using System.Text;
using System.Windows.Forms;

namespace AgainstRomeModifier {
    // 程式入口類別
    public class Program {
        [STAThread]
        public static void Main(string[] args) {
            try {
                // 註冊 CodePages 支援（例如 BIG5, CP1251 等編碼，以便解析遊戲資源檔）
                Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
                // 啟動主要的應用程式流程
                RunApplication(args);
            } catch (Exception ex) {
                try {
                    // 若發生未預期的崩潰，將異常寫入 crash_log.txt 中以利後續分析
                    File.WriteAllText(Path.Combine(AppContext.BaseDirectory, "crash_log.txt"), ex.ToString());
                } catch (Exception writeEx) {
                    System.Diagnostics.Debug.WriteLine("日誌寫入失敗: " + writeEx.Message);
                }
                try {
                    // 不再無聲閃退：讓使用者知道發生了什麼、去哪裡找詳細記錄
                    MessageBox.Show(
                        "修改器發生未預期的錯誤，已中止執行。\n\n" + ex.Message +
                        "\n\n詳細記錄已寫入程式目錄下的 crash_log.txt。",
                        "Against Rome Modifier",
                        MessageBoxButtons.OK,
                        MessageBoxIcon.Error);
                } catch (Exception msgEx) {
                    System.Diagnostics.Debug.WriteLine("錯誤對話框顯示失敗: " + msgEx.Message);
                }
            }
        }

        // 啟動主表單。
        // 管理員權限由 app.manifest 的 requestedExecutionLevel=requireAdministrator 保證：
        // OS 會在行程啟動前強制 UAC，因此這裡不需要（也永遠不會走到）手動 runas 重啟邏輯。
        // 若 UAC 被使用者取消，程式根本不會啟動——這是 Windows 的標準行為。
        private static void RunApplication(string[] args) {
            Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            StartupOptions options = ParseStartupOptions(args);
            if (!string.IsNullOrWhiteSpace(options.MapId)) {
                if (string.IsNullOrWhiteSpace(options.GamePath))
                    throw new ArgumentException("使用 --map 直接開啟地圖時必須同時指定 --game <path>。");
                var selectedMap = new AgainstRomeModifier.Maps.GameMapCatalog().Require(options.GamePath, options.MapId);
                Application.Run(new AgainstRomeMapEditor.MapEditorForm(options.GamePath, selectedMap));
                return;
            }

            // 一般啟動仍進入統一啟動器；只有明確的 --game + --map 會直達地圖編輯器。
            Application.Run(new LauncherForm(options.GamePath));
        }

        public static StartupOptions ParseStartupOptions(string[] args) {
            string? gamePath = null;
            string? mapId = null;
            for (int i = 0; i < args.Length; i++) {
                if (args[i].Equals("--game", StringComparison.OrdinalIgnoreCase)) {
                    if (i + 1 >= args.Length) throw new ArgumentException("--game 缺少路徑參數。");
                    gamePath = args[++i];
                } else if (args[i].Equals("--map", StringComparison.OrdinalIgnoreCase)) {
                    if (i + 1 >= args.Length) throw new ArgumentException("--map 缺少地圖代號。");
                    mapId = args[++i];
                }
            }
            return new StartupOptions(gamePath, mapId);
        }

        public readonly record struct StartupOptions(string? GamePath, string? MapId);
    }
}
