using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Cinematics;
using static AgainstRomeModifier.Tests.DialogControlTestSupport;

namespace AgainstRomeModifier.Tests;

public sealed class StartCameraDialogTests
{
    [Fact]
    public void Dialog_InitializesWithDefaults_AndAcceptsValues() => InSta(() =>
    {
        using var dialog = new StartCameraDialog();
        Assert.Equal(AutoScaleMode.Dpi, dialog.AutoScaleMode);
        Assert.True(dialog.Camera.Enabled);
        Assert.Equal(8192.0, dialog.Camera.X);
        Assert.Equal(8192.0, dialog.Camera.Z);
        Assert.Equal(0.0, dialog.Camera.Y);
        Assert.Equal(5.0, dialog.Camera.Zoom);
        Assert.Equal(StartCameraPrimitive.CenterToPos, dialog.Camera.Primitive);

        ShowOffscreen(dialog);
        ((Button)dialog.AcceptButton!).PerformClick();
        Assert.Equal(DialogResult.OK, dialog.DialogResult);
    });

    [Fact]
    public void Dialog_WithCustomInitialCamera_LoadsParametersCorrectly() => InSta(() =>
    {
        var initial = new MapStartCamera
        {
            Enabled = true,
            X = 12512,
            Y = 15,
            Z = 14816,
            Zoom = 7.5,
            Primitive = StartCameraPrimitive.DirectEnginePos
        };

        using var dialog = new StartCameraDialog(initial);
        Assert.Equal(12512, dialog.Camera.X);
        Assert.Equal(15, dialog.Camera.Y);
        Assert.Equal(14816, dialog.Camera.Z);
        Assert.Equal(7.5, dialog.Camera.Zoom);
        Assert.Equal(StartCameraPrimitive.DirectEnginePos, dialog.Camera.Primitive);
    });

    [Fact]
    public void Dialog_CaptureCurrentView_UpdatesCoordinatesFromProvider() => InSta(() =>
    {
        using var dialog = new StartCameraDialog
        {
            ViewCaptureProvider = () => (4096.0, 25.0, 8192.0, 6.0)
        };
        ShowOffscreen(dialog);
        dialog.CaptureFromCurrentView();

        Assert.Equal(4096.0, dialog.Camera.X);
        Assert.Equal(25.0, dialog.Camera.Y);
        Assert.Equal(8192.0, dialog.Camera.Z);
        Assert.Equal(6.0, dialog.Camera.Zoom);
    });

    [Fact]
    public void Dialog_Cancel_SetsDialogResultCancel() => InSta(() =>
    {
        using var dialog = new StartCameraDialog();
        ShowOffscreen(dialog);
        ((Button)dialog.CancelButton!).PerformClick();
        Assert.Equal(DialogResult.Cancel, dialog.DialogResult);
    });
}
