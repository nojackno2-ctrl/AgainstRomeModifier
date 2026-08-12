# argm-trace — Against Rome 執行期飛行紀錄器 / Runtime Flight Recorder

> 狀態 / Status: **已用 MSVC 編譯成功,但尚未在真實遊戲上實測 (compiles; not yet run against the live game).** 目前輸出為 Win32 `winmm.dll` 代理；host 測試（指令長度解碼器 + 對真 Win32 API 的完整 inline-hook 機制）透過 `-DARGM_BUILD_TESTS=ON` + CTest 執行。所有 hook 目標的序言與參數慣例已直接從安裝版 EXE（`TimeDateStamp=0x404D1710`）逐位元組驗證。剩下的遊戲安裝與無盡模式實跑會寫入遊戲目錄，必須由使用者透過修改器的 Apply／Restore 流程執行。

## 這是什麼 / What it is

《Against Rome》(2004) 是閉源引擎，本身沒有 log 輸出。`argm-trace` 是一個 32 位元的 `winmm.dll` 代理 DLL，由修改器部署後載入遊戲行程，對已逆向出的關鍵函式安裝 **僅記錄、不改變行為** 的 inline hook，把電腦 AI 的實際動作寫進 `argm_trace.log`：

- **AI 增援生成** (`s_addNPCJob_createUnit` 實作 `0x00547F50`):每次增援的隊伍、模式、單位數量範圍。這是「電腦到底做了什麼」最直接的紀錄。
- **AI 復活啟用** (`s_setNPCActive` `0x00548CE0`):被擊敗的隊伍何時重新變成可重生。
- **聚落式抵達** (`s_setVillageTemplate` `0x00549500`)。
- **單位建立** (`s_createUnitAndMems` `0x0052A020`)。
- **復活資格查詢** (`s_NPCActive` getter `0x00548D20`):每次 AI 查詢隊伍復活資格時,一次快照全部 8 隊的 `DAT_029e6000` 旗標。這是「為什麼某隊不增援」最直接的證據。
- **新局邊界** (level-init sweep `0x0054A070`):每次載入關卡清零隊伍狀態時記一條分隔線,把不同局的事件在同一 log 檔裡分開。
- **單位數量上限** (`s_createBattleUnitsMax`/`s_createCiviUnitsMax` `0x005249D0`/`0x00524D70`):記錄 clamp(≤20) 前的請求數量。
- **陣營選擇** (無盡羅馬陣營 setter `0x0045BD60`,有完整 21-byte 簽章驗證;同時附「強制羅馬補丁後」的第二組簽章,已套用 Modifier 補丁的安裝一樣能追蹤)。
- **完整 BCI opcode 串流** (VM dispatcher 真正入口 `0x005B1C60`;VM context 是第 1 個堆疊參數):逐指令追蹤,音量極大,預設關閉。

搭配 `docs/reverse-engineering/endless-mode-ai.md` 的 party 狀態機說明,這份 log 可以回答「AI 增援為什麼停了」「隊伍槽是不是耗盡」「party 卡在哪個狀態」這類問題。

## 安全設計 / Safety model

和 repo 現有 patch「先驗證原始 bytes 才寫入」的哲學一致,任何一個 hook 失敗都是 no-op,不會弄壞遊戲:

1. **組建指紋閘門**:啟動時讀取 `Against_Rome.exe` 的 PE 指紋 (ImageBase / TimeDateStamp / SizeOfImage / entry bytes) 並寫入 log。除了有簽章驗證的陣營 hook 之外,所有以位址為基礎的 hook 只有在 `argm_trace.ini` 的 `expectedTimeDateStamp` 與實際組建相符時才會安裝。
2. **簽章驗證**:有已知原始 bytes 的目標會先比對再 hook,不符就跳過。
3. **序言可重定位性檢查**:內建 32 位元指令長度解碼器,若要竊取的序言含相對跳轉/呼叫或無法解碼的指令,直接放棄該 hook。
4. **ASLR 重定位**:文件位址假設 image base `0x00400000`,執行期依實際載入位址修正。
5. **純記錄**:產生的 stub 保留所有暫存器與旗標、不動堆疊,對 cdecl / stdcall 皆安全;formatter 例外被 `__try/__except` 吞掉。

## 建置 / Build (Windows + MSVC, 32-bit)

```powershell
cmake -S native/argm-trace -B build/argm-trace -A Win32
cmake --build build/argm-trace --config Release
```

輸出為 `winmm.dll`。需要 Visual Studio 2019+ 的 C++ 桌面工作負載。

## 使用 / Usage

1. 先用 Modifier 的備份功能備份遊戲(本工具不改檔,但養成習慣)。
2. 由修改器的 ArgmTrace Apply／Restore 流程管理 `winmm.dll`，不要直接改寫遊戲安裝目錄。
3. (可選) 複製 `argm_trace.ini.sample` 為 `argm_trace.ini` 調整選項。
4. 啟動遊戲一次,打開產生的 `argm_trace.log`,把 `[build]` banner 裡的 `TimeDateStamp` 值填進 `argm_trace.ini` 的 `expectedTimeDateStamp`。這一步確認你的 EXE 就是被逆向的組建,之後 AI 事件 hook 才會啟用。
5. 再次啟動遊戲、進入無盡模式重現問題,然後把 `argm_trace.log` 交給分析。

### 與 dgVoodoo2 併用

`argm-trace` 使用 `winmm.dll` 代理，與 dgVoodoo2 的 `DDraw.dll` / `D3D8.dll` 不衝突。

## 日誌格式 / Log format

```
[      1234.567][t0a1c][ai.spawn  ] addNPCJob_createUnit team=5 mode=1 a3=4 ... count=20..20 ret=004195F8
```

欄位:自載入起的毫秒時戳、執行緒 id、事件分類、內容。時戳用 QueryPerformanceCounter,單調遞增,方便建立時間序列。

## 限制 / Limitations

- 尚未在真實遊戲驗證(見頁首)。務必先在可拋棄的遊戲副本上測試。
- 位址僅對被逆向的那個 `Against_Rome.exe` 組建有效;其他組建會被指紋閘門擋下、不安裝 hook。
- 這是原生 C++ 子專案,不屬於 .NET solution (`AgainstRomeModifier.slnx`),不會被 `dotnet build` / `dotnet test` 涵蓋。
