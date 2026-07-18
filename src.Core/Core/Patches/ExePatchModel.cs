namespace AgainstRomeModifier.Core.Patches;

/// <summary>Against_Rome.exe 失焦暫停相容性補丁狀態。</summary>
public enum ExePatchState {
    Unknown,
    Original,
    FocusPatched
}

/// <summary>法術免祭壇需求補丁狀態。</summary>
public enum ExeSpellAltarPatchState {
    Unknown,
    Original,
    Patched
}

/// <summary>無盡模式部族選擇強制為羅馬的補丁狀態。</summary>
public enum ExeRomanEndlessPatchState {
    Unknown,
    Original,
    Patched
}

/// <summary>住宅帳篷 ♂/♀ 生產鈕「點一次 +N」補丁狀態。</summary>
public enum ExeCiviProduce20PatchState {
    Unknown,
    Original,
    Patched
}

/// <summary>招募/裝備面板「點一次選取數直接跳到上限」補丁狀態。</summary>
public enum ExeUnitRecruit20PatchState {
    Unknown,
    Original,
    Patched
}

/// <summary>IGM「選取閒置村民」鈕一次選取上限 40 → 999 補丁狀態。</summary>
public enum ExeIdleSelect999PatchState {
    Unknown,
    Original,
    Patched
}

/// <summary>已被實機否定、僅供遷移還原的 getter-only v1 狀態。</summary>
public enum ExeDefaultSpecialArrowsPatchState {
    Unknown,
    Original,
    Patched
}

/// <summary>1600x1200 32-bit 原生顯示模式替換為 1920x1080 的狀態。</summary>
public enum ExeNativeWidescreenPatchState {
    Unknown,
    Original,
    LegacyUnforced,
    LegacyForcedStaleUi,
    Patched
}

/// <summary>Persistent camera zoom setters clamped to the first effective native zoom step.</summary>
public enum ExeCameraZoomOutPatchState {
    Unknown,
    Original,
    LegacyZoomHalf,
    Patched
}

/// <summary>已淘汰的村落建造範圍候選補丁（僅用於偵測與還原舊寫入）狀態。</summary>
public enum ExeVillageRangePatchState {
    Unknown,
    Original,
    LegacyLogicOnly,
    Expanded
}

/// <summary>村落建造範圍 setter 跳板補丁狀態。</summary>
public enum ExeVillageSetterPatchState {
    Unknown,
    Original,
    Legacy2x,
    Legacy2Point5x,
    Legacy3x,
    Legacy5x,
    EntireMap
}

/// <summary>單一固定偏移寫入計畫：只有目前位元組等於 <see cref="Expected"/> 時才允許覆寫為 <see cref="Replacement"/>。</summary>
public readonly record struct ExeWriteOp(long Offset, byte[] Expected, byte[] Replacement, string PatchName);

/// <summary>
/// Against_Rome.exe 固定偏移補丁的純資料模型：集中偵測目前狀態，並依狀態規劃
/// 「預期位元組 → 取代位元組」的寫入清單。所有邏輯不依賴 UI，可單獨測試；
/// 呼叫端（<c>ModifierForm</c>）只負責日誌、在地化與 exeModified 旗標。
/// </summary>
public static class ExePatchModel {
    // === 失焦暫停相容性 ===
    public static readonly byte[] FocusOriginalBytes = { 0x89, 0x15, 0xC4, 0x7D, 0x9E, 0x02 };
    public static readonly byte[] FocusPatchedBytes = { 0x90, 0x90, 0x90, 0x90, 0x90, 0x90 };
    public const long FocusPatchOffset = 0x161a88;
    public const long FocusPatchRequiredLength = 0x161a8e;

    // === 無盡模式 dlg_volk 部族選擇強制為羅馬 ===
    public const long RomanEndlessPatchOffset = 0x5bd60;
    public static readonly byte[] RomanEndlessOriginalBytes = {
        0x53, 0x8B, 0x5C, 0x24, 0x08, 0x53, 0xE8, 0x25, 0x3B, 0xFE, 0xFF,
        0x83, 0xC4, 0x04, 0x53, 0x89, 0x1D, 0x78, 0x74, 0x73, 0x00
    };
    public static readonly byte[] RomanEndlessPatchedBytes = {
        0x53, 0x6A, 0x03, 0x5B, 0x90, 0x53, 0xE8, 0x25, 0x3B, 0xFE, 0xFF,
        0x83, 0xC4, 0x04, 0x53, 0x89, 0x1D, 0x78, 0x74, 0x73, 0x00
    };

    // === 住宅帳篷 ♂/♀ 生產鈕：點一次 +1 → +20 ===
    // 玩家點住宅帳篷的男/女生產鈕 → EXE 處理器 0x44FBED，內部以 push 1 當作
    // 「本次要加入未出生佇列的數量」，經 addUnborn 轉發器(0x4211B0)→worker(0x517AD0)
    // 寫入未出生男/女計數（worker 會自動夾到剩餘居住容量）。此路徑玩家專屬，AI 不經過。
    // 特徵含尾端 call 0x4211B0 的相對位移(e8 90 15 fd ff)，以與相鄰、幾乎相同的「減少」
    // 處理器(call 0x4211E0)區分；僅改 `6A 01` 的運算元位元組（索引 14）為 `6A 14`。
    public const long CiviProduce20PatchOffset = 0x4FC00;
    public static readonly byte[] CiviProduce20OriginalBytes = {
        0x83, 0x7C, 0x24, 0x18, 0x01, 0x0F, 0x95, 0xC0, 0x25, 0xFF, 0x00, 0x00, 0x00,
        0x6A, 0x01, 0x40, 0x50, 0x8B, 0x74, 0x24, 0x0C, 0x56, 0x8B, 0x7C, 0x24, 0x0C, 0x57,
        0xE8, 0x90, 0x15, 0xFD, 0xFF
    };
    public static readonly byte[] CiviProduce20PatchedBytes = {
        0x83, 0x7C, 0x24, 0x18, 0x01, 0x0F, 0x95, 0xC0, 0x25, 0xFF, 0x00, 0x00, 0x00,
        0x6A, 0x14, 0x40, 0x50, 0x8B, 0x74, 0x24, 0x0C, 0x56, 0x8B, 0x7C, 0x24, 0x0C, 0x57,
        0xE8, 0x90, 0x15, 0xFD, 0xFF
    };

    // === 招募/裝備面板：點一次選取數直接到上限 20 ===
    // 玩家點兵種/裝備頭像 → EXE 點擊處理器 0x44C7CB..0x44C7E7：
    //   mov edi,1 ; mov ebp,[0x722F14] ; mov esi,[item*20+0x722E28]
    //   sub ebp,edi ; add esi,edi ; mov [0x722F14],ebp ; mov [item*20+0x722E28],esi
    // 上限檢查在前（ebp=0x14=20，count>=20 就跳過），故本補丁把「count+1」改為「count=20」。
    // 只改 4 個位元組：`sub ebp,edi; add esi,edi`(29 FD 01 FE) → `push 20; pop esi; nop`(6A 14 5E 90)。
    // edi(=1) 保留供迴圈旗標；ebp 原值寫回（資源池不變，無副作用）；esi=20 直接寫入選取數。
    // 已於執行中的遊戲記憶體即時驗證：點一下選取數直接跳 20，且受既有上限檢查保護不會超過。
    public const long UnitRecruit20PatchOffset = 0x4C7DD;
    public static readonly byte[] UnitRecruit20OriginalBytes = { 0x29, 0xFD, 0x01, 0xFE };
    public static readonly byte[] UnitRecruit20PatchedBytes = { 0x6A, 0x14, 0x5E, 0x90 };

