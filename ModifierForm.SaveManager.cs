using System;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;
using System.Collections.Generic;
using System.Globalization;
using System.Windows.Forms;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        /// <summary>
        /// 建立並設定用於顯示遊戲存檔或備份存檔列表的 DataGridView 表格。
        /// </summary>
        private DataGridView CreateSaveGrid(bool isBackup) {
            var dgv = new DataGridView {
                AllowUserToAddRows = false,
                AllowUserToDeleteRows = false,
                RowHeadersVisible = false,
                BackgroundColor = Color.FromArgb(20, 20, 25),
                ForeColor = Color.FromArgb(230, 235, 240),
                GridColor = Color.FromArgb(45, 45, 55),
                BorderStyle = BorderStyle.None,
                EnableHeadersVisualStyles = false,
                RowTemplate = { Height = 35 },
                SelectionMode = DataGridViewSelectionMode.FullRowSelect,
                MultiSelect = false,
                ReadOnly = true
            };

            dgv.ColumnHeadersDefaultCellStyle.BackColor = Color.FromArgb(32, 32, 40);
            dgv.ColumnHeadersDefaultCellStyle.ForeColor = Color.FromArgb(0, 220, 255);
            dgv.ColumnHeadersDefaultCellStyle.SelectionBackColor = Color.FromArgb(32, 32, 40);
            dgv.ColumnHeadersDefaultCellStyle.Font = fontJhengHei95B;
            dgv.ColumnHeadersHeight = 35;
            dgv.ColumnHeadersBorderStyle = DataGridViewHeaderBorderStyle.Single;

            dgv.DefaultCellStyle.BackColor = Color.FromArgb(24, 24, 30);
            dgv.DefaultCellStyle.ForeColor = Color.FromArgb(230, 235, 240);
            dgv.DefaultCellStyle.SelectionBackColor = Color.FromArgb(45, 45, 60);
            dgv.DefaultCellStyle.SelectionForeColor = Color.White;
            dgv.DefaultCellStyle.Font = fontJhengHei9R;

            dgv.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(28, 28, 35);

            if (!isBackup) {
                dgv.Columns.Add("Folder", "資料夾");
                dgv.Columns["Folder"].Width = 120;
                dgv.Columns.Add("Title", "存檔標題");
                dgv.Columns["Title"].Width = 330;
                dgv.Columns.Add("Level", "原版關卡");
                dgv.Columns["Level"].Width = 140;
                dgv.Columns.Add("Time", "存檔時間");
                dgv.Columns["Time"].Width = 180;
            } else {
                dgv.Columns.Add("File", "備份檔名");
                dgv.Columns["File"].Width = 120;
                dgv.Columns.Add("Title", "存檔標題");
                dgv.Columns["Title"].Width = 230;
                dgv.Columns.Add("Level", "原版關卡");
                dgv.Columns["Level"].Width = 120;
                dgv.Columns.Add("Time", "備份時間");
                dgv.Columns["Time"].Width = 160;
                dgv.Columns.Add("Folder", "原資料夾");
                dgv.Columns["Folder"].Width = 120;
            }

            return dgv;
        }

        /// <summary>
        /// 重新整理並讀取遊戲存檔目錄 (SAVE) 與備份目錄 (SavesBackup) 下的資料，並將結果載入到介面表格中。
        /// </summary>
        private void RefreshSavesAndBackups() {
            try {
                dgvGameSaves.Rows.Clear();
                dgvBackups.Rows.Clear();
                if (picSavePreview.Image != null) {
                    picSavePreview.Image.Dispose();
                    picSavePreview.Image = null;
                }
                lblSaveDetail.Text = "";

                string gamePath = GetGamePath();
                if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                    Log("遊戲路徑未設定，無法載入存檔。");
                    return;
                }

                string saveDir = Path.Combine(gamePath, "SAVE");
                if (Directory.Exists(saveDir)) {
                    var dirs = Directory.GetDirectories(saveDir);
                    foreach (var dir in dirs) {
                        string folderName = Path.GetFileName(dir);
                        string saveIni = Path.Combine(dir, "save.ini");
                        if (File.Exists(saveIni)) {
                            string title = "";
                            string level = "";
                            try {
                                byte[] raw = File.ReadAllBytes(saveIni);
                                byte[] decomp = GameLZSS.DecompressPfil(raw);
                                string text = Encoding.GetEncoding(1251).GetString(decomp);
                                var lines = text.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries);
                                for (int i = 0; i < lines.Length; i++) {
                                    string line = lines[i].Trim();
                                    if (line.Contains("[orglevelname]")) {
                                        if (i + 1 < lines.Length) level = lines[i + 1].Trim();
                                    }
                                    if (line.Contains("[titel]")) {
                                        if (i + 1 < lines.Length) title = lines[i + 1].Trim();
                                    }
                                }
                            } catch {
                                title = "無法解析";
                                level = "未知";
                            }
                            DateTime writeTime = Directory.GetLastWriteTime(dir);
                            dgvGameSaves.Rows.Add(folderName, title, level, writeTime.ToString("yyyy-MM-dd HH:mm:ss"));
                        }
                    }
                }

                string backupDir = Path.Combine(AppContext.BaseDirectory, "SavesBackup");
                if (!Directory.Exists(backupDir)) {
                    Directory.CreateDirectory(backupDir);
                }

                var files = Directory.GetFiles(backupDir, "*.zip");
                foreach (var file in files) {
                    string fileName = Path.GetFileName(file);
                    DateTime lastWrite = File.GetLastWriteTime(file);

                    BackupSaveCache? cache = null;
                    if (_backupSaveCache.TryGetValue(fileName, out var existingCache) && existingCache.LastWriteTime == lastWrite) {
                        cache = existingCache;
                    } else {
                        string title = "";
                        string level = "";
                        string origFolder = "";
                        string backupTimeStr = "";
                        try {
                            using (var archive = ZipFile.OpenRead(file)) {
                                var entry = archive.GetEntry("save.ini");
                                if (entry != null) {
                                    using (var stream = entry.Open()) {
                                        using (var ms = new MemoryStream()) {
                                            stream.CopyTo(ms);
                                            byte[] decomp = GameLZSS.DecompressPfil(ms.ToArray());
                                            string text = Encoding.GetEncoding(1251).GetString(decomp);
                                            var lines = text.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries);
                                            for (int i = 0; i < lines.Length; i++) {
                                                string line = lines[i].Trim();
                                                if (line.Contains("[orglevelname]")) {
                                                    if (i + 1 < lines.Length) level = lines[i + 1].Trim();
                                                }
                                                if (line.Contains("[titel]")) {
                                                    if (i + 1 < lines.Length) title = lines[i + 1].Trim();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } catch {
                            title = "無法解析";
                            level = "未知";
                        }

                        var parts = fileName.Split('_');
                        if (parts.Length >= 4 && parts[0] == "Backup") {
                            origFolder = parts[1];
                            string dateStr = parts[2];
                            string timeStr = parts[3].Replace(".zip", "");
                            if (dateStr.Length == 8 && timeStr.Length == 6) {
                                backupTimeStr = string.Format("{0}-{1}-{2} {3}:{4}:{5}", dateStr.Substring(0, 4), dateStr.Substring(4, 2), dateStr.Substring(6, 2), timeStr.Substring(0, 2), timeStr.Substring(2, 2), timeStr.Substring(4, 2));
                            }
                        }
                        if (string.IsNullOrEmpty(backupTimeStr)) {
                            backupTimeStr = File.GetCreationTime(file).ToString("yyyy-MM-dd HH:mm:ss");
                        }
                        if (string.IsNullOrEmpty(origFolder)) {
                            origFolder = "未知";
                        }

                        cache = new BackupSaveCache {
                            FileName = fileName,
                            Title = title,
                            Level = level,
                            OrigFolder = origFolder,
                            BackupTimeStr = backupTimeStr,
                            LastWriteTime = lastWrite
                        };
                        _backupSaveCache[fileName] = cache;
                    }

                    dgvBackups.Rows.Add(cache.FileName, cache.Title, cache.Level, cache.BackupTimeStr, cache.OrigFolder);
                }
            } catch (Exception ex) {
                Log("重新整理存檔列表失敗: " + ex.Message);
            }
        }

        /// <summary>
        /// 當選擇的遊戲存檔發生改變時觸發，載入並顯示該存檔的詳細文字資訊與 TGA 圖像預覽。
        /// </summary>
        private void DgvGameSaves_SelectionChanged(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) return;
            try {
                var row = dgvGameSaves.SelectedRows[0];
                string folder = row.Cells[0].Value?.ToString() ?? "";
                string title = row.Cells[1].Value?.ToString() ?? "";
                string level = row.Cells[2].Value?.ToString() ?? "";
                string time = row.Cells[3].Value?.ToString() ?? "";

                lblSaveDetail.Text = string.Format(
                    "存檔類型: 遊戲存檔\n\n資料夾: {0}\n\n存檔標題: {1}\n\n原版關卡: {2}\n\n存檔時間: {3}",
                    folder, title, level, time
                );

                string gamePath = GetGamePath();
                string picPath = Path.Combine(gamePath, "SAVE", folder, "savepic.tga");
                if (File.Exists(picPath)) {
                    byte[] bytes = File.ReadAllBytes(picPath);
                    if (picSavePreview.Image != null) picSavePreview.Image.Dispose();
                    picSavePreview.Image = LoadTga(bytes);
                } else {
                    if (picSavePreview.Image != null) picSavePreview.Image.Dispose();
                    picSavePreview.Image = null;
                }
            } catch (Exception ex) {
                Log("載入存檔預覽失敗: " + ex.Message);
            }
        }

        /// <summary>
        /// 當選擇的備份檔案發生改變時觸發，自 zip 壓縮包中讀取 savepic.tga 圖示預覽，並顯示詳細備份資訊。
        /// </summary>
        private void DgvBackups_SelectionChanged(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) return;
            try {
                var row = dgvBackups.SelectedRows[0];
                string file = row.Cells[0].Value?.ToString() ?? "";
                string title = row.Cells[1].Value?.ToString() ?? "";
                string level = row.Cells[2].Value?.ToString() ?? "";
                string time = row.Cells[3].Value?.ToString() ?? "";
                string origFolder = row.Cells[4].Value?.ToString() ?? "";

                lblSaveDetail.Text = string.Format(
                    "存檔類型: 備份檔案\n\n備份檔名: {0}\n\n原資料夾: {1}\n\n存檔標題: {2}\n\n原版關卡: {3}\n\n備份時間: {4}",
                    file, origFolder, title, level, time
                );

                string backupDir = Path.Combine(AppContext.BaseDirectory, "SavesBackup");
                string zipPath = Path.Combine(backupDir, file);
                if (File.Exists(zipPath)) {
                    using (var archive = ZipFile.OpenRead(zipPath)) {
                        var entry = archive.GetEntry("savepic.tga");
                        if (entry != null) {
                            using (var stream = entry.Open()) {
                                using (var ms = new MemoryStream()) {
                                    stream.CopyTo(ms);
                                    if (picSavePreview.Image != null) picSavePreview.Image.Dispose();
                                    picSavePreview.Image = LoadTga(ms.ToArray());
                                }
                            }
                        } else {
                            if (picSavePreview.Image != null) picSavePreview.Image.Dispose();
                            picSavePreview.Image = null;
                        }
                    }
                } else {
                    if (picSavePreview.Image != null) picSavePreview.Image.Dispose();
                    picSavePreview.Image = null;
                }
            } catch (Exception ex) {
                Log("載入備份預覽失敗: " + ex.Message);
            }
        }

        /// <summary>
        /// 當使用者點擊「備份此存檔」時觸發，將選取的存檔資料夾壓縮備份至 SavesBackup 目錄。
        /// </summary>
        private void BtnBackupSave_Click(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) {
                MessageBox.Show("請先選擇要備份的存檔。", "提示", MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                var row = dgvGameSaves.SelectedRows[0];
                string folder = (row.Cells[0].Value?.ToString() ?? "").Trim();
                if (string.IsNullOrEmpty(folder)) {
                    MessageBox.Show("無效的存檔目錄，操作已取消。", "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                string title = row.Cells[1].Value?.ToString() ?? "";
                string gamePath = GetGamePath();
                string srcDir = Path.Combine(gamePath, "SAVE", folder);
                if (!Directory.Exists(srcDir)) {
                    MessageBox.Show("找不到該存檔的原始資料夾。", "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                string backupDir = Path.Combine(AppContext.BaseDirectory, "SavesBackup");
                if (!Directory.Exists(backupDir)) {
                    Directory.CreateDirectory(backupDir);
                }
                string timeStr = DateTime.Now.ToString("yyyyMMdd_HHmmss");
                string zipName = string.Format("Backup_{0}_{1}.zip", folder, timeStr);
                string zipPath = Path.Combine(backupDir, zipName);

                ZipFile.CreateFromDirectory(srcDir, zipPath);
                Log(string.Format("備份存檔成功: {0} -> {1}", folder, zipName));
                RefreshSavesAndBackups();
                MessageBox.Show("備份存檔成功！", "成功", MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log("備份存檔失敗: " + ex.Message);
                MessageBox.Show("備份存檔失敗: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        /// <summary>
        /// 當使用者點擊「還原此備份」時觸發，解壓所選的備份檔案，並覆蓋回原存檔目錄。
        /// </summary>
        private void BtnRestoreBackup_Click(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) {
                MessageBox.Show("請先選擇要還原的備份。", "提示", MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                var row = dgvBackups.SelectedRows[0];
                string file = row.Cells[0].Value?.ToString() ?? "";
                string origFolder = row.Cells[4].Value?.ToString() ?? "";
                if (string.IsNullOrEmpty(origFolder) || origFolder == "未知") {
                    MessageBox.Show("無法判斷該備份的原資料夾，無法還原。", "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }

                string gamePath = GetGamePath();
                if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                    MessageBox.Show("遊戲路徑未設定，無法還原。", "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }

                string destDir = Path.Combine(gamePath, "SAVE", origFolder);
                if (Directory.Exists(destDir)) {
                    var dr = MessageBox.Show(string.Format("目標存檔資料夾 [{0}] 已存在，是否覆蓋？", origFolder), "確認覆蓋", MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
                    if (dr == DialogResult.No) return;
                    Directory.Delete(destDir, true);
                }

                string backupDir = Path.Combine(AppContext.BaseDirectory, "SavesBackup");
                string zipPath = Path.Combine(backupDir, file);

                Directory.CreateDirectory(destDir);
                ZipFile.ExtractToDirectory(zipPath, destDir);
                Log(string.Format("還原備份成功: {0} -> {1}", file, origFolder));
                RefreshSavesAndBackups();
                MessageBox.Show("還原備份成功！", "成功", MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log("還原備份失敗: " + ex.Message);
                MessageBox.Show("還原備份失敗: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        /// <summary>
        /// 當使用者點擊「刪除存檔」時觸發，永久刪除實體硬碟上的遊戲存檔資料夾。
        /// </summary>
        private void BtnDeleteSave_Click(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) {
                MessageBox.Show("請先選擇要刪除的存檔。", "提示", MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                var row = dgvGameSaves.SelectedRows[0];
                string folder = (row.Cells[0].Value?.ToString() ?? "").Trim();
                if (string.IsNullOrEmpty(folder)) {
                    MessageBox.Show("無效的存檔目錄，操作已取消。", "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                var dr = MessageBox.Show(string.Format("確定要永久刪除遊戲存檔 [{0}] 嗎？此操作不可還原！", folder), "確認刪除", MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
                if (dr == DialogResult.No) return;

                string gamePath = GetGamePath();
                string srcDir = Path.Combine(gamePath, "SAVE", folder);
                if (Directory.Exists(srcDir)) {
                    Directory.Delete(srcDir, true);
                }
                Log(string.Format("已刪除遊戲存檔: {0}", folder));
                RefreshSavesAndBackups();
                MessageBox.Show("已刪除存檔！", "成功", MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log("刪除存檔失敗: " + ex.Message);
                MessageBox.Show("刪除存檔失敗: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        /// <summary>
        /// 當使用者點擊「刪除備份」時觸發，永久刪除本地的備份 zip 檔案。
        /// </summary>
        private void BtnDeleteBackup_Click(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) {
                MessageBox.Show("請先選擇要刪除的備份。", "提示", MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                var row = dgvBackups.SelectedRows[0];
                string file = row.Cells[0].Value?.ToString() ?? "";
                var dr = MessageBox.Show(string.Format("確定要永久刪除備份檔案 [{0}] 嗎？", file), "確認刪除", MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
                if (dr == DialogResult.No) return;

                string backupDir = Path.Combine(AppContext.BaseDirectory, "SavesBackup");
                string zipPath = Path.Combine(backupDir, file);
                if (File.Exists(zipPath)) {
                    File.Delete(zipPath);
                    _backupSaveCache.Remove(file);
                }
                Log(string.Format("已刪除備份檔案: {0}", file));
                RefreshSavesAndBackups();
                MessageBox.Show("已刪除備份！", "成功", MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log("刪除備份失敗: " + ex.Message);
                MessageBox.Show("刪除備份失敗: " + ex.Message, "錯誤", MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }
    }
}
