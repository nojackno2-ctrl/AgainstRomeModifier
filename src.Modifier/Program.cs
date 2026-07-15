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

            string? gamePath = null;
            for (int i = 0; i < args.Length; i++) {
                if (args[i].Equals("--game", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length) {
                    gamePath = args[++i];
                }
            }

            // 啟動主啟動器介面
            Application.Run(new ModifierForm(gamePath));
        }
    }
}
