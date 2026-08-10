using System.Reflection;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// Loc.Get 找不到 key 時會回傳 key 本身，缺漏因此是「靜默錯誤」——畫面上出現
/// 一串英數 id 而不是文字，測試不看的話沒有人會發現。這裡把兩份字典的 key 集合鎖住。
/// </summary>
public sealed class LocalizationParityTests
{
    [Fact]
    public void English_and_chinese_dictionaries_expose_the_same_keys()
    {
        var en = GetDictionary("En");
        var zh = GetDictionary("Zh");

        string missingInZh = string.Join(", ", en.Keys.Except(zh.Keys).OrderBy(x => x));
        string missingInEn = string.Join(", ", zh.Keys.Except(en.Keys).OrderBy(x => x));

        Assert.True(missingInZh.Length == 0, "Zh 缺少的 key: " + missingInZh);
        Assert.True(missingInEn.Length == 0, "En 缺少的 key: " + missingInEn);
    }

    [Fact]
    public void No_translation_value_is_empty()
    {
        foreach (string name in new[] { "En", "Zh" })
        {
            foreach (var (key, value) in GetDictionary(name))
            {
                Assert.False(string.IsNullOrWhiteSpace(value), $"{name}[{key}] 是空字串。");
            }
        }
    }

    private static Dictionary<string, string> GetDictionary(string fieldName)
    {
        FieldInfo field = typeof(Loc).GetField(fieldName, BindingFlags.Static | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException($"找不到 Loc.{fieldName}。");
        return (Dictionary<string, string>)field.GetValue(null)!;
    }
}