    // === IGM「選取閒置村民」鈕：一次選取上限 40 → 999 ===
    // 按鈕 igm_select_idle 的處理器（分派點 VA 0x44138B）呼叫 0x44D110（全部取消選取）
    // 後進入 VA 0x451DC0：以兩個各 40 格的堆疊陣列 + 字面值 push 0x28 呼叫收集器
    // 0x421820（→過濾器 0x538320 → 搜尋 0x5388A0 填入全域 scratch 清單，上限 1000），
    // 再逐格以 0x421130 還原物件索引並呼叫 0x44D5A0 加入主選取清單。
    // 上限鏈：按鈕 40 → 搜尋 scratch 1000（405 個程式碼引用，不可搬移）→
    // 主選取清單 999（0x7286E8 計數緊貼 1000 格陣列 0x727748，結構固定）。
    // 因此本補丁把整個函式區域（0x451DC0..0x451E6F，含對齊填充共 0xB0 位元組）
    // 改寫為 N=999 版本：堆疊框架 0x140 → 0x1F38、緩衝 0xA0 → 0xF9C、push 0x28 →
    // push 0x3E7；初始化迴圈改為由高位址往低位址遞減寫入，順帶完成 8KB 框架的
    // stack guard-page 探測。結構與原版逐指令對應，五個 call 目標不變（rel32 依
    // 位移重算）。0x451DC0 僅有兩個呼叫端（0x42347A 熱鍵、0x441398 按鈕），皆呼叫
    // 函式入口，無跳入函式中段之處，整段改寫安全。1600 不可達：搜尋 scratch 與
    // 主選取清單皆為緊鄰全域的固定容量陣列，擴充需搬移重定位，超出安全補丁範圍。
    public const long IdleSelect999PatchOffset = 0x51DC0;
    public static readonly byte[] IdleSelect999OriginalBytes = Convert.FromHexString(
        "53565781EC40010000BAFFFFFFFF31DB31F683C30489541CFC81FBA000000075F1" +
        "6A036A016A016A006A01E800B1FFFF506A288D44241C508D8424C00000005031DB" +
        "E819FAFCFF83C4248B0C1C518BBC1CA400000057E815F3FCFF83C40883F8FF7519" +
        "83C30481FBA000000075DC85F6751A81C4400100005F5E5BC350BE01000000E859" +
        "B7FFFF83C404EBD76A7CE8EDBCFEFF83C40481C4400100005F5E5BC3" +
        "8D80000000008D92000000008D442000");
    public static readonly byte[] IdleSelect999PatchedBytes = Convert.FromHexString(
        "53565781EC381F0000BAFFFFFFFF31F6BB9C0F000083EB0489141C75F8" +
        "6A036A016A016A006A01E804B1FFFF5068E70300008D44241C508D8424BC0F0000" +
        "5031DBE81AFAFCFF83C4248B0C1C518BBC1CA00F000057E816F3FCFF83C40883F8" +
        "FF751983C30481FB9C0F000075DC85F6751A81C4381F00005F5E5BC350BE010000" +
        "00E85AB7FFFF83C404EBD76A7CE8EEBCFEFF83C40481C4381F00005F5E5BC3" +
        "9090909090909090909090909090909090");

    // === 已解鎖特殊箭矢預設（runtime-rejected getter-only v1；restore-only）===
    // 原版把遠程模式放在 BSS 0x00736A58：0=普通、1=火箭/毒箭、2=掠奪。
    // 舊設計因 BSS 無檔案內容而把 getter 導向相鄰的 22-byte 對齊區：
    // raw 0（從未選擇）=> 1，raw 3（普通箭按鈕 sentinel）=> 0，raw 1/2 原樣回傳。
    // 使用者實測仍發射普通箭；現只保留 bytes 以辨識舊安裝並還原。
    public const long DefaultSpecialArrowsNormalButtonOffset = 0x413FC;
    public const long DefaultSpecialArrowsGetterOffset = 0x4E980;
    public const long DefaultSpecialArrowsCaveOffset = 0x4E96A;
    public static readonly byte[] DefaultSpecialArrowsNormalButtonOriginalBytes = { 0x00 };
    public static readonly byte[] DefaultSpecialArrowsNormalButtonPatchedBytes = { 0x03 };
    public static readonly byte[] DefaultSpecialArrowsGetterOriginalBytes = { 0xA1, 0x58, 0x6A, 0x73, 0x00, 0xC3 };
    public static readonly byte[] DefaultSpecialArrowsGetterPatchedBytes = { 0xE9, 0xE5, 0xFF, 0xFF, 0xFF, 0xC3 };
    public static readonly byte[] DefaultSpecialArrowsCaveOriginalBytes = {
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x8D, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x8D, 0x92, 0x00, 0x00, 0x00, 0x00,
        0x8D, 0x44, 0x20, 0x00
    };
    public static readonly byte[] DefaultSpecialArrowsCavePatchedBytes = {
        0xA1, 0x58, 0x6A, 0x73, 0x00, // mov eax,[0x00736A58]
        0x83, 0xF8, 0x03,             // cmp eax,3
        0x75, 0x03,                   // jne test_default
        0x31, 0xC0, 0xC3,             // xor eax,eax; ret
        0x85, 0xC0,                   // test eax,eax
        0x75, 0x01,                   // jne return
        0x40,                         // inc eax
        0xC3,                         // ret
        0x90, 0x90, 0x90
    };

    // === 原生 1920x1080 viewport（取代 1600x1200 32-bit mode 0x22）===
    // VA 0x424590 會把目前顯示寬高辨識回 mode ID；VA 0x424760 建立並刷新顯示模式。
    // IGM UI 仍沿用原版 igm16001200 資源，故此功能必須保持 Experimental，待實機驗證 UI 與滑鼠座標。
    public const long NativeWidescreenIdentifyOffset = 0x246B3;
    public const long NativeWidescreenCreateOffset = 0x249AA;
    public const long NativeWidescreenActiveModeGetterOffset = 0x24B80;
    public const long NativeWidescreenForceModeOffset = 0x24BD0;
    public const long NativeWidescreenIgmDialogModeOffset = 0x41E25;
    public const long NativeWidescreenModeTextOffset = 0x1DCC76;
    public static readonly byte[] NativeWidescreenIdentifyOriginalBytes = {
        0x81, 0x3E, 0x40, 0x06, 0x00, 0x00, 0x75, 0x12,
        0x81, 0x3B, 0xB0, 0x04, 0x00, 0x00, 0x75, 0x0A,
        0x83, 0xFD, 0x20, 0x75, 0x05, 0xBF, 0x22, 0x00, 0x00, 0x00
    };
    public static readonly byte[] NativeWidescreenIdentifyPatchedBytes = {
        0x81, 0x3E, 0x80, 0x07, 0x00, 0x00, 0x75, 0x12,
        0x81, 0x3B, 0x38, 0x04, 0x00, 0x00, 0x75, 0x0A,
        0x83, 0xFD, 0x20, 0x75, 0x05, 0xBF, 0x22, 0x00, 0x00, 0x00
    };
    public static readonly byte[] NativeWidescreenCreateOriginalBytes = {
        0x6A, 0x50, 0x6A, 0x20,
        0x68, 0xB0, 0x04, 0x00, 0x00, 0x68, 0x40, 0x06, 0x00, 0x00,
        0xE8, 0x83, 0x96, 0x15, 0x00, 0x83, 0xC4, 0x10, 0x85, 0xC0,
        0x0F, 0x85, 0x01, 0xFE, 0xFF, 0xFF,
        0x68, 0xB0, 0x04, 0x00, 0x00, 0x68, 0x40, 0x06, 0x00, 0x00,
        0xE8, 0xD9, 0x96, 0x15, 0x00, 0x83, 0xC4, 0x08
    };
    public static readonly byte[] NativeWidescreenCreatePatchedBytes = {
        0x6A, 0x50, 0x6A, 0x20,
        0x68, 0x38, 0x04, 0x00, 0x00, 0x68, 0x80, 0x07, 0x00, 0x00,
        0xE8, 0x83, 0x96, 0x15, 0x00, 0x83, 0xC4, 0x10, 0x85, 0xC0,
        0x0F, 0x85, 0x01, 0xFE, 0xFF, 0xFF,
        0x68, 0x38, 0x04, 0x00, 0x00, 0x68, 0x80, 0x07, 0x00, 0x00,
        0xE8, 0xD9, 0x96, 0x15, 0x00, 0x83, 0xC4, 0x08
    };
    public static readonly byte[] NativeWidescreenModeTextOriginalBytes =
        System.Text.Encoding.ASCII.GetBytes("Modus 1600x1200 32bit\n\0");
    public static readonly byte[] NativeWidescreenModeTextPatchedBytes =
        System.Text.Encoding.ASCII.GetBytes("Modus 1920x1080 32bit\n\0");
    public static readonly byte[] NativeWidescreenForceModeOriginalBytes = {
        0x8B, 0x15, 0xB0, 0x80, 0x65, 0x00, 0x52
    };
    public static readonly byte[] NativeWidescreenForceModePatchedBytes = {
        0x6A, 0x22, 0x90, 0x90, 0x90, 0x90, 0x90
    };
    public static readonly byte[] NativeWidescreenActiveModeGetterOriginalBytes = {
        0xA1, 0xB0, 0x80, 0x65, 0x00, 0xC3
    };
    public static readonly byte[] NativeWidescreenActiveModeGetterPatchedBytes = {
        0xA1, 0xBC, 0x80, 0x65, 0x00, 0xC3
    };
    public static readonly byte[] NativeWidescreenIgmDialogModeOriginalBytes = {
        0x83, 0xF8, 0x21, 0x74, 0xD4
    };
    public static readonly byte[] NativeWidescreenIgmDialogModePatchedBytes = {
        0x83, 0xF8, 0x22, 0x74, 0xD4
    };

