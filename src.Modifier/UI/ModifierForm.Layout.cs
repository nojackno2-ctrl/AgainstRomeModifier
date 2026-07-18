using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Collections.Generic;
using System.Globalization;
using System.Windows.Forms;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Runtime.InteropServices;

namespace AgainstRomeModifier {
    // 主表單的版面配置：現代化外殼、側邊欄、設定卡片、統計頁與存檔管理頁的響應式排版。
    // 由 ModifierForm.cs 拆出（純程式碼搬移，行為不變）。
    public partial class ModifierForm {
        private void ApplyModernLayout() {
            SuspendLayout();

            Size = new Size(2000, 940);
            MinimumSize = new Size(1800, 760);
            AutoScroll = false;
            BackColor = Color.FromArgb(9, 12, 18);

            pnlTitleBar.Height = 56;
            pnlTitleBar.BackColor = Color.FromArgb(13, 17, 25);
            lblMainTitle.Location = new Point(24, 16);
            lblMainTitle.Size = new Size(360, 26);
            lblMainTitle.ForeColor = Color.FromArgb(226, 241, 252);


            pnlSidebar.BackColor = Color.FromArgb(12, 16, 24);
            pnlSidebar.Width = 250;

            pnlRightSidebar.BackColor = Color.FromArgb(12, 16, 24);
            pnlRightSidebar.Width = 250;

            ConfigureSidebarLayout();
            ConfigureSystemDashboard();
            ConfigureStatsPages();



            foreach (DataGridView grid in defaultStatsGrids.Values.Concat(currentStatsGrids.Values)) {
                grid.ScrollBars = ScrollBars.Both;
            }

            LayoutModernShell();
            Resize += (s, e) => LayoutModernShell();
            ResumeLayout(true);
        }

        private void LayoutModernShell() {
            pnlTitleBar.Location = Point.Empty;
            pnlTitleBar.Size = new Size(ClientSize.Width, 56);
            btnClose.Location = new Point(ClientSize.Width - 46, 12);
            btnMinimize.Location = new Point(ClientSize.Width - 86, 12);

            pnlSidebar.Location = new Point(0, 56);
            pnlSidebar.Size = new Size(250, Math.Max(0, ClientSize.Height - 56));

            pnlRightSidebar.Location = new Point(ClientSize.Width - 250, 56);
            pnlRightSidebar.Size = new Size(250, Math.Max(0, ClientSize.Height - 56));

            mainTabControl.Location = new Point(266, 70);
            mainTabControl.Size = new Size(
                Math.Max(0, ClientSize.Width - 532),
                Math.Max(0, ClientSize.Height - 84));

            lblSidebarLang.Location = new Point(16, Math.Max(610, pnlSidebar.Height - 72));
            btnLangZH.Location = new Point(16, Math.Max(634, pnlSidebar.Height - 46));
            btnLangEN.Location = new Point(126, Math.Max(634, pnlSidebar.Height - 46));
        }

        private void ConfigureSidebarLayout() {
            Button[] navButtons = {
                btnNavSystem,
                btnNavDefaultStats,
                btnNavCurrentStats
            };
            for (int i = 0; i < navButtons.Length; i++) {
                navButtons[i].Location = new Point(10, 22 + i * 52);
                navButtons[i].Size = new Size(230, 44);
            }

            lblGamePath.Location = new Point(16, 22);
            lblGamePath.Size = new Size(218, 20);
            lblGamePath.ForeColor = Color.FromArgb(128, 143, 163);

            Panel pathWrapper = txtGamePath.Parent as Panel
                ?? throw new InvalidOperationException("Game path input wrapper was not initialized.");
            pathWrapper.Location = new Point(16, 48);
            pathWrapper.Size = new Size(218, 32);
            pathWrapper.BackColor = Color.FromArgb(22, 28, 39);
            txtGamePath.Location = new Point(9, 7);
            txtGamePath.Size = new Size(200, 20);
            txtGamePath.BackColor = pathWrapper.BackColor;
            txtGamePath.ForeColor = Color.FromArgb(222, 230, 240);

            btnBrowseGamePath.Location = new Point(16, 88);
            btnBrowseGamePath.Size = new Size(218, 34);
            btnLoadCurrent.Location = new Point(16, 148);
            btnRestore.Location = new Point(16, 198);
            btnApply.Location = new Point(16, 258);
            btnStartGame.Location = new Point(16, 308);
            foreach (Button actionButton in new[] { btnLoadCurrent, btnRestore, btnApply, btnStartGame }) {
                actionButton.Size = new Size(218, 40);
            }

            lblSidebarLang.Size = new Size(218, 20);
            lblSidebarLang.ForeColor = Color.FromArgb(128, 143, 163);
            btnLangZH.Size = new Size(102, 30);
            btnLangEN.Size = new Size(108, 30);
        }

