using System;
using System.IO;
using System.Threading.Tasks;
using System.Windows.Forms;
using System.Drawing;
using AgainstRomeModifier.Core.Services;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        private DataGridView CreateSaveGrid(bool isBackup) {
            var dgv = new DataGridView {
                AllowUserToAddRows = false,
                AllowUserToDeleteRows = false,
                RowHeadersVisible = false,
                BackgroundColor = Color.FromArgb(10, 11, 16),
                ForeColor = Color.FromArgb(230, 235, 240),
                GridColor = Color.FromArgb(28, 30, 42),
                BorderStyle = BorderStyle.None,
                EnableHeadersVisualStyles = false,
                RowTemplate = { Height = 35 },
                SelectionMode = DataGridViewSelectionMode.FullRowSelect,
                MultiSelect = false,
                ReadOnly = true
            };

            dgv.ColumnHeadersDefaultCellStyle.BackColor = Color.FromArgb(26, 27, 37);
            dgv.ColumnHeadersDefaultCellStyle.ForeColor = Color.FromArgb(0, 230, 255);
            dgv.ColumnHeadersDefaultCellStyle.SelectionBackColor = Color.FromArgb(26, 27, 37);
            dgv.ColumnHeadersDefaultCellStyle.Font = fontJhengHei95B;
            dgv.ColumnHeadersHeight = 35;
            dgv.ColumnHeadersBorderStyle = DataGridViewHeaderBorderStyle.Single;
            dgv.DefaultCellStyle.BackColor = Color.FromArgb(20, 21, 31);
            dgv.DefaultCellStyle.ForeColor = Color.FromArgb(230, 235, 240);
            dgv.DefaultCellStyle.SelectionBackColor = Color.FromArgb(35, 37, 54);
            dgv.DefaultCellStyle.SelectionForeColor = Color.White;
            dgv.DefaultCellStyle.Font = fontJhengHei9R;
            dgv.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(24, 25, 35);

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

        private bool _savesRefreshInFlight;

        private async void RefreshSavesAndBackups() {
            if (_savesRefreshInFlight || saveBackupService == null) return;
            _savesRefreshInFlight = true;
            try {
                dgvGameSaves.Rows.Clear();
                dgvBackups.Rows.Clear();
                ReplaceSavePreview(null);
                lblSaveDetail.Text = "";

                string gamePath = GetGamePath();
                if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                    Log(Loc.Get("LogSavePathNotSet"));
                    return;
                }

                SaveBackupCatalog catalog = await Task.Run(() => saveBackupService.Scan(gamePath));
                foreach (GameSaveInfo save in catalog.Saves) {
                    dgvGameSaves.Rows.Add(
                        save.Folder,
                        save.Parsed ? save.Title : Loc.Get("Unparsable"),
                        save.Parsed ? save.Level : Loc.Get("Unknown"),
                        save.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss"));
                }
                foreach (SaveBackupInfo backup in catalog.Backups) {
                    dgvBackups.Rows.Add(
                        backup.FileName,
                        backup.Parsed ? backup.Title : Loc.Get("Unparsable"),
                        backup.Parsed ? backup.Level : Loc.Get("Unknown"),
                        backup.BackupTime,
                        string.IsNullOrEmpty(backup.OrigFolder) ? Loc.Get("Unknown") : backup.OrigFolder);
                }
            } catch (Exception ex) {
                Log(Loc.Get("LogRefreshSavesFailed") + ex.Message);
            } finally {
                _savesRefreshInFlight = false;
            }
        }

        private void DgvGameSaves_SelectionChanged(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) return;
            try {
                DataGridViewRow row = dgvGameSaves.SelectedRows[0];
                string folder = Cell(row, 0);
                lblSaveDetail.Text = string.Format(Loc.Get("SaveDetailGameSave"),
                    folder, Cell(row, 1), Cell(row, 2), Cell(row, 3));
                string gamePath = GetGamePath();
                ReplaceSavePreview(string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)
                    ? null
                    : saveBackupService.ReadSavePreview(gamePath, folder));
            } catch (Exception ex) {
                Log(Loc.Get("LogRefreshSavesFailed") + ex.Message);
            }
        }

        private void DgvBackups_SelectionChanged(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) return;
            try {
                DataGridViewRow row = dgvBackups.SelectedRows[0];
                string file = Cell(row, 0);
                lblSaveDetail.Text = string.Format(Loc.Get("SaveDetailBackup"),
                    file, Cell(row, 4), Cell(row, 1), Cell(row, 2), Cell(row, 3));
                ReplaceSavePreview(saveBackupService.ReadBackupPreview(file));
            } catch (Exception ex) {
                Log(Loc.Get("LogRefreshSavesFailed") + ex.Message);
            }
        }

        private void BtnBackupSave_Click(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectSaveToBackup"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                DataGridViewRow row = dgvGameSaves.SelectedRows[0];
                string folder = Cell(row, 0).Trim();
                if (!SaveBackupService.IsSimpleName(folder)) {
                    MessageBox.Show(Loc.Get("MsgInvalidSaveDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                string fileName = saveBackupService.CreateBackup(GetGamePath(), folder, Cell(row, 1), Cell(row, 2));
                Log(string.Format(Loc.Get("LogBackupSaveSuccessDetail"), folder, fileName));
                RefreshSavesAndBackups();
                MessageBox.Show(Loc.Get("MsgBackupSaveSuccess"), Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (DirectoryNotFoundException) {
                MessageBox.Show(Loc.Get("MsgNoOrigFolderToBackup"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            } catch (Exception ex) {
                Log(Loc.Get("LogBackupSaveFailedDetail") + ex.Message);
                MessageBox.Show(Loc.Get("MsgBackupSaveFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        private void BtnRestoreBackup_Click(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectBackup"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                DataGridViewRow row = dgvBackups.SelectedRows[0];
                string file = Cell(row, 0);
                string folder = Cell(row, 4);
                if (!SaveBackupService.IsSimpleName(file)) {
                    MessageBox.Show(Loc.Get("MsgSelectBackup"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                if (!SaveBackupService.IsSimpleName(folder) || folder == Loc.Get("Unknown")) {
                    MessageBox.Show(Loc.Get("MsgCannotResolveOrigFolder"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                string gamePath = GetGamePath();
                if (string.IsNullOrEmpty(gamePath) || !Directory.Exists(gamePath)) {
                    MessageBox.Show(Loc.Get("MsgGamePathNotSet"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                if (Directory.Exists(Path.Combine(gamePath, "SAVE", folder)) &&
                    MessageBox.Show(string.Format(Loc.Get("MsgConfirmOverwriteSave"), folder), Loc.Get("TitleWarning"),
                        MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes) return;

                SaveRestoreResult result = saveBackupService.RestoreBackup(gamePath, file, folder,
                    message => Log("還原存檔失敗後回復原存檔也失敗: " + message));
                foreach (string warning in result.CleanupWarnings)
                    Log(Loc.Get("LogRestoreBackupCleanupFailed") + warning);
                Log(string.Format(Loc.Get("LogRestoreBackupSuccessDetail"), file, folder));
                RefreshSavesAndBackups();
                MessageBox.Show(Loc.Get("MsgRestoreBackupSuccess"), Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log(Loc.Get("LogRestoreBackupFailedDetail") + ex.Message);
                MessageBox.Show(Loc.Get("MsgRestoreBackupFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        private void BtnDeleteSave_Click(object? sender, EventArgs e) {
            if (dgvGameSaves.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectSaveToDelete"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                string folder = Cell(dgvGameSaves.SelectedRows[0], 0).Trim();
                if (!SaveBackupService.IsSimpleName(folder)) {
                    MessageBox.Show(Loc.Get("MsgInvalidSaveDir"), Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return;
                }
                if (MessageBox.Show(string.Format(Loc.Get("MsgConfirmDeleteSave"), folder), Loc.Get("TitleConfirmDelete"),
                        MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes) return;
                saveBackupService.DeleteSave(GetGamePath(), folder);
                Log(string.Format(Loc.Get("LogDeleteSaveSuccessDetail"), folder));
                RefreshSavesAndBackups();
                MessageBox.Show(Loc.Get("MsgDeleteSaveSuccess"), Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log(Loc.Get("LogDeleteSaveFailedDetail") + ex.Message);
                MessageBox.Show(Loc.Get("MsgDeleteSaveFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        private void BtnDeleteBackup_Click(object? sender, EventArgs e) {
            if (dgvBackups.SelectedRows.Count == 0) {
                MessageBox.Show(Loc.Get("MsgSelectBackupToDelete"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                return;
            }
            try {
                string file = Cell(dgvBackups.SelectedRows[0], 0);
                if (!SaveBackupService.IsSimpleName(file)) {
                    MessageBox.Show(Loc.Get("MsgSelectBackupToDelete"), Loc.Get("TitleTips"), MessageBoxButtons.OK, MessageBoxIcon.Information);
                    return;
                }
                if (MessageBox.Show(string.Format(Loc.Get("MsgConfirmDeleteBackup"), file), Loc.Get("TitleConfirmDelete"),
                        MessageBoxButtons.YesNo, MessageBoxIcon.Warning) != DialogResult.Yes) return;
                saveBackupService.DeleteBackup(file);
                Log(string.Format(Loc.Get("LogDeleteBackupSuccessDetail"), file));
                RefreshSavesAndBackups();
                MessageBox.Show(Loc.Get("MsgDeleteBackupSuccess"), Loc.Get("TitleSuccess"), MessageBoxButtons.OK, MessageBoxIcon.Information);
            } catch (Exception ex) {
                Log(Loc.Get("LogDeleteBackupFailedDetail") + ex.Message);
                MessageBox.Show(Loc.Get("MsgDeleteBackupFailed") + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
        }

        private void ReplaceSavePreview(byte[]? bytes) {
            picSavePreview.Image?.Dispose();
            picSavePreview.Image = bytes == null ? null : LoadTga(bytes);
        }

        private static string Cell(DataGridViewRow row, int index) => row.Cells[index].Value?.ToString() ?? "";
    }
}
