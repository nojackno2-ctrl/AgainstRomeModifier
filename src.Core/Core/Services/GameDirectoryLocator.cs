using System;

namespace AgainstRomeModifier.Core.Services {
    /// <summary>
    /// 從系統登錄檔自動偵測《Against Rome》的安裝路徑。
    /// 供 Modifier / Launcher / SaveManager 共用，避免各表單維護重複邏輯。
    /// </summary>
    public static class GameDirectoryLocator {
        /// <summary>《Against Rome》的預設安裝路徑（找不到其他來源時的最後退路）。</summary>
        public const string DefaultInstallPath = @"C:\Program Files (x86)\Against Rome";

        /// <summary>
        /// 依統一順序解析初始遊戲路徑：明確指定的有效目錄 → 與修改器同目錄的可攜安裝
        /// → 登錄檔偵測 → 預設安裝路徑；全部落空時回傳空字串。
        /// 供 Modifier / SaveManager 共用，取代各自重複的 fallback。
        /// </summary>
        public static string ResolveInitialGamePath(string? preferred) {
            if (!string.IsNullOrWhiteSpace(preferred) && System.IO.Directory.Exists(preferred))
                return preferred;
            if (System.IO.File.Exists(System.IO.Path.Combine(AppContext.BaseDirectory, "Against_Rome.exe")))
                return AppContext.BaseDirectory;
            string detected = DetectFromRegistry();
            if (!string.IsNullOrEmpty(detected))
                return detected;
            if (System.IO.Directory.Exists(DefaultInstallPath))
                return DefaultInstallPath;
            return "";
        }

        /// <summary>依 HKCU → HKLM → HKLM\WOW6432Node 的順序查詢 "Against Rome" 的 Path 值；查無或失敗時回傳空字串。</summary>
        public static string DetectFromRegistry() {
            string[] hkcuPaths = { @"Software\Against Rome" };
            string[] hklmPaths = { @"SOFTWARE\Against Rome", @"SOFTWARE\WOW6432Node\Against Rome" };
            try {
                foreach (string subKey in hkcuPaths) {
                    string? path = ReadPath(Microsoft.Win32.Registry.CurrentUser, subKey);
                    if (path != null) return path;
                }
                foreach (string subKey in hklmPaths) {
                    string? path = ReadPath(Microsoft.Win32.Registry.LocalMachine, subKey);
                    if (path != null) return path;
                }
            } catch (Exception ex) {
                System.Diagnostics.Debug.WriteLine("讀取登錄檔遊戲路徑失敗: " + ex.Message);
            }
            return "";
        }

        private static string? ReadPath(Microsoft.Win32.RegistryKey root, string subKey) {
            using (var key = root.OpenSubKey(subKey)) {
                return key?.GetValue("Path")?.ToString();
            }
        }
    }
}
