using System;
using System.IO;
using System.Text;
using System.Windows.Forms;

namespace AgainstRomeModifier {
    // 程式入口類別
    public class Program {
        [STAThread]
        public static void Main() {
            try {
                // 註冊 CodePages 支援（例如 BIG5, CP1251 等編碼，以便解析遊戲資源檔）
                Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
                // 啟動主要的應用程式流程
                RunApplication();
            } catch (Exception ex) {
                try {
                    // 若發生未預期的崩潰，將異常寫入 crash_log.txt 中以利後續分析
                    File.WriteAllText(Path.Combine(AppContext.BaseDirectory, "crash_log.txt"), ex.ToString());
                } catch (Exception writeEx) {
                    System.Diagnostics.Debug.WriteLine("日誌寫入失敗: " + writeEx.Message);
                }
            }
        }

        // 處理管理員權限提權以及表單的啟動
        private static void RunApplication() {
            var id = System.Security.Principal.WindowsIdentity.GetCurrent();
            var principal = new System.Security.Principal.WindowsPrincipal(id);
            // 檢查當前執行程序是否具有系統管理員權限（修改遊戲檔案需要管理員權限）
            bool isAdmin = principal.IsInRole(System.Security.Principal.WindowsBuiltInRole.Administrator);

            if (!isAdmin) {
                try {
                    // 記錄嘗試提權啟動的事件至日誌檔
                    File.AppendAllText(Path.Combine(AppContext.BaseDirectory, "modifier_log.txt"), string.Format("[{0}] 嘗試提權啟動...\r\n", DateTime.Now.ToString("HH:mm:ss")), Encoding.UTF8);
                } catch (Exception writeEx) {
                    System.Diagnostics.Debug.WriteLine("日誌寫入失敗: " + writeEx.Message);
                }

                // 設定以系統管理員權限 (runas 動作) 重新啟動當前執行檔
                var startInfo = new System.Diagnostics.ProcessStartInfo {
                    FileName = Environment.ProcessPath ?? System.Diagnostics.Process.GetCurrentProcess().MainModule?.FileName ?? "",
                    UseShellExecute = true,
                    Verb = "runas",
                    WorkingDirectory = AppContext.BaseDirectory
                };
                try {
                    System.Diagnostics.Process.Start(startInfo);
                } catch (Exception ex) {
                    try {
                        // 提權失敗時寫入日誌檔
                        File.AppendAllText(Path.Combine(AppContext.BaseDirectory, "modifier_log.txt"), string.Format("[{0}] 提權啟動失敗: {1}\r\n", DateTime.Now.ToString("HH:mm:ss"), ex.Message), Encoding.UTF8);
                    } catch (Exception writeEx) {
                        System.Diagnostics.Debug.WriteLine("日誌寫入失敗: " + writeEx.Message);
                    }
                }
                // 結束目前未提權的程式執行個體
                Application.Exit();
                return;
            }

            // 若已具備系統管理員權限，則啟動主表單視窗
            StartForm();
        }

        // 避免編譯器內聯此方法，確保權限檢查與 UI 啟動正確分離
        [System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
        private static void StartForm() {
            Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            // 啟動主修改器介面
            Application.Run(new ModifierForm());
        }
    }
}
