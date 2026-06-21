using System;
using System.IO;
using System.Text;
using System.Globalization;
using System.Windows.Forms;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        /// <summary>
        /// 當使用者點擊「匯出設定」時觸發，將當前 UI 上的數值與勾選狀態序列化為設定檔 (.arpreset)。
        /// </summary>
        private void BtnPresetSave_Click(object? sender, EventArgs e) {
            SaveFileDialog sfd = new SaveFileDialog {
                Filter = "修改器設定檔 (*.arpreset)|*.arpreset",
                DefaultExt = "arpreset",
                FileName = "modifier_preset"
            };
            if (sfd.ShowDialog() != DialogResult.OK) return;
            try {
                var sb = new StringBuilder();
                sb.AppendLine("[Settings]");
                sb.AppendLine("Version=1");
                sb.AppendLine(string.Format("PopLimit={0}", numPopLimit.Value));

                sb.AppendLine(string.Format("CiviSpeed={0}", numCiviSpeed.Value.ToString(CultureInfo.InvariantCulture)));
                sb.AppendLine(string.Format("FreeProd={0}", chkFreeProd.Checked));
                sb.AppendLine(string.Format("FreeUpgrade={0}", chkFreeUpgrade.Checked));
                sb.AppendLine(string.Format("NoSpellCost={0}", chkNoSpellCost.Checked));
                sb.AppendLine(string.Format("FocusLoss={0}", chkFocusLoss.Checked));
                sb.AppendLine(string.Format("Balance={0}", chkBalance.Checked));
                sb.AppendLine(string.Format("ToEng={0}", chkToEng.Checked));
                sb.AppendLine(string.Format("InfiniteMorale={0}", chkInfiniteMorale.Checked));

                File.WriteAllText(sfd.FileName, sb.ToString(), Encoding.UTF8);
                Log("已匯出設定至: " + sfd.FileName);
            } catch (Exception ex) {
                Log("匯出設定失敗: " + ex.Message + "\r\n" + ex.StackTrace);
            }
        }

        /// <summary>
        /// 當使用者點擊「匯入設定」時觸發，載入並反序列化設定檔 (.arpreset) 以套用數值至 UI 介面。
        /// </summary>
        private void BtnPresetLoad_Click(object? sender, EventArgs e) {
            OpenFileDialog ofd = new OpenFileDialog {
                Filter = "修改器設定檔 (*.arpreset)|*.arpreset"
            };
            if (ofd.ShowDialog() != DialogResult.OK) return;
            try {
                string[] lines = File.ReadAllLines(ofd.FileName, Encoding.UTF8);

                foreach (string line in lines) {
                    if (line.Trim().StartsWith("[") && line.Trim().EndsWith("]")) continue;
                    if (!line.Contains("=")) continue;
                    string[] kv = line.Split(new char[] { '=' }, 2);
                    string k = kv[0].Trim();
                    string v = kv[1].Trim();

                    try {
                        if (k == "Version") continue;
                        else if (k == "PopLimit") { var lv = decimal.Parse(v, CultureInfo.InvariantCulture); numPopLimit.Value = lv < numPopLimit.Minimum ? numPopLimit.Minimum : (lv > numPopLimit.Maximum ? numPopLimit.Maximum : lv); }

                        else if (k == "CiviSpeed") { var cv = decimal.Parse(v, CultureInfo.InvariantCulture); numCiviSpeed.Value = cv < numCiviSpeed.Minimum ? numCiviSpeed.Minimum : (cv > numCiviSpeed.Maximum ? numCiviSpeed.Maximum : cv); }
                        else if (k == "FreeProd") chkFreeProd.Checked = bool.Parse(v);
                        else if (k == "FreeUpgrade") chkFreeUpgrade.Checked = bool.Parse(v);
                        else if (k == "NoSpellCost") chkNoSpellCost.Checked = bool.Parse(v);
                        else if (k == "FocusLoss") chkFocusLoss.Checked = bool.Parse(v);
                        else if (k == "Balance") chkBalance.Checked = bool.Parse(v);
                        else if (k == "ToEng") chkToEng.Checked = bool.Parse(v);
                        else if (k == "InfiniteMorale") chkInfiniteMorale.Checked = bool.Parse(v);
                    } catch (Exception parseEx) {
                        Log(string.Format("解析設定欄位 {0}={1} 失敗: {2}", k, v, parseEx.Message));
                    }
                }

                Log("已匯入設定自: " + ofd.FileName);
            } catch (Exception ex) {
                Log("匯入設定失敗: " + ex.Message + "\r\n" + ex.StackTrace);
            }
        }
    }
}
