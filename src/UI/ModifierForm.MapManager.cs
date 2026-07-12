using System;
using System.Diagnostics;
using System.IO;
using System.Collections.Generic;
using System.Linq;
using System.Windows.Forms;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier {
    public partial class ModifierForm {
        private readonly EndlessMapCatalog mapCatalog = new EndlessMapCatalog();
        private ListView mapList = null!;
        private Label mapManagerStatus = null!;

        private void InitializeMapManagerPage() {
            var title = new Label { Text = "地圖管理", AutoSize = true, Location = new System.Drawing.Point(22, 20), Font = fontJhengHei105B, ForeColor = System.Drawing.Color.FromArgb(0, 220, 255) };
            var description = new Label { Text = "檢視無盡模式地圖，並以獨立地圖編輯器安全建立或編輯自製地圖。", AutoSize = true, Location = new System.Drawing.Point(22, 58), ForeColor = System.Drawing.Color.Gainsboro };
            mapList = new ListView { Location = new System.Drawing.Point(22, 96), Size = new System.Drawing.Size(700, 560), View = View.Details, FullRowSelect = true, MultiSelect = false };
            mapList.Columns.Add("槽位", 100); mapList.Columns.Add("名稱", 400); mapList.Columns.Add("類型", 120);
            var launch = new Button { Text = "啟動地圖編輯器", Location = new System.Drawing.Point(22, 680), Size = new System.Drawing.Size(180, 38) };
            launch.Click += (_, _) => LaunchMapEditor();
            var refresh = new Button { Text = "重新整理", Location = new System.Drawing.Point(214, 680), Size = new System.Drawing.Size(120, 38) };
            refresh.Click += (_, _) => RefreshMapManager();
            mapManagerStatus = new Label { AutoSize = true, Location = new System.Drawing.Point(22, 735), ForeColor = System.Drawing.Color.SlateGray };
            tabMapManager.Controls.AddRange(new Control[] { title, description, mapList, launch, refresh, mapManagerStatus });
        }

        private void RefreshMapManager() {
            if (mapList == null) return;
            mapList.Items.Clear();
            string gamePath = GetGamePath();
            if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath)) { mapManagerStatus.Text = "請先選擇有效的遊戲路徑。"; return; }
            try {
                foreach (EndlessMapInfo map in mapCatalog.List(gamePath)) {
                    var item = new ListViewItem(map.Id) { Tag = map };
                    item.SubItems.Add(map.DisplayName ?? "(無標題)");
                    item.SubItems.Add(map.IsCustom ? "自製" : "原廠");
                    mapList.Items.Add(item);
                }
                mapManagerStatus.Text = $"已找到 {mapList.Items.Count} 張無盡模式地圖。";
            } catch (Exception ex) { mapManagerStatus.Text = "讀取地圖失敗: " + ex.Message; }
        }

        private void LaunchMapEditor() {
            string gamePath = GetGamePath();
            if (string.IsNullOrWhiteSpace(gamePath) || !Directory.Exists(gamePath)) { MessageBox.Show("請先選擇有效的遊戲路徑。", Loc.Get("TitlePathError"), MessageBoxButtons.OK, MessageBoxIcon.Error); return; }
            string? editorPath = ResolveMapEditorPath();
            if (editorPath == null) { MessageBox.Show("找不到 AgainstRomeMapEditor.exe。請重新建置修改器。", Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error); return; }
            string args = "--game \"" + gamePath + "\"";
            if (mapList.SelectedItems.Count == 1 && mapList.SelectedItems[0].Tag is EndlessMapInfo map) args += " --map " + map.Id;
            try { Process.Start(new ProcessStartInfo(editorPath, args) { WorkingDirectory = gamePath, UseShellExecute = true }); }
            catch (Exception ex) { MessageBox.Show("無法啟動地圖編輯器: " + ex.Message, Loc.Get("TitleError"), MessageBoxButtons.OK, MessageBoxIcon.Error); }
        }

        private static string? ResolveMapEditorPath() {
            var candidates = new List<string> {
                Path.Combine(AppContext.BaseDirectory, "AgainstRomeMapEditor.exe")
            };

            // 開發環境可能同時殘留主專案與 MapEditor 專案的輸出；以 DLL 時間選最新版，
            // 避免修改器啟動到先前建置留下的舊 UI。發佈環境則只會使用同目錄副本。
            DirectoryInfo? directory = new DirectoryInfo(AppContext.BaseDirectory);
            while (directory != null && !File.Exists(Path.Combine(directory.FullName, "AgainstRomeModifier.csproj"))) {
                directory = directory.Parent;
            }
            if (directory != null) {
                candidates.Add(Path.Combine(directory.FullName, "src.MapEditor", "bin", "Debug", "net8.0-windows", "AgainstRomeMapEditor.exe"));
                candidates.Add(Path.Combine(directory.FullName, "src.MapEditor", "bin", "Release", "net8.0-windows", "AgainstRomeMapEditor.exe"));
            }

            return candidates
                .Where(File.Exists)
                .OrderByDescending(path => {
                    string dll = Path.ChangeExtension(path, ".dll");
                    return File.Exists(dll) ? File.GetLastWriteTimeUtc(dll) : File.GetLastWriteTimeUtc(path);
                })
                .FirstOrDefault();
        }
    }
}
