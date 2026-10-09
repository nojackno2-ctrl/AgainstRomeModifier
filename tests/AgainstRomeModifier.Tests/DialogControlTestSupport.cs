using System.Drawing;
using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace AgainstRomeModifier.Tests;

internal static class DialogControlTestSupport
{
    internal static void InSta(Action action)
    {
        Exception? failure = null;
        var thread = new Thread(() =>
        {
            try { action(); }
            catch (Exception ex) { failure = ex; }
        }) { IsBackground = true };
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.True(thread.Join(TimeSpan.FromSeconds(30)), "STA test timed out");
        if (failure is not null) ExceptionDispatchInfo.Capture(failure).Throw();
    }

    internal static IEnumerable<T> Descendants<T>(Control parent) where T : Control
    {
        foreach (Control child in parent.Controls)
        {
            if (child is T match) yield return match;
            foreach (T descendant in Descendants<T>(child)) yield return descendant;
        }
    }

    internal static void ShowOffscreen(Form form)
    {
        form.ShowInTaskbar = false;
        form.StartPosition = FormStartPosition.Manual;
        form.Location = new Point(-20000, -20000);
        form.Show();
        Application.DoEvents();
    }

    internal static void PressKey(TextBox input, Keys key)
    {
        // Deliver real WinForms KeyDown events without accessing private handlers.
        SendMessage(input.Handle, 0x0100 /* WM_KEYDOWN */, (nint)key, (nint)1);
        SendMessage(input.Handle, 0x0101 /* WM_KEYUP */, (nint)key, unchecked((nint)0xC0000001L));
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern nint SendMessage(nint hWnd, uint message, nint wParam, nint lParam);
}

