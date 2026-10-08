namespace AgainstRomeMapEditor.Modules.Nature;

/// <summary>
/// 《反抗羅馬》(Against Rome) 歷史戰區與地理生態圈類型。
/// 定義特定區域的地貌特徵、氣候環境與植物群落組成。
/// </summary>
public enum BiomeType
{
    /// <summary>
    /// 日耳曼原始森林（Germanic Coniferous &amp; Mixed Forest - "Ger"）：
    /// 歐陸高緯度幽深黑森林，高大鬱閉冷杉、雲杉、松樹，林下耐陰灌木、地衣野花與花崗岩石塊。
    /// </summary>
    GermanicForest = 0,

    /// <summary>
    /// 多瑙河溫帶闊葉林（Danubian Temperate Broadleaf Forest - "Kel"/"Ger"）：
    /// 中歐大河流域沖積平原與溫暖丘陵，古老巨櫟、山毛櫸、闊葉林，繁茂灌木與繽紛原野草花。
    /// </summary>
    DanubeBroadleaf = 1,

    /// <summary>
    /// 義大利地中海灌木丘陵（Italian Mediterranean Scrub Hills - "Ita"/"Rom"）：
    /// 乾燥陽光丘陵，耐旱常綠硬葉林（馬基群落 Maquis），絲柏、石松，低矮刺灌木與裸露石灰岩石。
    /// </summary>
    ItalianHills = 2,

    /// <summary>
    /// 不列顛沼澤荒原（British Bog &amp; Heathland - "Bri"/"Kel"）：
    /// 潮濕陰冷石楠荒原與泥炭沼澤，喬木稀疏，低矮石楠灌叢、金雀花、沼澤莎草蘆葦與荒原巨石。
    /// </summary>
    BritishMarsh = 3,
}

/// <summary>
/// 植被生態垂直分層與生境類別。
/// 用於模擬自然界的林冠結構與微地形生境適配。
/// </summary>
public enum VegetationLayer
{
    /// <summary>喬木層（林冠）：深林核心的高大喬木與次冠層樹木。</summary>
    Canopy = 0,

    /// <summary>灌木層（林下/林緣）：低矮灌木、荊棘、樹籬。</summary>
    Understory = 1,

    /// <summary>地表草花層（林窗/原野）：原野野花、草叢、地衣。</summary>
    GroundFlora = 2,

    /// <summary>水岸生境層（濱水帶）：水際線蘆葦、浮萍、水草、垂柳。</summary>
    Riparian = 3,

    /// <summary>地質岩石層（坡地/懸崖）：碎石、卵石、沉積岩、巨石。</summary>
    Rock = 4,
}
