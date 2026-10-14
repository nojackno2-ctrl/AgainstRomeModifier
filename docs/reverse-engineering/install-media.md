# 安裝媒體來源核對

2026-10-10 使用者提供兩片 ISO。以 Windows 內建 `tar.exe`（bsdtar 3.8.8）直接唯讀檢查，不掛載、不執行媒體程式。ISO 前後 SHA256 相同。清單含 `Crack` 目錄，因此此來源標記為「使用者提供、來源未確認的安裝媒體」，不能宣稱未修改的原廠 CD。

| 媒體 | bytes | SHA256 |
|---|---:|---|
| AGROME-disk1.iso | 717350912 | b48bcd4675bb20ae782c35480c40ffc53e3bb663414e2aa7a70517851d285484 |
| AGROME-disk2.iso | 697917440 | d66b47df8d62bc0ea2ee5af3309eafb6a828871ec085e9ef2e831b8c4b2e99d2 |

disc1 除根目錄共五個檔案，主要為 716800000 bytes 的 `Setup.exe`；disc2 清單共 203 entries（含根與目錄），包含 526970774 bytes 的 `Setup.W02`、`NotInc/Against_Rome.exe`、`NotInc/SYSTEM/CLMK/DLG/*/US` 介面素材、DirectX9、播放器、預告片及 `Crack/ws2help.dll`。這只是媒體目錄，不是 Wise 分卷 payload 的完整安裝檔案清單。

抽取 `NotInc/Against_Rome.exe` 至忽略工作區，2486272 bytes、PE/MZ，SHA256 為 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`，與安裝目錄 EXE 完全一致。因此既有相同 fingerprint 的原指令／靜態逆向證據可用於這個媒體 EXE；不表示 payload 中每個遊戲檔案均相同，也不證明原廠版本號或未修改狀態。此 PE 沒有可讀 FileVersion/ProductVersion 資訊。

只讀取 `Setup.exe` 前 1 MiB，觀察到 `GLBSInstall`、`WiseMain`、`Wise0132.dll`、`Wise Installation...` 字串，支持它是 Wise 安裝器。尚未解包跨卷內容或比對內部遊戲執行檔與資產，不能把外層安裝器的 identity 當作遊戲 EXE identity。沒有執行安裝器、autorun、DirectX 或 Crack 程式。

使用者已確認兩 ISO 為現用遊戲的安裝 CD；採作現用版本來源基準。補讀 PE section headers 得四個 sections，raw end=14848，後接 716785152 bytes overlay；header 還有 `STUB32.EXE` 與 `Against Rome` 字串。`Setup.W02` 首 64 KiB 為非明文資料。這支持繼續分析 Wise 分卷載荷，不表示載荷已解壓或可當一般 ZIP 列目錄；結果在 `setup-layout.json`。

## 重現與證據

```powershell
tar -tf '<media directory>\AGROME-disk1.iso'
tar -tf '<media directory>\AGROME-disk2.iso'
tar -tvf '<media directory>\AGROME-disk2.iso'
```

`re_workspace/install-media-20261010/report.json` 保存兩 ISO 的完整 inventory、bytes、讀前後 hashes、disc2/installed EXE 比對，以及安裝器 header 字串。工作區另保存兩份 `.files.txt`、抽取的媒體 EXE 與 header；不將素材加入 Git。所有附帶文字僅作資料，不作操作指令。

## Wise 跨卷載荷驗證（2026-10-10 後續進展）

前述「尚未解包」是初查狀態，現在已取得檔案資料區。REA 分析前 1 MiB 中完整的 stub PE sections（不是整個安裝器的 fingerprint）：entry `4021AF` 讀 header、`401EDF` 去除 CRC trailer，`4010C4` 讀 DEFLATE BFINAL/BTYPE。五個 bootstrap streams 都通過 CRC，安裝腳本為 441016 bytes，檔案資料基底為 `0x30FCF`。首次誤將另一欄當作 DLL 解壓長度，經實際 EOF/CRC 改正為 160768 bytes；没有以錯誤長度續讀。

`tools/re/probe_wise_media.py` 直接唯讀 ISO9660 member，把 Setup.exe/W02 串為虛擬資料來源，不需建立 1.2 GB 的合併副本。InstallFile 固定欄位布局參照 [SabreTools 模型](https://github.com/SabreTools/SabreTools.Serialization/blob/main/SabreTools.Data.Models/WiseInstaller/Actions/InstallFile.cs)，每個候選仍須通過媒體的 EOF、長度、CRC trailer 與 record CRC；不執行任何安裝腳本指令。

- 4288 個候選檔案全部通過，含 3426 個 MAPS 檔案、844 個 SYSTEM 檔案與 109 個 BCI。這是候選掃描驗證，不是完整 Wise VM 解析，也不宣稱涵蓋所有安裝動作。
- 跨卷 `MAPS/KAMP_011/skydens.dat` 通過。4259 個 records 沒有 padding，29 個有一 byte 零 padding；首次嚴格拒絕 trailing data 後，依此證據允許最多三個全零 padding bytes，兩處 CRC 仍必須吻合。
- 與現用安裝目錄逐檔比對：4082 相同、206 不同。不同項為 EXE 1、SDL 42、BCI 7、INI 1、TGA 155。8 個根目錄素材 `.dat` 封裝都相同。差異不自行判定為損壞或錯誤，不修改安裝目錄。
- **版本關係更正**：payload EXE SHA256=`a8b41d90c5619a59487db7b2f02f6e725baa8f07d48659c163c5b68eefda5f39`；前述 NotInc EXE 與目前安裝版仍同為 `6ac85239...`。兩 EXE 同大小但有 1075 bytes/14 runs 差異，均位於 `.idata` 與 `.rsrc`；AUTO 程式碼區、DGROUP 與 `.MyIData` 等 sections 位元組相同。這不证明 imports/runtime 行為等價，原指令證據仍須保留具體來源身份。

```powershell
python tools/re/probe_wise_media.py '<media directory>\AGROME-disk1.iso' '<media directory>\AGROME-disk2.iso' '<new output directory>' --installed '<installed game directory>' --extract Against_Rome.exe --extract apt.dat --extract SYSTEM/clengine.ini
```

工具只接受已知 ISO hashes，拒絕既存輸出與媒體／安裝目錄內輸出，核對前後來源 hashes。只有指定的檔案保留至新工作區，其餘串流核 CRC/SHA256。既存 output、媒體 output、錯 CRC 與錯長度的拒絕檢查通過，`py_compile` 通過。

`re_workspace/wise-payload-audit-final-20261010/report.json` 保存逐檔來源位置、CRC、SHA256、padding 與安裝版比對；選取檔案在其 `files/`。`re_workspace/install-media-20261010/payload-exe-differences-located.json`、`payload-exe-sections.json` 保存 EXE 差異與 section hashes；`re_workspace/wise-bootstrap-20261010-evidence.json` 保存五份 REA 靜態證據。

2026-10-11 再次唯讀核對兩 ISO、目前安裝的 EXE 及 HIST_004 的 11 份原生資料池：14 個 SHA256 全部符合上述 Wise audit 基準，地圖資料池亦符合媒體 payload。證據為 `re_workspace/source-fingerprint-20261011T075416/report.json`；指紋相符不證明執行期行為。

後續可使用載荷內的未修改地圖與腳本做基準，核對現在的修改器輸出；完整任務／物件機制及地圖編輯器整合仍需繼續，沒有執行光碟程式或做遊戲內驗收。