        private void ConfigureSystemDashboard() {
            tabSystem.BackColor = Color.FromArgb(9, 12, 18);

            Panel header = new Panel {
                Dock = DockStyle.Top,
                Height = 72,
                BackColor = Color.FromArgb(9, 12, 18)
            };
            lblSystemHeading = new Label {
                Text = Loc.Get("SystemHeading"),
                Location = new Point(4, 4),
                Size = new Size(430, 28),
                Font = fontJhengHei115B,
                ForeColor = Color.FromArgb(235, 242, 250),
                BackColor = Color.Transparent
            };
            lblSystemSubtitle = new Label {
                Text = Loc.Get("SystemSubtitle"),
                Location = new Point(4, 35),
                Size = new Size(620, 22),
                Font = fontJhengHei9R,
                ForeColor = Color.FromArgb(128, 143, 163),
                BackColor = Color.Transparent
            };
            header.Controls.Add(lblSystemHeading);
            header.Controls.Add(lblSystemSubtitle);
            header.Controls.Add(btnEnableAll);
            header.Controls.Add(btnDisableAll);
            btnEnableAll.Size = new Size(136, 36);
            btnDisableAll.Size = new Size(136, 36);
            header.Resize += (s, e) => {
                btnDisableAll.Location = new Point(Math.Max(0, header.Width - 140), 12);
                btnEnableAll.Location = new Point(Math.Max(0, header.Width - 284), 12);
            };

            Panel pnlContent = new Panel {
                Location = new Point(0, 72),
                Size = new Size(tabSystem.ClientSize.Width, tabSystem.ClientSize.Height - 72),
                Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right,
                BackColor = Color.FromArgb(9, 12, 18),
                // 卡片在較矮視窗仍完整可用；寬螢幕時則維持沒有捲軸的四欄工作區。
                AutoScroll = true
            };

            // 依功能語意分為六張卡片：系統相容、建設經濟、村民操作、法術祭司、戰鬥部隊、無盡模式。
            ConfigureSettingsCard(pnlNumericCard, lblNumericTitle, 398,
                chkFocusLoss,
                chkToEng,
                chkDgVoodoo,
                chkArgmTrace,
                chkGameSpeed,
                chkNativeWidescreen1920x1080,
                chkCameraZoomOut1);
            ConfigureSettingsCard(pnlBuildCard, lblBuildTitle, 446,
                chkFreeProd,
                chkFreeUpgrade,
                chkMaxPopulation,
                chkHousingCapacity20x,
                chkStorageCapacity10x,
                chkHqHp10x,
                chkFastBuildUpgradeRepair,
                chkVillageBuildRange);
            ConfigureSettingsCard(pnlVillagerCard, lblVillagerTitle, 302,
                chkFastCiviProduction,
                chkCiviProduce20,
                chkUnitRecruit20,
                chkIdleSelect999,
                chkVillagerMovementSpeed5x);
            ConfigureSettingsCard(pnlSpellCard, lblSpellTitle, 398,
                chkNoSpellCost,
                chkNoSpellAltar,
                chkSpellDamage5x,
                chkSpellHealing10x,
                chkSpellResurrection,
                chkSpellEntireMap,
                chkSpellRange3x);
            ConfigureSettingsCard(pnlCombatCard, lblCombatTitle, 494,
                chkInfiniteMorale,
                chkFoodHealing10x,
                chkGeneralSkills,
                chkLeaderGlory,
                chkBalance,
                chkRangedRange3x,
                chkProjectileArcHeight,
                chkUnitMovementSpeed2x,
                chkAllUnitsEntireMapVision);
            ConfigureSettingsCard(pnlAiCard, lblAiTitle, 302,
                chkRomanEndless,
                chkAiM1, chkAiCore, chkAiM5, chkRomanReinforcementGarrison);

            // 每組設定以同一層卡片底色收攏，讓長短不一的功能群組仍有清楚邊界。
            Color cardBackColor = Color.FromArgb(14, 18, 26);
            pnlNumericCard.BackColor = cardBackColor;
            pnlSpellCard.BackColor = cardBackColor;
            pnlVillagerCard.BackColor = cardBackColor;
            pnlCombatCard.BackColor = cardBackColor;
            pnlBuildCard.BackColor = cardBackColor;
            pnlAiCard.BackColor = cardBackColor;

            pnlNumericCard.Dock = DockStyle.None;
            pnlSpellCard.Dock = DockStyle.None;
            pnlVillagerCard.Dock = DockStyle.None;
            pnlCombatCard.Dock = DockStyle.None;
            pnlBuildCard.Dock = DockStyle.None;
            pnlAiCard.Dock = DockStyle.None;

            pnlContent.Controls.Add(pnlNumericCard);
            pnlContent.Controls.Add(pnlSpellCard);
            pnlContent.Controls.Add(pnlVillagerCard);
            pnlContent.Controls.Add(pnlCombatCard);
            pnlContent.Controls.Add(pnlBuildCard);
            pnlContent.Controls.Add(pnlAiCard);

            tabSystem.Controls.Clear();
            tabSystem.Controls.Add(pnlContent);
            tabSystem.Controls.Add(header);
            header.BringToFront();

            pnlContent.Resize += (s, e) => LayoutSystemCardsFlat(pnlContent);
            LayoutSystemCardsFlat(pnlContent);
        }