    // === Camera zoom-out +1 (first runtime-effective native zoom step) ===
    // Redirect only persistent setters (startup, save-load and mission script) through
    // a shared code cave. Internal temporary zoom calls remain untouched.
    public const long CameraZoomInitCallOffset = 0x81388;
    public const long CameraZoomLoadCallOffset = 0x8D880;
    public const long CameraZoomScriptCallOffset = 0x14C1FA;
    public const long CameraZoomCaveOffset = 0x1625C0;
    public static readonly byte[] CameraZoomInitCallOriginalBytes = { 0xE8, 0xA3, 0x76, 0x01, 0x00 };
    public static readonly byte[] CameraZoomInitCallPatchedBytes = { 0xE8, 0x33, 0x12, 0x0E, 0x00 };
    public static readonly byte[] CameraZoomLoadCallOriginalBytes = { 0xE8, 0xAB, 0xB1, 0x00, 0x00 };
    public static readonly byte[] CameraZoomLoadCallPatchedBytes = { 0xE8, 0x3B, 0x4D, 0x0D, 0x00 };
    public static readonly byte[] CameraZoomScriptCallOriginalBytes = { 0xE8, 0x31, 0xC8, 0xF4, 0xFF };
    public static readonly byte[] CameraZoomScriptCallPatchedBytes = { 0xE8, 0xC1, 0x63, 0x01, 0x00 };
    public static readonly byte[] CameraZoomCaveOriginalBytes = new byte[28];
    public static readonly byte[] CameraZoomCaveLegacyZoomHalfBytes = {
        0x8B, 0x44, 0x24, 0x04,                         // mov eax,[esp+4]
        0x85, 0xC0,                                     // test eax,eax
        0x78, 0x07,                                     // js set_half
        0x3D, 0x00, 0x00, 0x00, 0x3F,                   // cmp eax,0.5f
        0x73, 0x08,                                     // jae jump_setter
        0xC7, 0x44, 0x24, 0x04, 0x00, 0x00, 0x00, 0x3F, // mov [esp+4],0.5f
        0xE9, 0x54, 0x64, 0xF3, 0xFF                    // jmp native setter
    };
    public static readonly byte[] CameraZoomCavePatchedBytes = {
        0x8B, 0x44, 0x24, 0x04,                         // mov eax,[esp+4]
        0x85, 0xC0,                                     // test eax,eax
        0x78, 0x07,                                     // js set_one
        0x3D, 0x00, 0x00, 0x80, 0x3F,                   // cmp eax,1.0f
        0x73, 0x08,                                     // jae jump_setter
        0xC7, 0x44, 0x24, 0x04, 0x00, 0x00, 0x80, 0x3F, // mov [esp+4],1.0f
        0xE9, 0x54, 0x64, 0xF3, 0xFF                    // jmp native setter
    };

    // === 法術免祭壇需求（各族群 12 處特徵）===
    public static readonly (long Offset, byte[] Original, byte[] Patched)[] SpellAltarPatchSites = new[] {
        // Germans
        (0x4A112L, new byte[] { 0x83, 0xFE, 0x01, 0x0F, 0x8C, 0x54, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x54, 0xFF, 0xFF, 0xFF }),
        (0x4A136L, new byte[] { 0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x58, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x58, 0xFF, 0xFF, 0xFF }),
        (0x4A15AL, new byte[] { 0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x5C, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x5C, 0xFF, 0xFF, 0xFF }),
        (0x4A0E3L, new byte[] { 0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x92, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x92, 0x00, 0x00, 0x00 }),

        // Celts
        (0x4A1CCL, new byte[] { 0x83, 0xFE, 0x01, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }),
        (0x4A293L, new byte[] { 0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }),
        (0x4A2B7L, new byte[] { 0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }),
        (0x4A249L, new byte[] { 0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }),

        // Huns
        (0x4A329L, new byte[] { 0x83, 0xFE, 0x01, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00 }),
        (0x4A3F0L, new byte[] { 0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF }),
        (0x4A414L, new byte[] { 0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF }),
        (0x4A3A6L, new byte[] { 0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }, new byte[] { 0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00 }),
    };

    // === 已淘汰的村落建造範圍候選（保留以偵測並還原舊寫入）===
    public static readonly byte[] VillageRangeXOriginalBytes = { 0xC1, 0xE2, 0x06 };
    public static readonly byte[] VillageRangeZOriginalBytes = { 0xC1, 0xE1, 0x06 };
    public static readonly byte[] VillageRangeXPatchedBytes = { 0xC1, 0xE2, 0x07 };
    public static readonly byte[] VillageRangeZPatchedBytes = { 0xC1, 0xE1, 0x07 };
    public static readonly byte[] VillageFrameXOriginalBytes = { 0xC1, 0xE6, 0x06 };
    public static readonly byte[] VillageFrameZOriginalBytes = { 0xC1, 0xE7, 0x06 };
    public static readonly byte[] VillageFrameXPatchedBytes = { 0xC1, 0xE6, 0x07 };
    public static readonly byte[] VillageFrameZPatchedBytes = { 0xC1, 0xE7, 0x07 };
    public const long VillageRangeXPatchOffset = 0x1366c4;
    public const long VillageRangeZPatchOffset = 0x1366cd;
    public const long VillageFrameXPatchOffset = 0x0d722c;
    public const long VillageFrameZPatchOffset = 0x0d723b;
    public const long VillageRangePatchRequiredLength = 0x1366d0;

