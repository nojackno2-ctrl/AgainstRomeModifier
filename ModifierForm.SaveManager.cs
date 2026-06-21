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
                dgv.Columns.Add("Folder", Loc.Get("HeaderFolder"));
                dgv.Columns["Folder"].Width = 120;
                dgv.Columns.Add("Title", Loc.Get("HeaderSaveTitle"));
                dgv.Columns["Title"].Width = 330;
                dgv.Columns.Add("Level", Loc.Get("HeaderLevel"));
                dgv.Columns["Level"].Width = 140;
                dgv.Columns.Add("Time", Loc.Get("HeaderTime"));
                dgv.Columns["Time"].Width = 180;
            } else {
                dgv.Columns.Add("File", Loc.Get("HeaderBackupFile"));
                dgv.Columns["File"].Width = 120;
                dgv.Columns.Add("Title", Loc.Get("HeaderSaveTitle"));
                dgv.Columns["Title"].Width = 230;
                dgv.Columns.Add("Level", Loc.Get("HeaderLevel"));
                dgv.Columns["Level"].Width = 120;
                dgv.Columns.Add("Time", Loc.Get("HeaderBackupTime"));
                dgv.Columns["Time"].Width = 160;
                dgv.Columns.Add("Folder", Loc.Get("HeaderOrigFolder"));
                dgv.Columns["Folder"].Width = 120;
            }

            return dgv;
        }

        /// <summary>
        /// 重新整理並讀取遊戲存檔目錄 (SAVE) 與備份目錄 (SavesBackup) 下的資料，並將結果載入到介面表格中。
        /// </summary>
        private async void RefreshSavesAndBackups() {
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
                    Log(Loc.Get("LogSavePathNotSet"));
                    return;
                }

                var (savesData, backupsData) = await Task.Run(() => {
                    var savesList = new List<object[]>();
                    var backupsList = new List<object[]>();

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
                                    title = Loc.Get("Unparsable");
                                    level = Loc.Get("Unknown");
                                }
                                DateTime writeTime = Directory.GetLastWriteTime(dir);
                                savesList.Add(new object[] { folderName, title, level, writeTime.ToString("yyyy-MM-dd HH:mm:ss") });
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
                        lock (_backupSaveCache) {
                            if (_backupSaveCache.TryGetValue(fileName, out var existingCache) && existingCache.LastWriteTime == lastWrite) {
                                cache = existingCache;
                            }
                        }

                        if (cache == null) {
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
                                title = Loc.Get("Unparsable");
                                level = Loc.Get("Unknown");
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
                                            origFolder = Loc.Get("Unknown");
                            }

                            cache = new BackupSaveCache {
                                FileName = fileName,
                                Title = title,
                                Level = level,
                                OrigFolder = origFolder,
                                BackupTimeStr = backupTimeStr,
                                LastWriteTime = lastWrite
                            };
                            lock (_backupSaveCache) {
                                _backupSaveCache[fileName] = cache;
                            }
                        }

                        backupsList.Add(new object[] { cache.FileName, cache.Title, cache.Level, cache.BackupTimeStr, cache.OrigFolder });
                    }

                    return (savesList, backupsList);
                });

                foreach (var row in savesData) {
                    dgvGameSaves.Rows.Add(row);
                }
                foreach (var row in backupsData) {
                    dgvBackups.Rows.Add(row);
                }
            } catch (Exception ex) {
                Log(Loc.Get("LogRefreshSavesFailed") + ex.Message);
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
                    Loc.Get("SaveDetailGameSave"),
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
                Log(Loc.Get("LogRefreshSavesFailed") + ex.Message);
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
                    Loc.Get("SaveDetailBackup"),
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
                Log(Loc.Get("LogRefreshSavesFailed") + ex.Message);
            }
        }

        /// <summary>
        /// 當使用者點擊「備份此存檔」時觸發，將選取的存檔資料夾壓縮備份至 SavesBackup 目錄。
        /// </summary>
        private void BtnBackupSave_Click(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectSaveToBackup"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                var row = dgvGameSaves.SelectedRows[0];
                string folder = (row.Cells[0].Value?.ToString() ?? "").Trim();
                if (string.IsNullOrEmpty(folder)) {
                    MessageBox.Show(Loc.Get("MsgInvalidSaveDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                string title = row.Cells[1].Value?.ToString() ?? "";
                string gamePath = GetGamePath();
                string srcDir = Path.Combine(gamePath, "SAVE", folder);
                if (!Directory.Exists(srcDir)) {
                    MessageBox.Show(Loc.Get("MsgNoOrigFolderToBackup"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
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
                Log(string.Format(Loc.Get("LogBackupSaveSuccessDetail"), folder, zipName));
                RefreshSavesAndBackups();
                MessageBox.Show(Loc.Get("MsgBackupSaveSuccess"), Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log(Loc.Get("LogBackupSaveFailedDetail") + ex.Message);
                MessageBox.Show(Loc.Get("MsgBackupSaveFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        /// <summary>
        /// 當使用者點擊「還原此備份」時觸發，解壓所選的備份檔案，並覆蓋回原存檔目錄。
        /// </summary>
        private void BtnRestoreBackup_Click(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectBackup"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                var row = dgvBackups.SelectedRows[0];
                string file = row.Cells[0].Value?.ToString() ?? "";
                string origFolder = row.Cells[4].Value?.ToString() ?? "";
                if (string.IsNullOrEmpty(origFolder) || origFolder == Loc.Get("Unknown")) {
                    MessageBox.Show(Loc.Get("MsgCannotResolveOrigFolder"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }

                string gamePath = GetGamePath();
                if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                    MessageBox.Show(Loc.Get("MsgGamePathNotSet"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }

                string destDir = Path.Combine(gamePath, "SAVE", origFolder);
                if (Directory.Exists(destDir)) {
                    var dr = MessageBox.Show(string.Format(Loc.Get("MsgConfirmOverwriteSave"), origFolder), Loc.Get("TitleWarning"), MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
                    if (dr == DialogResult.No) return;
                    Directory.Delete(destDir, true);
                }

                string backupDir = Path.Combine(AppContext.BaseDirectory, "SavesBackup");
                string zipPath = Path.Combine(backupDir, file);

                Directory.CreateDirectory(destDir);
                ZipFile.ExtractToDirectory(zipPath, destDir);
                Log(string.Format(Loc.Get("LogRestoreBackupSuccessDetail"), file, origFolder));
                RefreshSavesAndBackups();
                MessageBox.Show(Loc.Get("MsgRestoreBackupSuccess"), Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log(Loc.Get("LogRestoreBackupFailedDetail") + ex.Message);
                MessageBox.Show(Loc.Get("MsgRestoreBackupFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        /// <summary>
        /// 當使用者點擊「刪除存檔」時觸發，永久刪除實體硬碟上的遊戲存檔資料夾。
        /// </summary>
        private void BtnDeleteSave_Click(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectSaveToDelete"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                var row = dgvGameSaves.SelectedRows[0];
                string folder = (row.Cells[0].Value?.ToString() ?? "").Trim();
                if (string.IsNullOrEmpty(folder)) {
                    MessageBox.Show(Loc.Get("MsgInvalidSaveDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                var dr = MessageBox.Show(string.Format(Loc.Get("MsgConfirmDeleteSave"), folder), Loc.Get("TitleConfirmDelete"), MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
                if (dr == DialogResult.No) return;

                string gamePath = GetGamePath();
                string srcDir = Path.Combine(gamePath, "SAVE", folder);
                if (Directory.Exists(srcDir)) {
                    Directory.Delete(srcDir, true);
                }
                Log(string.Format(Loc.Get("LogDeleteSaveSuccessDetail"), folder));
                RefreshSavesAndBackups();
                MessageBox.Show(Loc.Get("MsgDeleteSaveSuccess"), Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log(Loc.Get("LogDeleteSaveFailedDetail") + ex.Message);
                MessageBox.Show(Loc.Get("MsgDeleteSaveFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        /// <summary>
        /// 當使用者點擊「刪除備份」時觸發，永久刪除本地的備份 zip 檔案。
        /// </summary>
        private void BtnDeleteBackup_Click(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectBackupToDelete"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                var row = dgvBackups.SelectedRows[0];
                string file = row.Cells[0].Value?.ToString() ?? "";
                var dr = MessageBox.Show(string.Format(Loc.Get("MsgConfirmDeleteBackup"), file), Loc.Get("TitleConfirmDelete"), MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
                if (dr == DialogResult.No) return;

                string backupDir = Path.Combine(AppContext.BaseDirectory, "SavesBackup");
                string zipPath = Path.Combine(backupDir, file);
                if (File.Exists(zipPath)) {
                    File.Delete(zipPath);
                    _backupSaveCache.Remove(file);
                }
                Log(string.Format(Loc.Get("LogDeleteBackupSuccessDetail"), file));
                RefreshSavesAndBackups();
                MessageBox.Show(Loc.Get("MsgDeleteBackupSuccess"), Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log(Loc.Get("LogDeleteBackupFailedDetail") + ex.Message);
                MessageBox.Show(Loc.Get("MsgDeleteBackupFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }
    }
}