        private void LayoutSystemCardsFlat(Panel container) {
            container.SuspendLayout();

            int paddingX = 20;
            int paddingY = 20;
            int gapX = 24;
            int gapY = 16;

            int availWidth = container.ClientSize.Width - (paddingX * 2);
            int columnCount = 4;
            int columnWidth = (availWidth - (gapX * (columnCount - 1))) / columnCount;
            if (columnWidth < 260) columnWidth = 260;

            // 四欄瀑布式排列：每張卡片放入目前總高度最短的欄位。
            // 新增或調整卡片高度後會自動維持緊湊，不再依賴固定欄位歸屬。
            int[] columnBottoms = Enumerable.Repeat(paddingY, columnCount).ToArray();
            Panel[] cards = {
                pnlNumericCard,
                pnlBuildCard,
                pnlCombatCard,
                pnlSpellCard,
                pnlVillagerCard,
                pnlAiCard
            };
            foreach (Panel card in cards) {
                int column = Array.IndexOf(columnBottoms, columnBottoms.Min());
                card.Location = new Point(paddingX + column * (columnWidth + gapX), columnBottoms[column]);
                card.Width = columnWidth;
                columnBottoms[column] = card.Bottom + gapY;
            }

            int contentHeight = columnBottoms.Max() - gapY + paddingY;
            int contentWidth = paddingX * 2 + columnWidth * columnCount + gapX * (columnCount - 1);
            container.AutoScrollMinSize = new Size(Math.Max(container.ClientSize.Width, contentWidth), contentHeight);

            container.ResumeLayout(true);
        }

        private void ConfigureSettingsCard(
            Panel card,
            Label title,
            int height,
            params ModernToggle[] toggles) {
            card.Height = height;
            card.MinimumSize = new Size(0, height);
            card.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            card.Margin = new Padding(6, 0, 6, 0);
            card.BackColor = Color.FromArgb(14, 18, 26);

            title.Location = new Point(20, 17);
            title.Size = new Size(280, 24);
            title.ForeColor = Color.FromArgb(105, 205, 255);

            void LayoutRows() {
                for (int i = 0; i < toggles.Length; i++) {
                    ModernToggle toggle = toggles[i];
                    int y = 60 + i * 48;
                    toggle.Location = new Point(20, y);
                    toggle.Size = new Size(Math.Max(120, card.Width - 40), 26);
                    toggle.Font = fontJhengHei95R;
                    toggle.BackColor = card.BackColor;
                }
            }

            card.Resize += (s, e) => LayoutRows();
            LayoutRows();
        }