    // === 村落建造範圍 setter 跳板 ===
    public static readonly byte[] VillageSetterHookOriginalBytes = {
        0x85, 0xF6, 0x7C, 0xA6, 0x85, 0xFF, 0x7C, 0xA2
    };
    public static readonly byte[] VillageSetterHookPatchedBytes = {
        0xE9, 0xC9, 0xC0, 0x02, 0x00, 0x90, 0x90, 0x90
    };
    public static readonly byte[] VillageSetterCaveOriginalBytes = new byte[39];
    // 舊版 modifier 安裝 33 位元組的 2x 跳板並留下 6 位元組零填補；保留辨識以便
    // Apply 能把已修補的執行檔遷移到目前的 2.5x 版本。
    public static readonly byte[] VillageSetterCaveLegacy2xBytes = {
        0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
        0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
        0xD1, 0xE6, 0xD1, 0xE7, 0x57, 0x56, 0x50, 0xE8,
        0x55, 0xE3, 0xF5, 0xFF, 0xE9, 0x21, 0x3F, 0xFD, 0xFF,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00
    };
    public static readonly byte[] VillageSetterCaveLegacy3xBytes = {
        0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
        0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
        0x8D, 0x34, 0x76, 0x90, 0x90,
        0x8D, 0x3C, 0x7F, 0x90, 0x90,
        0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
        0xE9, 0x1B, 0x3F, 0xFD, 0xFF
    };
    public static readonly byte[] VillageSetterCaveLegacy5xBytes = {
        0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
        0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
        0x8D, 0x34, 0xB6, 0x90, 0x90,
        0x8D, 0x3C, 0xBF, 0x90, 0x90,
        0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
        0xE9, 0x1B, 0x3F, 0xFD, 0xFF
    };
    public static readonly byte[] VillageSetterCavePatchedBytes = {
        0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
        0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
        0xBE, 0x30, 0x75, 0x00, 0x00, // mov esi, 30000 (0x7530)
        0xBF, 0x30, 0x75, 0x00, 0x00, // mov edi, 30000 (0x7530)
        0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
        0xE9, 0x1B, 0x3F, 0xFD, 0xFF
    };
    public static readonly byte[] VillageSetterCaveLegacy2Point5xBytes = {
        0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF,
        0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
        0x8D, 0x34, 0xB6, 0xD1, 0xEE,
        0x8D, 0x3C, 0xBF, 0xD1, 0xEF,
        0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3, 0xF5, 0xFF,
        0xE9, 0x1B, 0x3F, 0xFD, 0xFF
    };
    public const long VillageSetterHookOffset = 0x1364c1;
    public const long VillageSetterCaveOffset = 0x16258f;
    public const long VillageSetterPatchRequiredLength = 0x1625b6;

