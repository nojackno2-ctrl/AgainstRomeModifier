using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Theory]
    [InlineData("_title")]
    [InlineData("_subtitle")]
    [InlineData("_briefing")]
    public void Unsupported_text_rejects_save_preserves_all_files_and_can_retry(string field)
    {
        string map = CreateFixture();
        var initial = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Text", "Endless"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            GetField<TextBox>(form, field).Text = "中文";
            Assert.Contains("CP1251", GetField<ErrorProvider>(form, "_gameTextErrors").GetError(GetField<TextBox>(form, field)));
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.False(form.TrySaveMap(false, out Exception? error));
            Assert.Contains("CP1251", Assert.IsType<ArgumentException>(error).Message);
            Assert.Equal("中文", GetField<TextBox>(form, field).Text);
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            var rejected = SnapshotDirectory(map);
            Assert.Equal(initial.Keys.Order(), rejected.Keys.Order());
            foreach (var file in initial) Assert.Equal(file.Value, rejected[file.Key]);
            GetField<TextBox>(form, field).Text = "Valid text";
            Assert.Empty(GetField<ErrorProvider>(form, "_gameTextErrors").GetError(GetField<TextBox>(form, field)));
            Assert.True(form.TrySaveMap(false, out Exception? retryError), retryError?.ToString());
            Assert.Null(retryError);
            Assert.False(GetProperty<bool>(form, "IsDirty"));
        });
    }

    [Fact]
    public void Titles_teams_and_briefing_escaped_text_save_reopen_and_second_save_are_consistent()
    {
        string map = CreateFixture();
        string title = "Map \"Alpha\" C:\\new\\";
        string briefing = "First \"line\"\\path\r\nSecond line\tend";
        RunInSta(() =>
        {
            using (var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Text", "Endless")))
            {
                _ = form.Handle;
                Invoke(form, "LoadSelectedMap");
                GetField<TextBox>(form, "_title").Text = title;
                GetField<TextBox>(form, "_subtitle").Text = title;
                GetField<TextBox>(form, "_briefing").Text = briefing;
                foreach (var team in GetField<TextBox[]>(form, "_teamNames")) team.Text = title;
                Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
                Assert.Null(error);
            }
            var initial = SnapshotDirectory(map);
            using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Text", "Endless"));
            _ = reopened.Handle;
            Invoke(reopened, "LoadSelectedMap");
            Assert.Equal(title, GetField<TextBox>(reopened, "_title").Text);
            Assert.Equal(title, GetField<TextBox>(reopened, "_subtitle").Text);
            Assert.Equal(briefing, GetField<TextBox>(reopened, "_briefing").Text);
            Assert.All(GetField<TextBox[]>(reopened, "_teamNames"), team => Assert.Equal(title, team.Text));
            Assert.False(GetProperty<bool>(reopened, "IsDirty"));
            Assert.True(reopened.TrySaveMap(false, out Exception? retryError), retryError?.ToString());
            Assert.Null(retryError);
            var current = SnapshotDirectory(map);
            Assert.Equal(initial.Keys.Order(), current.Keys.Order());
            foreach (var file in initial) Assert.Equal(file.Value, current[file.Key]);
        });
    }
}