        private void ConfigureStatsPages() {
            Panel defaultHeader = lblDefaultStatsTitle.Parent as Panel
                ?? throw new InvalidOperationException("Default stats header was not initialized.");
            ConfigureStatsPage(tabDefaultStats, defaultHeader, defaultStatsTabControl);

            defaultHeader.Resize += (s, e) => {
                lblTroopPresetFile.Width = Math.Max(120, defaultHeader.Width - lblTroopPresetFile.Left - 18);
            };

            Panel currentHeader = lblCurrentStatsTitle.Parent as Panel
                ?? throw new InvalidOperationException("Current stats header was not initialized.");
            ConfigureStatsPage(tabCurrentStats, currentHeader, currentStatsTabControl);

            void ConfigureStatsPage(TabPage page, Panel header, TabControl statsTabs) {
                // Keep the title card and the tab content in separate layout rows. A Fill-docked
                // TabControl placed behind a Top-docked header still starts at y=0, which causes
                // the header to cover the faction tabs and most of the grid column headings.
                page.Controls.Remove(header);
                page.Controls.Remove(statsTabs);

                var layout = new TableLayoutPanel {
                    Dock = DockStyle.Fill,
                    BackColor = page.BackColor,
                    ColumnCount = 1,
                    RowCount = 3,
                    Margin = new Padding(0),
                    Padding = new Padding(0)
                };
                layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100F));
                layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 68F));
                layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 40F));
                layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100F));

                header.Dock = DockStyle.Fill;
                header.Margin = new Padding(0);
                header.BackColor = Color.FromArgb(18, 22, 31);
                statsTabs.Dock = DockStyle.Fill;
                statsTabs.Margin = new Padding(0);
                statsTabs.ItemSize = new Size(0, 1);
                statsTabs.Font = fontJhengHei95R;
                if (statsTabs is ModernTabControl modernTabs) {
                    modernTabs.HideTabs = true;
                }

                var factionBar = new TableLayoutPanel {
                    Dock = DockStyle.Fill,
                    BackColor = Color.FromArgb(14, 17, 24),
                    ColumnCount = statsTabs.TabCount,
                    RowCount = 1,
                    Margin = new Padding(0),
                    Padding = new Padding(0)
                };
                factionBar.RowStyles.Add(new RowStyle(SizeType.Percent, 100F));

                var factionButtons = new List<Button>();
                for (int i = 0; i < statsTabs.TabCount; i++) {
                    int tabIndex = i;
                    factionBar.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100F / statsTabs.TabCount));

                    var button = new Button {
                        Dock = DockStyle.Fill,
                        FlatStyle = FlatStyle.Flat,
                        BackColor = Color.FromArgb(14, 17, 24),
                        Cursor = Cursors.Hand,
                        Margin = new Padding(0),
                        TabStop = false,
                        UseVisualStyleBackColor = false
                    };
                    button.FlatAppearance.BorderSize = 0;
                    button.Paint += (s, e) => {
                        bool selected = statsTabs.SelectedIndex == tabIndex;
                        Color background = selected
                            ? Color.FromArgb(26, 31, 43)
                            : Color.FromArgb(14, 17, 24);
                        e.Graphics.Clear(background);

                        if (tabIndex > 0) {
                            using (var divider = new Pen(Color.FromArgb(38, 44, 58))) {
                                e.Graphics.DrawLine(divider, 0, 8, 0, button.Height - 8);
                            }
                        }
                        if (selected) {
                            using (var indicator = new SolidBrush(Color.FromArgb(62, 203, 255))) {
                                e.Graphics.FillRectangle(indicator, 12, button.Height - 3, button.Width - 24, 3);
                            }
                        }

                        TextRenderer.DrawText(
                            e.Graphics,
                            statsTabs.TabPages[tabIndex].Text.Trim(),
                            selected ? fontJhengHei95B : fontJhengHei95R,
                            button.ClientRectangle,
                            selected ? Color.FromArgb(235, 248, 255) : Color.FromArgb(145, 155, 172),
                            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.EndEllipsis);
                    };
                    button.Click += (s, e) => statsTabs.SelectedIndex = tabIndex;
                    factionButtons.Add(button);
                    factionBar.Controls.Add(button, i, 0);
                }
                statsTabs.SelectedIndexChanged += (s, e) => {
                    foreach (Button button in factionButtons) {
                        button.Invalidate();
                    }
                };

                layout.Controls.Add(header, 0, 0);
                layout.Controls.Add(factionBar, 0, 1);
                layout.Controls.Add(statsTabs, 0, 2);
                page.Controls.Add(layout);
            }
        }


        private Panel CreateInputWrapper(int x, int y, int w, int h) {
            Panel p = new Panel {
                Location = new Point(x, y),
                Size = new Size(w, h),
                BackColor = Color.FromArgb(38, 38, 48)
            };
            p.Paint += InputPanel_Paint;
            return p;
        }
    }
}