    // === 遊戲整體時脈加速（主時脈常數縮放）===
    // 主時脈函式（VA 0x55e530）回傳「奈秒級的當前時間(double)」，全遊戲模擬（移動／
    // 生產／戰鬥／AI）都透過它取得時間並以差值推進。它有兩條路徑，各用一個 rodata
    // double 常數換算單位：
    //   QPC 路徑（高精度計時器）：scale = 1e9 / freq，常數 1e9 位於 VA 0x604214／檔案偏移 0x204214
    //   timeGetTime 路徑（退回）：time = (elapsed_ms) * 1e6，常數 1e6 位於 VA 0x60424c／檔案偏移 0x20424c
    // 兩個常數各自「只被時脈碼引用一次、單一用途」（經 xref 確認），因此同步把兩者乘上
    // 相同倍率 s，即可讓整個遊戲時脈以 s 倍速前進——不論玩家機器走哪條路徑。差值運算在
    // 遊戲自身邏輯中先減去基準再乘常數，故無 32-bit 溢位風險，且時間仍由 ~0 開始、無跳變。
    // 完全可逆：把常數改回原值即還原。
    public const long GameSpeedQpcConstOffset = 0x204214; // 原版 double 1e9
    public const long GameSpeedTgtConstOffset = 0x20424c; // 原版 double 1e6
    private const double GameSpeedQpcBase = 1_000_000_000.0;
    private const double GameSpeedTgtBase = 1_000_000.0;
    /// <summary>工具提供的加速倍率選項（1 = 原版／關閉，最高 10 倍）。</summary>
    public static readonly int[] GameSpeedSupportedMultipliers = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 };

    /// <summary>
    /// 偵測目前時脈倍率：回傳 1（原版）／2／3／4…；若兩條路徑常數不一致、非整數倍或超出範圍，
    /// 回傳 0 表示 Unknown（呼叫端不應改寫）。
    /// </summary>
    public static int GetGameSpeedMultiplier(byte[] exeBytes) {
        if (exeBytes.Length < GameSpeedQpcConstOffset + 8 || exeBytes.Length < GameSpeedTgtConstOffset + 8) {
            return 0;
        }
        double qpc = BitConverter.ToDouble(exeBytes, (int)GameSpeedQpcConstOffset);
        double tgt = BitConverter.ToDouble(exeBytes, (int)GameSpeedTgtConstOffset);
        if (qpc <= 0 || tgt <= 0) return 0;
        double sQpc = qpc / GameSpeedQpcBase;
        double sTgt = tgt / GameSpeedTgtBase;
        int rq = (int)Math.Round(sQpc);
        int rt = (int)Math.Round(sTgt);
        if (rq != rt || rq < 1) return 0;
        if (Math.Abs(sQpc - rq) > 1e-6 || Math.Abs(sTgt - rt) > 1e-6) return 0;
        return rq;
    }

    /// <summary>
    /// 規劃把時脈倍率從 <paramref name="currentMultiplier"/> 切換為 <paramref name="desiredMultiplier"/>
    /// 的寫入清單。currentMultiplier 為 0（Unknown）時不動作；倍率相同時不動作。
    /// desiredMultiplier = 1 代表還原為原版。
    /// </summary>
    public static IReadOnlyList<ExeWriteOp> PlanGameSpeed(int desiredMultiplier, int currentMultiplier) {
        if (currentMultiplier <= 0) return Array.Empty<ExeWriteOp>();
        if (desiredMultiplier < 1) desiredMultiplier = 1;
        if (desiredMultiplier == currentMultiplier) return Array.Empty<ExeWriteOp>();

        byte[] expectedQpc = BitConverter.GetBytes(GameSpeedQpcBase * currentMultiplier);
        byte[] expectedTgt = BitConverter.GetBytes(GameSpeedTgtBase * currentMultiplier);
        byte[] newQpc = BitConverter.GetBytes(GameSpeedQpcBase * desiredMultiplier);
        byte[] newTgt = BitConverter.GetBytes(GameSpeedTgtBase * desiredMultiplier);
        string name = desiredMultiplier == 1 ? "遊戲加速還原" : $"遊戲加速 {desiredMultiplier}×";
        return new[] {
            new ExeWriteOp(GameSpeedQpcConstOffset, expectedQpc, newQpc, name + "（QPC 路徑）"),
            new ExeWriteOp(GameSpeedTgtConstOffset, expectedTgt, newTgt, name + "（timeGetTime 路徑）"),
        };
    }

    // === 狀態偵測 ===
    public static ExePatchState GetExePatchState(byte[] exeBytes) {
        if (exeBytes.Length < FocusPatchRequiredLength) {
            return ExePatchState.Unknown;
        }
        byte[] bytes = new byte[FocusOriginalBytes.Length];
        Buffer.BlockCopy(exeBytes, (int)FocusPatchOffset, bytes, 0, bytes.Length);
        if (bytes.SequenceEqual(FocusOriginalBytes)) return ExePatchState.Original;
        if (bytes.SequenceEqual(FocusPatchedBytes)) return ExePatchState.FocusPatched;
        return ExePatchState.Unknown;
    }

    public static ExeSpellAltarPatchState GetSpellAltarPatchState(byte[] exeBytes) {
        bool allOriginal = true;
        bool allPatched = true;

        foreach (var site in SpellAltarPatchSites) {
            if (exeBytes.Length < site.Offset + site.Original.Length) {
                return ExeSpellAltarPatchState.Unknown;
            }
            byte[] current = new byte[site.Original.Length];
            Buffer.BlockCopy(exeBytes, (int)site.Offset, current, 0, current.Length);

            if (!current.SequenceEqual(site.Original)) {
                allOriginal = false;
            }
            if (!current.SequenceEqual(site.Patched)) {
                allPatched = false;
            }
        }

        if (allOriginal) return ExeSpellAltarPatchState.Original;
        if (allPatched) return ExeSpellAltarPatchState.Patched;
        return ExeSpellAltarPatchState.Unknown;
    }

    public static ExeRomanEndlessPatchState GetRomanEndlessPatchState(byte[] exeBytes) {
        if (exeBytes.Length < RomanEndlessPatchOffset + RomanEndlessOriginalBytes.Length) {
            return ExeRomanEndlessPatchState.Unknown;
        }
        byte[] bytes = ReadSpan(exeBytes, RomanEndlessPatchOffset, RomanEndlessOriginalBytes.Length);
        if (bytes.SequenceEqual(RomanEndlessOriginalBytes)) return ExeRomanEndlessPatchState.Original;
        if (bytes.SequenceEqual(RomanEndlessPatchedBytes)) return ExeRomanEndlessPatchState.Patched;
        return ExeRomanEndlessPatchState.Unknown;
    }

    public static ExeCiviProduce20PatchState GetCiviProduce20PatchState(byte[] exeBytes) {
        if (exeBytes.Length < CiviProduce20PatchOffset + CiviProduce20OriginalBytes.Length) {
            return ExeCiviProduce20PatchState.Unknown;
        }
        byte[] bytes = ReadSpan(exeBytes, CiviProduce20PatchOffset, CiviProduce20OriginalBytes.Length);
        if (bytes.SequenceEqual(CiviProduce20OriginalBytes)) return ExeCiviProduce20PatchState.Original;
        if (bytes.SequenceEqual(CiviProduce20PatchedBytes)) return ExeCiviProduce20PatchState.Patched;
        return ExeCiviProduce20PatchState.Unknown;
    }

    public static ExeUnitRecruit20PatchState GetUnitRecruit20PatchState(byte[] exeBytes) {
        if (exeBytes.Length < UnitRecruit20PatchOffset + UnitRecruit20OriginalBytes.Length) {
            return ExeUnitRecruit20PatchState.Unknown;
        }
        byte[] bytes = ReadSpan(exeBytes, UnitRecruit20PatchOffset, UnitRecruit20OriginalBytes.Length);
        if (bytes.SequenceEqual(UnitRecruit20OriginalBytes)) return ExeUnitRecruit20PatchState.Original;
        if (bytes.SequenceEqual(UnitRecruit20PatchedBytes)) return ExeUnitRecruit20PatchState.Patched;
        return ExeUnitRecruit20PatchState.Unknown;
    }

    public static ExeIdleSelect999PatchState GetIdleSelect999PatchState(byte[] exeBytes) {
        if (exeBytes.Length < IdleSelect999PatchOffset + IdleSelect999OriginalBytes.Length) {
            return ExeIdleSelect999PatchState.Unknown;
        }
        byte[] bytes = ReadSpan(exeBytes, IdleSelect999PatchOffset, IdleSelect999OriginalBytes.Length);
        if (bytes.SequenceEqual(IdleSelect999OriginalBytes)) return ExeIdleSelect999PatchState.Original;
        if (bytes.SequenceEqual(IdleSelect999PatchedBytes)) return ExeIdleSelect999PatchState.Patched;
        return ExeIdleSelect999PatchState.Unknown;
    }

    public static ExeDefaultSpecialArrowsPatchState GetDefaultSpecialArrowsPatchState(byte[] exeBytes) {
        var sites = new[] {
            (DefaultSpecialArrowsNormalButtonOffset, DefaultSpecialArrowsNormalButtonOriginalBytes, DefaultSpecialArrowsNormalButtonPatchedBytes),
            (DefaultSpecialArrowsGetterOffset, DefaultSpecialArrowsGetterOriginalBytes, DefaultSpecialArrowsGetterPatchedBytes),
            (DefaultSpecialArrowsCaveOffset, DefaultSpecialArrowsCaveOriginalBytes, DefaultSpecialArrowsCavePatchedBytes),
        };
        bool allOriginal = true;
        bool allPatched = true;
        foreach (var (offset, original, patched) in sites) {
            if (exeBytes.Length < offset + original.Length) return ExeDefaultSpecialArrowsPatchState.Unknown;
            byte[] current = ReadSpan(exeBytes, offset, original.Length);
            allOriginal &= current.SequenceEqual(original);
            allPatched &= current.SequenceEqual(patched);
        }
        if (allOriginal) return ExeDefaultSpecialArrowsPatchState.Original;
        if (allPatched) return ExeDefaultSpecialArrowsPatchState.Patched;
        return ExeDefaultSpecialArrowsPatchState.Unknown;
    }

    public static ExeNativeWidescreenPatchState GetNativeWidescreenPatchState(byte[] exeBytes) {
        var modeSites = new[] {
            (NativeWidescreenIdentifyOffset, NativeWidescreenIdentifyOriginalBytes, NativeWidescreenIdentifyPatchedBytes),
            (NativeWidescreenCreateOffset, NativeWidescreenCreateOriginalBytes, NativeWidescreenCreatePatchedBytes),
            (NativeWidescreenModeTextOffset, NativeWidescreenModeTextOriginalBytes, NativeWidescreenModeTextPatchedBytes),
        };
        bool modesOriginal = true;
        bool modesPatched = true;
        foreach (var (offset, original, patched) in modeSites) {
            if (exeBytes.Length < offset + original.Length) return ExeNativeWidescreenPatchState.Unknown;
            byte[] current = ReadSpan(exeBytes, offset, original.Length);
            modesOriginal &= current.SequenceEqual(original);
            modesPatched &= current.SequenceEqual(patched);
        }
        if (exeBytes.Length < NativeWidescreenForceModeOffset + NativeWidescreenForceModeOriginalBytes.Length)
            return ExeNativeWidescreenPatchState.Unknown;
        byte[] forceMode = ReadSpan(exeBytes, NativeWidescreenForceModeOffset, NativeWidescreenForceModeOriginalBytes.Length);
        bool forceOriginal = forceMode.SequenceEqual(NativeWidescreenForceModeOriginalBytes);
        bool forcePatched = forceMode.SequenceEqual(NativeWidescreenForceModePatchedBytes);

        var uiSites = new[] {
            (NativeWidescreenActiveModeGetterOffset, NativeWidescreenActiveModeGetterOriginalBytes, NativeWidescreenActiveModeGetterPatchedBytes),
            (NativeWidescreenIgmDialogModeOffset, NativeWidescreenIgmDialogModeOriginalBytes, NativeWidescreenIgmDialogModePatchedBytes),
        };
        bool uiOriginal = true;
        bool uiPatched = true;
        foreach (var (offset, original, patched) in uiSites) {
            if (exeBytes.Length < offset + original.Length) return ExeNativeWidescreenPatchState.Unknown;
            byte[] current = ReadSpan(exeBytes, offset, original.Length);
            uiOriginal &= current.SequenceEqual(original);
            uiPatched &= current.SequenceEqual(patched);
        }

        if (modesOriginal && forceOriginal && uiOriginal) return ExeNativeWidescreenPatchState.Original;
        if (modesPatched && forceOriginal && uiOriginal) return ExeNativeWidescreenPatchState.LegacyUnforced;
        if (modesPatched && forcePatched && uiOriginal) return ExeNativeWidescreenPatchState.LegacyForcedStaleUi;
        if (modesPatched && forcePatched && uiPatched) return ExeNativeWidescreenPatchState.Patched;
        return ExeNativeWidescreenPatchState.Unknown;
    }

    public static ExeCameraZoomOutPatchState GetCameraZoomOutPatchState(byte[] exeBytes) {
        var callSites = new[] {
            (CameraZoomInitCallOffset, CameraZoomInitCallOriginalBytes, CameraZoomInitCallPatchedBytes),
            (CameraZoomLoadCallOffset, CameraZoomLoadCallOriginalBytes, CameraZoomLoadCallPatchedBytes),
            (CameraZoomScriptCallOffset, CameraZoomScriptCallOriginalBytes, CameraZoomScriptCallPatchedBytes),
        };
        bool callsOriginal = true;
        bool callsPatched = true;
        foreach (var (offset, original, patched) in callSites) {
            if (exeBytes.Length < offset + original.Length) return ExeCameraZoomOutPatchState.Unknown;
            byte[] current = ReadSpan(exeBytes, offset, original.Length);
            callsOriginal &= current.SequenceEqual(original);
            callsPatched &= current.SequenceEqual(patched);
        }
        if (exeBytes.Length < CameraZoomCaveOffset + CameraZoomCaveOriginalBytes.Length)
            return ExeCameraZoomOutPatchState.Unknown;
        byte[] cave = ReadSpan(exeBytes, CameraZoomCaveOffset, CameraZoomCaveOriginalBytes.Length);
        if (callsOriginal && cave.SequenceEqual(CameraZoomCaveOriginalBytes))
            return ExeCameraZoomOutPatchState.Original;
        if (callsPatched && cave.SequenceEqual(CameraZoomCaveLegacyZoomHalfBytes))
            return ExeCameraZoomOutPatchState.LegacyZoomHalf;
        if (callsPatched && cave.SequenceEqual(CameraZoomCavePatchedBytes))
            return ExeCameraZoomOutPatchState.Patched;
        return ExeCameraZoomOutPatchState.Unknown;
    }

    public static ExeVillageRangePatchState GetVillageBuildRangePatchState(byte[] exeBytes) {
        if (exeBytes.Length < VillageRangePatchRequiredLength) {
            return ExeVillageRangePatchState.Unknown;
        }

        byte[] xBytes = ReadSpan(exeBytes, VillageRangeXPatchOffset, VillageRangeXOriginalBytes.Length);
        byte[] zBytes = ReadSpan(exeBytes, VillageRangeZPatchOffset, VillageRangeZOriginalBytes.Length);
        byte[] frameXBytes = ReadSpan(exeBytes, VillageFrameXPatchOffset, VillageFrameXOriginalBytes.Length);
        byte[] frameZBytes = ReadSpan(exeBytes, VillageFrameZPatchOffset, VillageFrameZOriginalBytes.Length);

        bool original = xBytes.SequenceEqual(VillageRangeXOriginalBytes) &&
            zBytes.SequenceEqual(VillageRangeZOriginalBytes) &&
            frameXBytes.SequenceEqual(VillageFrameXOriginalBytes) &&
            frameZBytes.SequenceEqual(VillageFrameZOriginalBytes);
        bool legacyLogicOnly = xBytes.SequenceEqual(VillageRangeXPatchedBytes) &&
            zBytes.SequenceEqual(VillageRangeZPatchedBytes) &&
            frameXBytes.SequenceEqual(VillageFrameXOriginalBytes) &&
            frameZBytes.SequenceEqual(VillageFrameZOriginalBytes);
        bool expanded = xBytes.SequenceEqual(VillageRangeXPatchedBytes) &&
            zBytes.SequenceEqual(VillageRangeZPatchedBytes) &&
            frameXBytes.SequenceEqual(VillageFrameXPatchedBytes) &&
            frameZBytes.SequenceEqual(VillageFrameZPatchedBytes);
        if (original) return ExeVillageRangePatchState.Original;
        if (legacyLogicOnly) return ExeVillageRangePatchState.LegacyLogicOnly;
        if (expanded) return ExeVillageRangePatchState.Expanded;
        return ExeVillageRangePatchState.Unknown;
    }

    public static ExeVillageSetterPatchState GetVillageSetterPatchState(byte[] exeBytes) {
        if (exeBytes.Length < VillageSetterPatchRequiredLength) {
            return ExeVillageSetterPatchState.Unknown;
        }

        byte[] hookBytes = ReadSpan(exeBytes, VillageSetterHookOffset, VillageSetterHookOriginalBytes.Length);
        byte[] caveBytes = ReadSpan(exeBytes, VillageSetterCaveOffset, VillageSetterCaveOriginalBytes.Length);

        bool original = hookBytes.SequenceEqual(VillageSetterHookOriginalBytes) &&
            caveBytes.SequenceEqual(VillageSetterCaveOriginalBytes);
        bool legacy2x = hookBytes.SequenceEqual(VillageSetterHookPatchedBytes) &&
            caveBytes.SequenceEqual(VillageSetterCaveLegacy2xBytes);
        bool legacy2Point5x = hookBytes.SequenceEqual(VillageSetterHookPatchedBytes) &&
            caveBytes.SequenceEqual(VillageSetterCaveLegacy2Point5xBytes);
        bool legacy3x = hookBytes.SequenceEqual(VillageSetterHookPatchedBytes) &&
            caveBytes.SequenceEqual(VillageSetterCaveLegacy3xBytes);
        bool legacy5x = hookBytes.SequenceEqual(VillageSetterHookPatchedBytes) &&
            caveBytes.SequenceEqual(VillageSetterCaveLegacy5xBytes);
        bool entireMap = hookBytes.SequenceEqual(VillageSetterHookPatchedBytes) &&
            caveBytes.SequenceEqual(VillageSetterCavePatchedBytes);
        if (original) return ExeVillageSetterPatchState.Original;
        if (legacy2x) return ExeVillageSetterPatchState.Legacy2x;
        if (legacy2Point5x) return ExeVillageSetterPatchState.Legacy2Point5x;
        if (legacy3x) return ExeVillageSetterPatchState.Legacy3x;
        if (legacy5x) return ExeVillageSetterPatchState.Legacy5x;
        if (entireMap) return ExeVillageSetterPatchState.EntireMap;
        return ExeVillageSetterPatchState.Unknown;
    }

    // === 依狀態規劃寫入（核心：state → 預期/取代位元組選擇）===
    public static IReadOnlyList<ExeWriteOp> PlanFocus(bool enabled, ExePatchState state) {
        if (enabled && state == ExePatchState.Original) {
            return new[] { new ExeWriteOp(FocusPatchOffset, FocusOriginalBytes, FocusPatchedBytes, "失焦暫停相容性") };
        }
        if (!enabled && state == ExePatchState.FocusPatched) {
            return new[] { new ExeWriteOp(FocusPatchOffset, FocusPatchedBytes, FocusOriginalBytes, "失焦暫停相容性還原") };
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanSpellAltar(bool enabled, ExeSpellAltarPatchState state) {
        if (enabled && state == ExeSpellAltarPatchState.Original) {
            return SpellAltarPatchSites
                .Select(site => new ExeWriteOp(site.Offset, site.Original, site.Patched, "法術免祭壇需求"))
                .ToArray();
        }
        if (!enabled && state == ExeSpellAltarPatchState.Patched) {
            return SpellAltarPatchSites
                .Select(site => new ExeWriteOp(site.Offset, site.Patched, site.Original, "法術免祭壇需求還原"))
                .ToArray();
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanRomanEndless(bool enabled, ExeRomanEndlessPatchState state) {
        if (enabled && state == ExeRomanEndlessPatchState.Original) {
            return new[] { new ExeWriteOp(RomanEndlessPatchOffset, RomanEndlessOriginalBytes, RomanEndlessPatchedBytes, "無盡模式羅馬陣營") };
        }
        if (!enabled && state == ExeRomanEndlessPatchState.Patched) {
            return new[] { new ExeWriteOp(RomanEndlessPatchOffset, RomanEndlessPatchedBytes, RomanEndlessOriginalBytes, "無盡模式羅馬陣營還原") };
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanCiviProduce20(bool enabled, ExeCiviProduce20PatchState state) {
        if (enabled && state == ExeCiviProduce20PatchState.Original) {
            return new[] { new ExeWriteOp(CiviProduce20PatchOffset, CiviProduce20OriginalBytes, CiviProduce20PatchedBytes, "住宅生產一次數量") };
        }
        if (!enabled && state == ExeCiviProduce20PatchState.Patched) {
            return new[] { new ExeWriteOp(CiviProduce20PatchOffset, CiviProduce20PatchedBytes, CiviProduce20OriginalBytes, "住宅生產一次數量還原") };
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanUnitRecruit20(bool enabled, ExeUnitRecruit20PatchState state) {
        if (enabled && state == ExeUnitRecruit20PatchState.Original) {
            return new[] { new ExeWriteOp(UnitRecruit20PatchOffset, UnitRecruit20OriginalBytes, UnitRecruit20PatchedBytes, "招募一次到上限") };
        }
        if (!enabled && state == ExeUnitRecruit20PatchState.Patched) {
            return new[] { new ExeWriteOp(UnitRecruit20PatchOffset, UnitRecruit20PatchedBytes, UnitRecruit20OriginalBytes, "招募一次到上限還原") };
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanIdleSelect999(bool enabled, ExeIdleSelect999PatchState state) {
        if (enabled && state == ExeIdleSelect999PatchState.Original) {
            return new[] { new ExeWriteOp(IdleSelect999PatchOffset, IdleSelect999OriginalBytes, IdleSelect999PatchedBytes, "閒置村民一次全選 999") };
        }
        if (!enabled && state == ExeIdleSelect999PatchState.Patched) {
            return new[] { new ExeWriteOp(IdleSelect999PatchOffset, IdleSelect999PatchedBytes, IdleSelect999OriginalBytes, "閒置村民一次全選 999 還原") };
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanRetiredDefaultSpecialArrowsRestore(ExeDefaultSpecialArrowsPatchState state) {
        if (state == ExeDefaultSpecialArrowsPatchState.Patched) {
            return new[] {
                // Restore the getter before clearing its target.
                new ExeWriteOp(DefaultSpecialArrowsGetterOffset, DefaultSpecialArrowsGetterPatchedBytes, DefaultSpecialArrowsGetterOriginalBytes, "特殊箭矢預設 getter 還原"),
                new ExeWriteOp(DefaultSpecialArrowsNormalButtonOffset, DefaultSpecialArrowsNormalButtonPatchedBytes, DefaultSpecialArrowsNormalButtonOriginalBytes, "普通箭模式 sentinel 還原"),
                new ExeWriteOp(DefaultSpecialArrowsCaveOffset, DefaultSpecialArrowsCavePatchedBytes, DefaultSpecialArrowsCaveOriginalBytes, "特殊箭矢預設程式碼洞還原"),
            };
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanNativeWidescreen(bool enabled, ExeNativeWidescreenPatchState state) {
        if (enabled && state == ExeNativeWidescreenPatchState.Original) {
            return new[] {
                new ExeWriteOp(NativeWidescreenIdentifyOffset, NativeWidescreenIdentifyOriginalBytes, NativeWidescreenIdentifyPatchedBytes, "1920x1080 顯示模式辨識"),
                new ExeWriteOp(NativeWidescreenCreateOffset, NativeWidescreenCreateOriginalBytes, NativeWidescreenCreatePatchedBytes, "1920x1080 顯示模式建立"),
                new ExeWriteOp(NativeWidescreenModeTextOffset, NativeWidescreenModeTextOriginalBytes, NativeWidescreenModeTextPatchedBytes, "1920x1080 顯示模式文字"),
                new ExeWriteOp(NativeWidescreenForceModeOffset, NativeWidescreenForceModeOriginalBytes, NativeWidescreenForceModePatchedBytes, "啟動時強制選擇 1920x1080 mode 0x22"),
                new ExeWriteOp(NativeWidescreenActiveModeGetterOffset, NativeWidescreenActiveModeGetterOriginalBytes, NativeWidescreenActiveModeGetterPatchedBytes, "IGM 使用目前 1920x1080 mode 0x22"),
                new ExeWriteOp(NativeWidescreenIgmDialogModeOffset, NativeWidescreenIgmDialogModeOriginalBytes, NativeWidescreenIgmDialogModePatchedBytes, "mode 0x22 使用最高解析度 IGM 對話框"),
            };
        }
        if (enabled && state == ExeNativeWidescreenPatchState.LegacyUnforced) {
            return new[] {
                new ExeWriteOp(NativeWidescreenForceModeOffset, NativeWidescreenForceModeOriginalBytes, NativeWidescreenForceModePatchedBytes, "遷移舊版寬螢幕補丁並強制選擇 mode 0x22"),
                new ExeWriteOp(NativeWidescreenActiveModeGetterOffset, NativeWidescreenActiveModeGetterOriginalBytes, NativeWidescreenActiveModeGetterPatchedBytes, "遷移 IGM 至目前 mode 0x22"),
                new ExeWriteOp(NativeWidescreenIgmDialogModeOffset, NativeWidescreenIgmDialogModeOriginalBytes, NativeWidescreenIgmDialogModePatchedBytes, "遷移 mode 0x22 IGM 對話框"),
            };
        }
        if (enabled && state == ExeNativeWidescreenPatchState.LegacyForcedStaleUi) {
            return new[] {
                new ExeWriteOp(NativeWidescreenActiveModeGetterOffset, NativeWidescreenActiveModeGetterOriginalBytes, NativeWidescreenActiveModeGetterPatchedBytes, "修正 IGM 仍讀取舊解析度 mode"),
                new ExeWriteOp(NativeWidescreenIgmDialogModeOffset, NativeWidescreenIgmDialogModeOriginalBytes, NativeWidescreenIgmDialogModePatchedBytes, "修正 mode 0x22 IGM 對話框選擇"),
            };
        }
        if (!enabled && state == ExeNativeWidescreenPatchState.Patched) {
            return new[] {
                new ExeWriteOp(NativeWidescreenIdentifyOffset, NativeWidescreenIdentifyPatchedBytes, NativeWidescreenIdentifyOriginalBytes, "1920x1080 顯示模式辨識還原"),
                new ExeWriteOp(NativeWidescreenCreateOffset, NativeWidescreenCreatePatchedBytes, NativeWidescreenCreateOriginalBytes, "1920x1080 顯示模式建立還原"),
                new ExeWriteOp(NativeWidescreenModeTextOffset, NativeWidescreenModeTextPatchedBytes, NativeWidescreenModeTextOriginalBytes, "1920x1080 顯示模式文字還原"),
                new ExeWriteOp(NativeWidescreenForceModeOffset, NativeWidescreenForceModePatchedBytes, NativeWidescreenForceModeOriginalBytes, "啟動解析度選擇還原"),
                new ExeWriteOp(NativeWidescreenActiveModeGetterOffset, NativeWidescreenActiveModeGetterPatchedBytes, NativeWidescreenActiveModeGetterOriginalBytes, "IGM 解析度來源還原"),
                new ExeWriteOp(NativeWidescreenIgmDialogModeOffset, NativeWidescreenIgmDialogModePatchedBytes, NativeWidescreenIgmDialogModeOriginalBytes, "IGM 對話框 mode 還原"),
            };
        }
        if (!enabled && state == ExeNativeWidescreenPatchState.LegacyForcedStaleUi) {
            return new[] {
                new ExeWriteOp(NativeWidescreenIdentifyOffset, NativeWidescreenIdentifyPatchedBytes, NativeWidescreenIdentifyOriginalBytes, "1920x1080 顯示模式辨識還原"),
                new ExeWriteOp(NativeWidescreenCreateOffset, NativeWidescreenCreatePatchedBytes, NativeWidescreenCreateOriginalBytes, "1920x1080 顯示模式建立還原"),
                new ExeWriteOp(NativeWidescreenModeTextOffset, NativeWidescreenModeTextPatchedBytes, NativeWidescreenModeTextOriginalBytes, "1920x1080 顯示模式文字還原"),
                new ExeWriteOp(NativeWidescreenForceModeOffset, NativeWidescreenForceModePatchedBytes, NativeWidescreenForceModeOriginalBytes, "啟動解析度選擇還原"),
            };
        }
        if (!enabled && state == ExeNativeWidescreenPatchState.LegacyUnforced) {
            return new[] {
                new ExeWriteOp(NativeWidescreenIdentifyOffset, NativeWidescreenIdentifyPatchedBytes, NativeWidescreenIdentifyOriginalBytes, "1920x1080 顯示模式辨識還原"),
                new ExeWriteOp(NativeWidescreenCreateOffset, NativeWidescreenCreatePatchedBytes, NativeWidescreenCreateOriginalBytes, "1920x1080 顯示模式建立還原"),
                new ExeWriteOp(NativeWidescreenModeTextOffset, NativeWidescreenModeTextPatchedBytes, NativeWidescreenModeTextOriginalBytes, "1920x1080 顯示模式文字還原"),
            };
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanCameraZoomOut(bool enabled, ExeCameraZoomOutPatchState state) {
        if (enabled && state == ExeCameraZoomOutPatchState.Original) {
            return new[] {
                // Install the target before redirecting any call site.
                new ExeWriteOp(CameraZoomCaveOffset, CameraZoomCaveOriginalBytes, CameraZoomCavePatchedBytes, "攝影機拉遠 +1 code cave"),
                new ExeWriteOp(CameraZoomInitCallOffset, CameraZoomInitCallOriginalBytes, CameraZoomInitCallPatchedBytes, "攝影機初始縮放下限"),
                new ExeWriteOp(CameraZoomLoadCallOffset, CameraZoomLoadCallOriginalBytes, CameraZoomLoadCallPatchedBytes, "讀檔攝影機縮放下限"),
                new ExeWriteOp(CameraZoomScriptCallOffset, CameraZoomScriptCallOriginalBytes, CameraZoomScriptCallPatchedBytes, "任務腳本攝影機縮放下限"),
            };
        }
        if (enabled && state == ExeCameraZoomOutPatchState.LegacyZoomHalf) {
            return new[] {
                new ExeWriteOp(CameraZoomCaveOffset, CameraZoomCaveLegacyZoomHalfBytes, CameraZoomCavePatchedBytes, "攝影機拉遠 0.5 遷移為 +1"),
            };
        }
        if (!enabled && state == ExeCameraZoomOutPatchState.Patched) {
            return new[] {
                // Remove every redirect before clearing the shared target.
                new ExeWriteOp(CameraZoomInitCallOffset, CameraZoomInitCallPatchedBytes, CameraZoomInitCallOriginalBytes, "攝影機初始縮放還原"),
                new ExeWriteOp(CameraZoomLoadCallOffset, CameraZoomLoadCallPatchedBytes, CameraZoomLoadCallOriginalBytes, "讀檔攝影機縮放還原"),
                new ExeWriteOp(CameraZoomScriptCallOffset, CameraZoomScriptCallPatchedBytes, CameraZoomScriptCallOriginalBytes, "任務腳本攝影機縮放還原"),
                new ExeWriteOp(CameraZoomCaveOffset, CameraZoomCavePatchedBytes, CameraZoomCaveOriginalBytes, "攝影機拉遠 code cave 還原"),
            };
        }
        if (!enabled && state == ExeCameraZoomOutPatchState.LegacyZoomHalf) {
            return new[] {
                new ExeWriteOp(CameraZoomInitCallOffset, CameraZoomInitCallPatchedBytes, CameraZoomInitCallOriginalBytes, "攝影機初始縮放還原"),
                new ExeWriteOp(CameraZoomLoadCallOffset, CameraZoomLoadCallPatchedBytes, CameraZoomLoadCallOriginalBytes, "讀檔攝影機縮放還原"),
                new ExeWriteOp(CameraZoomScriptCallOffset, CameraZoomScriptCallPatchedBytes, CameraZoomScriptCallOriginalBytes, "任務腳本攝影機縮放還原"),
                new ExeWriteOp(CameraZoomCaveOffset, CameraZoomCaveLegacyZoomHalfBytes, CameraZoomCaveOriginalBytes, "舊版攝影機拉遠 0.5 code cave 還原"),
            };
        }
        return Array.Empty<ExeWriteOp>();
    }

    public static IReadOnlyList<ExeWriteOp> PlanVillageRangeRestore(ExeVillageRangePatchState state) {
        if (state != ExeVillageRangePatchState.Expanded &&
            state != ExeVillageRangePatchState.LegacyLogicOnly) {
            return Array.Empty<ExeWriteOp>();
        }

        byte[] expectedFrameX = state == ExeVillageRangePatchState.Expanded ? VillageFrameXPatchedBytes : VillageFrameXOriginalBytes;
        byte[] expectedFrameZ = state == ExeVillageRangePatchState.Expanded ? VillageFrameZPatchedBytes : VillageFrameZOriginalBytes;
        return new[] {
            new ExeWriteOp(VillageRangeXPatchOffset, VillageRangeXPatchedBytes, VillageRangeXOriginalBytes, "舊版村落建造範圍 X 還原"),
            new ExeWriteOp(VillageRangeZPatchOffset, VillageRangeZPatchedBytes, VillageRangeZOriginalBytes, "舊版村落建造範圍 Z 還原"),
            new ExeWriteOp(VillageFrameXPatchOffset, expectedFrameX, VillageFrameXOriginalBytes, "舊版村落框架 X 還原"),
            new ExeWriteOp(VillageFrameZPatchOffset, expectedFrameZ, VillageFrameZOriginalBytes, "舊版村落框架 Z 還原"),
        };
    }

    public static IReadOnlyList<ExeWriteOp> PlanVillageSetter(bool enabled, ExeVillageSetterPatchState state) {
        if (enabled) {
            if (state == ExeVillageSetterPatchState.Original ||
                state == ExeVillageSetterPatchState.Legacy2x ||
                state == ExeVillageSetterPatchState.Legacy2Point5x ||
                state == ExeVillageSetterPatchState.Legacy3x ||
                state == ExeVillageSetterPatchState.Legacy5x) {
                byte[] expectedCave = state == ExeVillageSetterPatchState.Original ? VillageSetterCaveOriginalBytes
                    : state == ExeVillageSetterPatchState.Legacy2x ? VillageSetterCaveLegacy2xBytes
                    : state == ExeVillageSetterPatchState.Legacy2Point5x ? VillageSetterCaveLegacy2Point5xBytes
                    : state == ExeVillageSetterPatchState.Legacy3x ? VillageSetterCaveLegacy3xBytes
                    : VillageSetterCaveLegacy5xBytes;
                byte[] expectedHook = state == ExeVillageSetterPatchState.Original ? VillageSetterHookOriginalBytes : VillageSetterHookPatchedBytes;
                return new[] {
                    new ExeWriteOp(VillageSetterCaveOffset, expectedCave, VillageSetterCavePatchedBytes, "村落建造範圍程式碼洞"),
                    new ExeWriteOp(VillageSetterHookOffset, expectedHook, VillageSetterHookPatchedBytes, "村落建造範圍跳板"),
                };
            }
            return Array.Empty<ExeWriteOp>();
        }

        if (state == ExeVillageSetterPatchState.Legacy2x ||
            state == ExeVillageSetterPatchState.Legacy2Point5x ||
            state == ExeVillageSetterPatchState.Legacy3x ||
            state == ExeVillageSetterPatchState.Legacy5x ||
            state == ExeVillageSetterPatchState.EntireMap) {
            byte[] expectedCave = state == ExeVillageSetterPatchState.Legacy2x ? VillageSetterCaveLegacy2xBytes
                : state == ExeVillageSetterPatchState.Legacy2Point5x ? VillageSetterCaveLegacy2Point5xBytes
                : state == ExeVillageSetterPatchState.Legacy3x ? VillageSetterCaveLegacy3xBytes
                : state == ExeVillageSetterPatchState.Legacy5x ? VillageSetterCaveLegacy5xBytes
                : VillageSetterCavePatchedBytes;
            return new[] {
                new ExeWriteOp(VillageSetterHookOffset, VillageSetterHookPatchedBytes, VillageSetterHookOriginalBytes, "村落建造範圍跳板還原"),
                new ExeWriteOp(VillageSetterCaveOffset, expectedCave, VillageSetterCaveOriginalBytes, "村落建造範圍程式碼洞還原"),
            };
        }
        return Array.Empty<ExeWriteOp>();
    }

    /// <summary>逐一套用寫入計畫；每筆寫入前先驗證目前位元組，任一不符即丟例外中止。
    /// 注意：先前已套用的項目會留在緩衝區中——呼叫端捕捉到例外後不得再使用該緩衝區寫檔。</summary>
    public static void Apply(byte[] exeBytes, IEnumerable<ExeWriteOp> ops) {
        foreach (ExeWriteOp op in ops) {
            VerifiedBinaryWriter.WriteBytes(exeBytes, op.Offset, op.Expected, op.Replacement, op.PatchName);
        }
    }

    private static byte[] ReadSpan(byte[] source, long offset, int length) {
        byte[] buffer = new byte[length];
        Buffer.BlockCopy(source, (int)offset, buffer, 0, length);
        return buffer;
    }
}
