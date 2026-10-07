# 地圖地形與物件光照：Task B

2026-10-07，Codex。只讀 repository EXE 與使用者指定 TEMP 副本；沒有存取遊戲安裝目錄、啟動遊戲、提交、push 或修改 Git 歷史。其他代理同時工作，本輪只建立指定的新檔案；依最新限制不修改 `AI_HANDOFF.md`，研究里程碑記錄於此。

**結論：已解出陰影 bitplane、日夜色表與部分原生光照公式；尚未完整重現截圖。** `shadows.dat` 不是單張灰階陰影圖，也不是物件素材 ZIP `shad.dat`。Sprite 光照包含空間局部光源及額外亮度項，不能只乘 `vertex.bmp` 或全域時間色。原筆記把同一張 `game-house.png` 的兩棟建築增益分別稱為白天／黃昏不成立，本輪把它們作同時觀察處理。

## 輸入與重現

EXE SHA256：`6AC85239EA3B87A4357ED8CE09A1818E68C09FE3C00E8D98831F473577B719BF`。
所有地址皆為此 x86 EXE 的 VA，不是檔案 offset；線性反組譯不是 callable ABI。

| 輸入 | SHA256 |
| --- | --- |
| ENDL_005/shadows.dat | `2569ED03B65B5D1AC9049C2867D60C8DFA6AA7558F3F905808EF2A3B0821B0B6` |
| ENDL_000/shadows.dat | `B459B7BC275FE244E31F0BA4FCF2B4062A46E47232426CFC28866E890FFF3635` |
| 兩張地圖 daynight.bmp（相同） | `EB3D85BB3ADB83F60D23239517CCB3F326900A751AC4BB8908AA1A3674C5163D` |
| game-house.png | `79CF156CD61F6D3F5290C7C1638D342934502C4029883860B65B6FA2A0EE22A7` |
| game-units.png | `3DD98158F0787BC94E3EA756E31A3C5D8E1EC1D404FC0F0AF1C79ECF296ACC18` |
| game-units-east.png | `74C93B2702A80B513F2D236454AA8C6C0A9A654822FE6CF247AD6420E6E79116` |

第三份 `%TEMP%/ArmNativeAssets_20261007/ENDL_000-shadows.dat` 與 ENDL_000 的檔案逐 byte 相同。地圖 BMP 使用 24-bit BI_RGB、正高度 bottom-up、4-byte stride 對齊；ENDL_000 的 smooth／vertex 有合法 2-byte trailer，不能強制 payload 結尾等於檔尾。

```powershell
$env:DOTNET_ROLL_FORWARD='Major'
$env:PYTHONPATH="$env:TEMP/arm-re-python"
python tools/re/scan_native_scene.py re_workspace/Against_Rome.exe `
  --target 0x5f3910 --target 0x5f3920 --target 0x5f395f `
  --target 0x5f3971 --target 0x5f3a22 --target 0x1b82eb0 `
  --target 0x1ce2c50 --target 0x49a490 --output "$env:TEMP/lighting-xrefs.txt"
python tools/re/lighting-probe/investigate.py disasm re_workspace/Against_Rome.exe `
  0x483BF0:0x483D4C 0x484460:0x4844EE 0x4845B0:0x484714 `
  0x486A00:0x486CE5 0x49A350:0x49A9AA 0x49FFE0:0x4A01AD
