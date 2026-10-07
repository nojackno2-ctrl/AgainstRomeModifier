namespace AgainstRomeMapEditor.Modules;

/// <summary>模組只管理編輯狀態；載入檔案及存檔交易由宿主負責。</summary>
public interface IEditorModule<TSnapshot>
{
    string ModuleId { get; }
    bool IsDirty { get; }
    void Load(TSnapshot snapshot);
    TSnapshot Capture();
    void AcceptChanges();
    void Reset();
}
