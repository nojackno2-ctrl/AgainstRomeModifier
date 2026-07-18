using System;

namespace AgainstRomeModifier.Core.Services {
    /// <summary>
    /// 從系統登錄檔自動偵測《Against Rome》的安裝路徑。
    /// 供 Modifier / Launcher / SaveManager 共用，避免各表單維護重複邏輯。
    /// </summary>
    public static class GameDirectoryLocator {
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