python tools/re/lighting-probe/quantitative.py "$env:TEMP/ArmGameCompare_20261007"
```

最後一條只讀指定 map／PNG／apt.dat 副本；APT 解碼原樣重用既有 C# parser，像素經 stdout 傳至 Python 記憶體。PNG decoder 使用標準庫處理 8-bit RGB／RGBA、非 interlaced PNG；不依賴 Pillow／NumPy、不寫出遊戲素材。工具建置產生正常 bin／obj，未將素材加入 Git。

## shadows.dat：已證實的格式與極性

PFIL 64-byte 壓縮 envelope（解壓長度在 +16）；本輪 stored lengths 為 132163／140553，解壓後均為 **1048608 bytes**。外部指標樣式的 envelope 欄位不屬於陰影 payload。

解壓後前 32 bytes 是八個 little-endian DWORD：

| offset | 欄位 | ENDL_005／ENDL_000 |
| --- | --- | --- |
| +0／+4 | 地圖 cell 寬／高（非 BMP 頂點數） | 256／256 |
| +8 | 時段數 | 128 |
| +12 | ShadowMeshAccuracy | 6 |
| +16 | ShadowMeshMode | 0 |
| +20／+24 | ShadowMeshXZsize／ShadowMeshYsize | 16000／5000 |
| +28 | native 高度總和，用於 cache 核對 | 10061300／12078871 |

其餘為 `128 × 256 × 32` bytes。`slice` 外層、Z 列中層、X/8 內層：

```text
offset = 32 + slice*8192 + z*32 + (x >> 3)
occluded = (byte & (1 << (x & 7))) != 0
```

bit1 是遮蔽；原生轉成 255 後使用 `1 - value/256*strength`，不是 brightness=255。

| EXE 位置 | 直接證據 |
| --- | --- |
| 0x486A65 | push 0x5F3A22，實際為 `shadows.dat`；掃描 maximal printable string 時因前方 `B` 顯示 `Bshadows.dat`，須使用內部正確起點 |
| 0x486AD6..0x486C10 | 讀八個 DWORD，與當前尺寸／設定／0x484E50 高度總和比對；失配重建，故此資料是地形 cache，不是 snapshot 物件清單 |
| 0x486C32..0x486CBA | slice stride 0x2000、256 列、每列32 bytes，寫到 0x01CE2C50 |
| 0x486E01..0x486E8A | 儲存相同八個欄位及相同 bitplane 次序 |
| 0x49A7DD..0x49A82A | secondsOfDay×128/86400 選 slice，675 秒週期、下一 slice mod128，8-bit 時間權重 |
| 0x49A893..0x49A90B | X/8 與 LSB-first mask；兩個 bit 轉 0／255，再按權重 >>8，寫到0x01CA2C50 |
| 0x49A627..0x49A741 | world X/Z <<10，即每64世界單位一格；clamp 0..255，fixed-point 雙線性插值，最後乘 strength 並從1減去 |

精確時間公式：`slice=floor(seconds*128/86400)`、`w=floor((seconds%675)*256/675)`、`q=(a*(256-w)+b*w)>>8`。空間公式**不是一般 float bilinear**：每軸補權重是65535；先Z加權 >>9，再X加權 >>23。因此四角皆255，在格點仍得到254。`MapLightingShadowMap` 保留此量化與 native 1/256 分母。

沒有證實這個 cache 包含建築／單位投影陰影；物件陰影使用另一條 `shad.dat` 路徑，見既有 native-scene-rendering.md。不得用本解析器代替物件 shadow masks。

## 日夜、平滑、emboss 與 vertex 的分工

| 檔案 | loader／consumer | 已驗證作用與限制 |
| --- | --- | --- |
| daynight.bmp | 0x484466→0x414AD0；pixel pointer 存0x01B82EB0；0x49A350 | 24×6 RGB；已追到的日夜消費端只按 hour0..23 讀首列，下一 hour mod24；row4 未出現在此公式 |
| vertex.bmp | 0x483BFA→0x414AD0；0x483CFF..0x483D0B→0x016365EC | 257×257 RGB；terrain consumer0x499A33..0x499A8F按色道讀值再+1，白色是256，不是255/255 |
| smooth.bmp | 0x4835FA→0x414AD0；0x4837C2..0x4837D7 | **只讀綠色通道作平滑程度**；不是乘色 mask；操作 native height grid |
| emboss.bmp | 0x4845BA→0x414AD0；0x4846C3..0x4846D2→0x01676DF0 | **只讀綠色通道存 ushort**；遺失／不符尺寸的 fallback 是255；非 RGB 法線图 |

原生 BMP reader0x56E080（0x56E0E0 比較 BM）把檔案列向轉成 runtime 順序；0x56E398..0x56E39D從height−1列起寫入。24-bit 檔案BGR轉換不能沿用原始 bytes 當RGB。

`smooth.bmp`：0x483801..0x483B36迴圈門檻0..maxGreen；只有 green>門檻的格點參與。每次從前一 pass 讀取高度：中心權重256、四鄰各3、對角各2，邊界略過缺鄰並重新除權重和。0x483B79..0x483B82將16.8結果 >>8 寫回0x016121E8 height grid。綠色0不平滑；綠色n參與n次 pass。此步改變高度／坡度，間接影響地形照明、遮蔽與投影，不是獨立 RGB gain。

`emboss.bmp`：0x4907D6..0x490812讀三角形各頂點的ushort、<<8填入 raster vertex 的+0x2C attribute；RGB來自另三個光照陣列0x016F7DF8／0x01738600／0x01778E08。0x4046A0按三頂點此attribute總和為0或非0選一般／額外attribute的raster path；0x406285..0x4062A5讀該attribute作插值，最後經0x40FC10的0x00618B44分派表。**已追到插值及分派，尚未逐一解析scalar／MMX等實作的最終emboss混色／查表公式**；不把G直接當`G/255`乘上texture。本輪只能純解碼其RGB／G資料。

`daynight.bmp` 的row0與row4均各有24個RGB，row1／2／3／5全黑。row0 h0=(61,153,195)、h12=(255,245,227)、h18=(203,200,149)；row4 h0=(169,177,255)、h12=(255,255,255)、h18=(255,239,151)。載入整張BMP不代表六列皆被使用。對0x01B82EB0／EB4的靜態引用僅見loader、釋放與0x49A350首列讀取；**目前沒有row4是物件色／sunlight／fog色的證據**。保留原值供後續研究，不套用row4。

row0插值：`wn=floor(minute*65536/60)`、`wc=floor((60-minute)*65536/60)`；`channel=((current*wc+next*wn)>>16)/256`。兩權重各自floor，因此不總是加成65536。`DayStartTime=6`／`DayEndTime=20`載入0x771C50等設定（0x4828A4..0x482A5D），沒有在此RGB公式中把時段重新映射為6..20。

`gametime.dat` 解壓32bytes為`(1,134,4,55,17,10,6,2002)`；0x487F50處理前置欄，後七欄按0x771D30..D48順序。色表用D38的minute55與D3C的hour17，故保存的時間是**17:55**，不是把4、55誤讀成04:55。0x49A780用D34／D38／D3C組秒、分、時。134的產品單位未完全追查。保存時間不是三張截圖拍攝時間；不得將文字「daytime／dusk」換成確定時刻。

## 物件與地形公式：哪些已證實

Sprite的0x49A490路徑依序：

1. 取得全域row0 ambient（0x771870／874／878）。
2. 若開啟cloud shading，0x49A0E0按world X/Z取移動的0x01BE2C50 map，計算並clamp到至少0.5的factor。
3. 若開啟terrain shadow，0x49A620取上節bitplane轉成的factor；與cloud factor相乘為S。
4. 0x49FFE0逐局部光源計算世界XYZ距離平方d²。若d²<radius²，候選`lightRGB*(1-d²/radius²)`與ambient**逐色道取max**，最後各色道上限1；不是把所有光相加。light runtime stride0x44，RGB位於0x7F9F00..08。
5. 0x4D3930在world X/Z經整數轉換、>>6後取0x019AE429 byte V。其完整產品語意尚未追到所有寫入端，可能是visibility／fog-of-war；不命名為另一張shadow.bmp。
6. `nativeColorScale = locallyLitRGB * 256 * (0.707*S + 0.293) * (V*k)^2`，`k=Float32(0x3B7FFF34)=0.0039062025025486946`。本輪SpriteGain回傳此值/256。

0x4CC6FE、0x4CC753、0x4CC7AC、0x4CC7EC在APT patch四角反覆呼叫此路徑；0x4CC7F4起乘16777216轉fixed-point。故一棟建築可隨patch位置／高度呈現不同光照，不能只用物件錨點的一個RGB。ALR等其他consumer在0x4B5439等也有呼叫，但未全面核對各類型是否同樣四角取樣。

上述sprite helper **不讀vertex.bmp**。地形光照0x499620則另讀vertex RGB+1、坡度／native預算shade、cloud／terrain shadow、局部光源，以及V等項，產生各色道的fixed-point格點陣列，再交raster插值；0x499BDB..0x499C7B有明確多項乘法。不是`texture*vertex*row0`便能完整重現。emboss另走raster attribute，smooth事先改height。

## 截圖定量：驗證與反證

以同一`game-house.png`、主屋anchor(512,340)、住宅anchor(896,532)，解碼gerhau00 frame1000／gerwoh00 frame40（palette0）。選不透明內部、周圍2px亦不透明、原色各channel≥30且max≤240、screenY<650的像素，避免輪廓與HUD。gain=`sum(source*screen)/sum(source²)`，每channel獨立、無截距。兩輪各刪除殘差最大的10%只是robust fitting，**不是已確認原生光照mask**。

| sprite／樣本 | 數量 | R／G／B gain | RGB pixel RMSE（0..255） |
| --- | --- | --- | --- |
| gerhau00，完整有效樣本 | 57308 | .68215／.87379／.85313 | 20.005 |
| gerhau00，trim後 | 46419 | .56070／.82355／.84671 | 11.152 |
| gerwoh00，完整有效樣本 | 13576 | .51163／.82914／.86496 | 3.610 |
| gerwoh00，trim後 | 10996 | .51370／.83228／.86930 | 2.815 |

主屋screenY<400與400..649的R gain分別約.666與1.043；後者含火光動畫區，不能把>1當已證實light增益。本輪主屋trim結果與原提供的.70有差異，說明抽样範圍／動畫／空間光照很重要，不能挑某一數字當全屋常數。住宅較接近固定gain，但仍有數pixel誤差。

可核對的例子：

| screen XY | sprite source RGB | screen RGB |
| --- | --- | --- |
| (474,270)，主屋 | (152,132,72) | (82,113,66) |
| (411,225)，主屋 | (124,120,56) | (57,93,49) |
| (867,408)，住宅 | (80,72,57) | (41,60,49) |
| (931,528)，住宅 | (181,167,118) | (90,142,99) |

窮舉row0全部1440個整分鐘：對trim主屋的最佳gain RMSE=.06284，住宅=.05619。再允許同一時間、各棟独立0..1暗化scalar，最佳RMSE仍為**.06100**（最佳候選minute219，两scalar均1）；這是反證簡化模型，不是推定截圖時間為03:39。保存17:55的row0為(.796875,.78125,.5859375)，也不能當拍照時刻。

兩棟world anchors=(10624,10112)／(11392,10112)，在map cell(166,158)／(178,158)：vertex均(255,255,255)、emboss均(14,14,14)、smooth均0、boden均35。已抽樣04:55、候選03:39、12:00、19:00的兩cell陰影時間值皆0。故**沒有證據把兩棟差異歸因於已抽樣的anchor陰影**。局部光源與APT patch取樣是EXE可證實的額外機制，但未有同時runtime light／weather／V snapshot，尚未定量證明哪項造成此差異。

地面抽樣`game-house.png`：(400,440)=(90,73,57)、(480,500)=(82,60,49)、(600,500)=(82,60,49)。按既有平地投影反推world约(10712,10424)、(10912,10464)、(11032,10344)，vertex皆白。未完成這些位置的texture ID／texel／occlusion配對，故這些是觀測值，**不是terrain公式驗收**。

另兩張1024×768 PNG已實際解碼：同screen三點game-units RGB=(16,24,8)、(33,56,24)、(16,40,24)；game-units-east=(16,20,8)、(49,77,57)、(16,36,24)。camera不同，不能把同screen XY當同world點；未推定曝光時間／單位動畫格。完整sprite／地形公式尚未對這兩張通過定量驗收。

## 實作、測試與下一步

新檔案：`MapLightingShadowMap.cs`、`MapLightingBitmap.cs`、`MapLightingModel.cs`、`MapLightingTests.cs`；工具位於`tools/re/lighting-probe/`。皆純bytes／數學；沒有接現有UI或修改既有parser。Shadow parser只接受**已解壓payload**，避免沿用既有GameLZSS截斷後補零的容忍行為而把損壞輸入當有效陰影。C#實際驗證兩張map共12個輸入，0失敗。

16個合成xUnit案例涵蓋BMP BGR、兩種列向、padding、trailer、snapshot所有權、截斷／extent／unsupported depth；row4保留、row0獨立floor／跨午夜；shadow layout／LSB／時間插值／65535空間量化／边界；局部光源max／d²與sprite平方V項。獨立測試project只link這些新檔案，供共用樹被其他代理暫時破壞時檢查本輪程式。

驗證里程碑：首次指定build被另一代理當時的NativeShadowCatalog.cs三個CS0050／CS0051阻擋，未改其檔案；稍後該錯誤解除。最終指定`dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false`為**0 warnings／0 errors**；指定modules `--no-build`為**188 passed／0 failed／0 skipped**，含本輪16個案例。探針曾因`--no-build`未附`-p:UseAppHost=false`尋找不存在exe而失敗；已修正後完整實際素材／像素probe exit0。未提交或改Git歷史；期間其他代理的commit不屬本輪操作。

後續需要：完成emboss的各renderer分支及terrain最終RGB公式；追完V map寫入端；取得同拍照時刻的時間、light與weather snapshot；按APT patchXYZ取樣並重現fixed-point interpolation；建立原地面texture texel對照後再量化terrain RMSE。row4須找到直接consumer才賦予產品語意。現有合成測試與反證不等於完整遊戲畫面重現。
