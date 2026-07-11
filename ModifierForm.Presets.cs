using System;
using System.IO;
using System.Text;
using System.Linq;
using System.Drawing;
using System.Globalization;
using System.Windows.Forms;
using System.Collections.Generic;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        /// <summary>
        /// 當使用者點擊「匯出設定」時觸發，將當前 UI 上的數值與勾選狀態序列化為設定檔 (.arpreset)，並一併寫入自訂兵種屬性。
        /// </summary>
        private void BtnPresetSave_Click(object? sender, EventArgs e) {
            using (SaveFileDialog sfd = new SaveFileDialog {
                Filter = "修改器設定檔 (*.arpreset)|*.arpreset",
                DefaultExt = "arpreset",
                FileName = "modifier_preset"
            }) {
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
                    sb.AppendLine(string.Format("AiUltimateMode={0}", chkAiUltimateMode.Checked));
                    sb.AppendLine(string.Format("VillageBuildRange={0}", chkVillageBuildRange.Checked));

                    // 如果當前有自訂兵種屬性設定，也一併寫入
                    if (customUnitStats != null && customUnitStats.Count > 0) {
                        sb.AppendLine();
                        sb.AppendLine("[TroopStats]");
                        foreach (var kvp in customUnitStats) {
                            string valStr = string.Join(",", kvp.Value.Select(x => x.ToString(CultureInfo.InvariantCulture)));
                            sb.AppendLine(string.Format(CultureInfo.InvariantCulture, "{0}={1}", kvp.Key, valStr));
                        }
                    }

                    File.WriteAllText(sfd.FileName, sb.ToString(), Encoding.UTF8);
                    Log("已匯出設定至: " + sfd.FileName);
                } catch (Exception ex) {
                    Log("匯出設定失敗: " + ex.Message + "\r\n" + ex.StackTrace);
                }
            }
        }

        /// <summary>
        /// 當使用者點擊「匯入設定」時觸發，載入設定檔 (.arpreset) 以套用全域數值與自訂兵種屬性。
        /// </summary>
        private void BtnPresetLoad_Click(object? sender, EventArgs e) {
            using (OpenFileDialog ofd = new OpenFileDialog {
                Filter = "修改器設定檔 (*.arpreset)|*.arpreset"
            }) {
                if (ofd.ShowDialog() != DialogResult.OK) return;
                try {
                    string[] lines = File.ReadAllLines(ofd.FileName, Encoding.UTF8);
                    string currentSection = "";
                    var tempCustomStats = new Dictionary<string, double[]>(StringComparer.OrdinalIgnoreCase);

                    foreach (string line in lines) {
                        string l = line.Trim();
                        if (l.StartsWith("[") && l.EndsWith("]")) {
                            currentSection = l.Substring(1, l.Length - 2).Trim();
                            continue;
                        }
                        if (!l.Contains("=")) continue;
                        string[] kv = l.Split(new char[] { '=' }, 2);
                        string k = kv[0].Trim();
                        string v = kv[1].Trim();

                        try {
                            if (currentSection.Equals("Settings", StringComparison.OrdinalIgnoreCase) || string.IsNullOrEmpty(currentSection)) {
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
                                else if (k == "AiUltimateMode") chkAiUltimateMode.Checked = bool.Parse(v);
                                else if (k == "VillageBuildRange") chkVillageBuildRange.Checked = bool.Parse(v);
                            } else if (currentSection.Equals("TroopStats", StringComparison.OrdinalIgnoreCase)) {
                                string[] vals = v.Split(',');
                                if (vals.Length >= 4) {
                                    double[] stats = new double[vals.Length];
                                    for (int i = 0; i < vals.Length; i++) {
                                        stats[i] = double.Parse(vals[i].Trim(), CultureInfo.InvariantCulture);
                                    }
                                    tempCustomStats[k] = stats;
                                }
                            }
                        } catch (Exception parseEx) {
                            Log(string.Format("解析設定欄位 {0}={1} 失敗: {2}", k, v, parseEx.Message));
                        }
                    }

                    // 如果讀取到自訂兵種屬性，則更新並重載預設屬性表格
                    if (tempCustomStats.Count > 0) {
                        customUnitStats = tempCustomStats;
                        presetFileSourceType = "preset";
                        presetFileName = Path.GetFileName(ofd.FileName);
                    } else {
                        customUnitStats = null;
                        presetFileSourceType = "default";
                        presetFileName = "";
                    }
                    UpdateTroopPresetLabel();

                    LoadDefaultStatsData();

                    Log("已匯入設定自: " + ofd.FileName);
                } catch (Exception ex) {
                    Log("匯入設定失敗: " + ex.Message + "\r\n" + ex.StackTrace);
                }
            }
        }
    }
}
