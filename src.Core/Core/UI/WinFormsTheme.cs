using System.Drawing;
using System.Windows.Forms;

namespace AgainstRomeModifier;

/// <summary>
/// Shared native visual language for every WinForms surface in the product.
/// Shape rule: 12px containers, 8px controls, and compact square utility controls.
/// </summary>
public static class WinFormsTheme
{
    public static readonly Color Window = Color.FromArgb(17, 19, 23);
    public static readonly Color Surface = Color.FromArgb(24, 27, 32);
    public static readonly Color SurfaceRaised = Color.FromArgb(31, 35, 41);
    public static readonly Color SurfaceHover = Color.FromArgb(39, 44, 51);
    public static readonly Color Border = Color.FromArgb(61, 67, 76);
    public static readonly Color BorderStrong = Color.FromArgb(83, 90, 100);
    public static readonly Color TextPrimary = Color.FromArgb(239, 240, 242);
    public static readonly Color TextSecondary = Color.FromArgb(177, 182, 190);
    public static readonly Color TextMuted = Color.FromArgb(131, 137, 146);
    public static readonly Color Accent = Color.FromArgb(201, 132, 73);
    public static readonly Color AccentHover = Color.FromArgb(220, 151, 88);
    public static readonly Color AccentPressed = Color.FromArgb(175, 109, 55);
    public static readonly Color AccentSoft = Color.FromArgb(55, 41, 31);
    public static readonly Color Danger = Color.FromArgb(218, 91, 91);
    public static readonly Color Warning = Color.FromArgb(222, 177, 94);
    public static readonly Color Success = Color.FromArgb(105, 184, 137);
    public static readonly Color Selection = Color.FromArgb(75, 52, 36);

    public const int ContainerRadius = 12;
    public const int ControlRadius = 8;

    public static Font CreateFont(float size, FontStyle style = FontStyle.Regular)
        => new("Segoe UI Variable Text", size, style, GraphicsUnit.Point);

    public static Font CreateDisplayFont(float size, FontStyle style = FontStyle.Bold)
        => new("Segoe UI Variable Display", size, style, GraphicsUnit.Point);

    public static void Apply(Form form)
    {
        form.BackColor = Window;
        form.ForeColor = TextPrimary;
        Apply(form.Controls);
    }

    public static void Apply(Control.ControlCollection controls)
    {
        foreach (Control control in controls)
        {
            Apply(control);
            if (control.HasChildren)
                Apply(control.Controls);
        }
    }

    private static void Apply(Control control)
    {
        switch (control)
        {
            case DataGridView grid:
                StyleGrid(grid);
                break;
            case ListView list:
                list.BackColor = Surface;
                list.ForeColor = TextPrimary;
                list.BorderStyle = BorderStyle.FixedSingle;
                break;
            case ListBox list:
                list.BackColor = Surface;
                list.ForeColor = TextPrimary;
                list.BorderStyle = BorderStyle.FixedSingle;
                break;
            case TextBoxBase text:
                text.BackColor = SurfaceRaised;
                text.ForeColor = TextPrimary;
                text.BorderStyle = BorderStyle.FixedSingle;
                break;
            case NumericUpDown numeric:
                numeric.BackColor = SurfaceRaised;
                numeric.ForeColor = TextPrimary;
                numeric.BorderStyle = BorderStyle.FixedSingle;
                break;
            case ComboBox combo:
                combo.BackColor = SurfaceRaised;
                combo.ForeColor = TextPrimary;
                combo.FlatStyle = FlatStyle.Flat;
                break;
            case Button button:
                StyleSecondaryButton(button);
                break;
            case CheckBox check:
                check.ForeColor = TextSecondary;
                check.FlatStyle = FlatStyle.Flat;
                break;
            case RadioButton radio:
                radio.ForeColor = TextSecondary;
                radio.FlatStyle = FlatStyle.Flat;
                break;
            case TabControl tabs:
                tabs.BackColor = Window;
                tabs.ForeColor = TextPrimary;
                break;
            case StatusStrip status:
                status.BackColor = Surface;
                status.ForeColor = TextSecondary;
                status.SizingGrip = false;
                break;
            case ToolStrip strip:
                strip.BackColor = Surface;
                strip.ForeColor = TextPrimary;
                strip.Renderer = new ToolStripProfessionalRenderer(new ThemeColorTable());
                break;
            case GroupBox group:
                group.ForeColor = TextPrimary;
                break;
        }
    }

    public static void StylePrimaryButton(Button button)
    {
        StyleButtonBase(button);
        button.BackColor = Accent;
        button.ForeColor = Color.FromArgb(27, 20, 15);
        button.FlatAppearance.BorderColor = Accent;
        button.FlatAppearance.MouseOverBackColor = AccentHover;
        button.FlatAppearance.MouseDownBackColor = AccentPressed;
    }

    public static void StyleSecondaryButton(Button button)
    {
        StyleButtonBase(button);
        button.BackColor = SurfaceRaised;
        button.ForeColor = TextPrimary;
        button.FlatAppearance.BorderColor = Border;
        button.FlatAppearance.MouseOverBackColor = SurfaceHover;
        button.FlatAppearance.MouseDownBackColor = Surface;
    }

    public static void StyleDangerButton(Button button)
    {
        StyleSecondaryButton(button);
        button.ForeColor = Danger;
        button.FlatAppearance.BorderColor = Color.FromArgb(104, 55, 57);
        button.FlatAppearance.MouseOverBackColor = Color.FromArgb(65, 35, 38);
        button.FlatAppearance.MouseDownBackColor = Color.FromArgb(53, 29, 32);
    }

    public static void StyleLanguageButton(Button button, bool selected)
    {
        StyleSecondaryButton(button);
        button.BackColor = selected ? AccentSoft : Surface;
        button.ForeColor = selected ? AccentHover : TextMuted;
        button.FlatAppearance.BorderColor = selected ? Accent : Border;
    }

    private static void StyleButtonBase(Button button)
    {
        button.FlatStyle = FlatStyle.Flat;
        button.FlatAppearance.BorderSize = 1;
        button.Cursor = button.Enabled ? Cursors.Hand : Cursors.Default;
        button.UseVisualStyleBackColor = false;
    }

    public static void StyleGrid(DataGridView grid)
    {
        grid.EnableHeadersVisualStyles = false;
        grid.BackgroundColor = Window;
        grid.BorderStyle = BorderStyle.None;
        grid.GridColor = Border;
        grid.ColumnHeadersBorderStyle = DataGridViewHeaderBorderStyle.Single;
        grid.ColumnHeadersDefaultCellStyle.BackColor = SurfaceRaised;
        grid.ColumnHeadersDefaultCellStyle.ForeColor = TextSecondary;
        grid.ColumnHeadersDefaultCellStyle.SelectionBackColor = SurfaceRaised;
        grid.ColumnHeadersDefaultCellStyle.SelectionForeColor = TextPrimary;
        grid.DefaultCellStyle.BackColor = Surface;
        grid.DefaultCellStyle.ForeColor = TextPrimary;
        grid.DefaultCellStyle.SelectionBackColor = Selection;
        grid.DefaultCellStyle.SelectionForeColor = TextPrimary;
        grid.AlternatingRowsDefaultCellStyle.BackColor = Color.FromArgb(27, 30, 35);
        grid.RowHeadersDefaultCellStyle.BackColor = SurfaceRaised;
        grid.RowHeadersDefaultCellStyle.ForeColor = TextMuted;
    }

    private sealed class ThemeColorTable : ProfessionalColorTable
    {
        public override Color ToolStripGradientBegin => Surface;
        public override Color ToolStripGradientMiddle => Surface;
        public override Color ToolStripGradientEnd => Surface;
        public override Color ToolStripBorder => Border;
        public override Color MenuStripGradientBegin => Surface;
        public override Color MenuStripGradientEnd => Surface;
        public override Color ToolStripDropDownBackground => SurfaceRaised;
        public override Color ImageMarginGradientBegin => SurfaceRaised;
        public override Color ImageMarginGradientMiddle => SurfaceRaised;
        public override Color ImageMarginGradientEnd => SurfaceRaised;
        public override Color MenuBorder => BorderStrong;
        public override Color MenuItemBorder => Accent;
        public override Color MenuItemSelected => AccentSoft;
        public override Color MenuItemSelectedGradientBegin => AccentSoft;
        public override Color MenuItemSelectedGradientEnd => AccentSoft;
        public override Color ButtonSelectedBorder => Accent;
        public override Color ButtonSelectedGradientBegin => AccentSoft;
        public override Color ButtonSelectedGradientMiddle => AccentSoft;
        public override Color ButtonSelectedGradientEnd => AccentSoft;
        public override Color ButtonPressedBorder => AccentPressed;
        public override Color ButtonPressedGradientBegin => Selection;
        public override Color ButtonPressedGradientMiddle => Selection;
        public override Color ButtonPressedGradientEnd => Selection;
        public override Color SeparatorDark => Border;
        public override Color SeparatorLight => SurfaceRaised;
    }
}
