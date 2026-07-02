using System;
using System.Collections.Generic;
using System.Buffers;

namespace AgainstRomeModifier {
    // 遊戲特化的 LZSS 壓縮與解壓縮演算法實作類別
    public class GameLZSS {
        // 解壓縮方法：將輸入的 LZSS 壓縮位元組陣列還原為原始資料
        public static byte[] Decompress(byte[] input) {
            // 安全防護：若輸入為空或小於 64 位元組的標頭長度，則直接返回
            if (input == null || input.Length < 64) {
                return input ?? Array.Empty<byte>();
            }
            // 讀取標頭中的未壓縮資料大小（第 16 到 19 位元組）
            int uncompressedSize = BitConverter.ToInt32(input, 16);
            // 防禦性邊界檢查：若解壓大小為負或大於 50MB，拋出異常以防記憶體炸彈攻擊
            if (uncompressedSize < 0 || uncompressedSize > 50 * 1024 * 1024) {
                throw new System.IO.InvalidDataException();
            }
            byte[] output = new byte[uncompressedSize];
            int ip = 64; // 輸入指標，跳過 64 位元組自訂標頭
            int op = 0; // 輸出指標
            int ringSize = 4096; // 4KB 環狀滑動視窗大小
            byte[] ring = ArrayPool<byte>.Shared.Rent(ringSize);
            try {
                // 初始化環狀視窗，原版規範以空格字元 (0x20) 填充
                for (int x = 0; x < ringSize; x++) ring[x] = 0x20;
                int r = 4078; // 視窗寫入起始位置 (4096 - 18)

                // 開始迴圈解壓直到處理完所有輸入或填滿輸出陣列
                while (ip < input.Length && op < uncompressedSize) {
                    byte flags = input[ip++]; // 讀取 8 位元的控制旗標
                    for (int i = 0; i < 8; i++) {
                         if (ip >= input.Length || op >= uncompressedSize) break;
                         // 控制旗標第 i 位元為 1：代表接下來是 1 位元組的原始字元
                         if ((flags & (1 << i)) != 0) {
                              byte b = input[ip++];
                              output[op++] = b; // 輸出原始字元
                              ring[r] = b; // 寫入環狀視窗
                              r = (r + 1) & 4095; // 指標向後移動（使用位元與運算 & 4095 代替取餘數 % 4096 進行優化）
                         } else {
                              // 控制旗標第 i 位元為 0：代表接下來是 2 位元組的字典指標（位移與長度）
                              if (ip + 1 >= input.Length) break;
                              byte p1 = input[ip++];
                              byte p2 = input[ip++];
                              // 解析字典指標中的視窗位移 (offset)
                              int offset = p1 | ((p2 & 0xF0) << 4);
                              // 解析字典指標中的比對長度 (length)，並加上基本長度限制 3
                              int length = (p2 & 0x0F) + 3;
                              // 從視窗中拷貝歷史字元到輸出，並更新滑動視窗
                              for (int k = 0; k < length; k++) {
                                   byte b = ring[(offset + k) & 4095];
                                   if (op < uncompressedSize) output[op++] = b;
                                   ring[r] = b;
                                   r = (r + 1) & 4095;
                              }
                         }
                    }
                }
            } finally {
                ArrayPool<byte>.Shared.Return(ring);
            }
            return output;
        }

        // 壓縮方法：將原始資料壓縮為 LZSS 格式位元組
        public static byte[] Compress(byte[] input) {
            int N = 4096; // 環狀視窗大小
            int F = 18; // 最大匹配長度
            int threshold = 2; // 超過此長度才進行壓縮編碼
            byte[] win = ArrayPool<byte>.Shared.Rent(N);
            int[] head = ArrayPool<int>.Shared.Rent(65536);
            int[] next = ArrayPool<int>.Shared.Rent(N);
            int[] prev = ArrayPool<int>.Shared.Rent(N);
            int[] currentHash = ArrayPool<int>.Shared.Rent(N);

            try {
                // 初始化視窗為空格字元 (0x20)
                for (int x = 0; x < N; x++) win[x] = 0x20;
                int r = N - F; // 視窗寫入位置起始於 4078
                List<byte> output = new List<byte>();
                int src = 0; // 來源資料指標
                int len = input.Length;
                byte[] codeBuf = new byte[17]; // 暫存編碼緩衝區
                codeBuf[0] = 0; // 控制旗標位元組
                int codePtr = 1;
                int mask = 1; // 控制旗標遮罩

                for (int i = 0; i < 65536; i++) head[i] = -1;
                for (int i = 0; i < N; i++) {
                    next[i] = -1;
                    prev[i] = -1;
                    currentHash[i] = -1;
                }

                // 初始化滑動視窗的雜湊值 (計算視窗中每 3 個字元組合的雜湊)
                for (int i = 0; i < N; i++) {
                    int h = ((win[i] << 8) ^ (win[(i + 1) & 4095] << 4) ^ win[(i + 2) & 4095]) & 0xFFFF;
                    next[i] = head[h];
                    prev[i] = -1;
                    if (head[h] != -1) {
                        prev[head[h]] = i;
                    }
                    head[h] = i;
                    currentHash[i] = h;
                }

                // 自雜湊表中移除滑動視窗過期節點的輔助方法
                static void RemoveNode(int idx, int[] currentHash, int[] prev, int[] next, int[] head) {
                    int oldHash = currentHash[idx];
                    if (oldHash != -1) {
                        int p = prev[idx];
                        int n = next[idx];
                        if (p != -1) {
                            next[p] = n;
                        } else {
                            head[oldHash] = n;
                        }
                        if (n != -1) {
                            prev[n] = p;
                        }
                        currentHash[idx] = -1;
                        next[idx] = -1;
                        prev[idx] = -1;
                    }
                }

                // 向雜湊表中插入新節點的輔助方法
                static void InsertNode(int idx, byte[] win, int[] next, int[] head, int[] prev, int[] currentHash) {
                    int h = ((win[idx] << 8) ^ (win[(idx + 1) & 4095] << 4) ^ win[(idx + 2) & 4095]) & 0xFFFF;
                    next[idx] = head[h];
                    prev[idx] = -1;
                    if (head[h] != -1) {
                        prev[head[h]] = idx;
                    }
                    head[h] = idx;
                    currentHash[idx] = h;
                }

                // 更新滑動視窗字元並重新維護雜湊表的輔助方法
                static void UpdateWindow(int idx, byte val, byte[] win, int[] currentHash, int[] prev, int[] next, int[] head) {
                    int p2 = (idx - 2) & 4095;
                    int p1 = (idx - 1) & 4095;
                    int p0 = idx;
                    RemoveNode(p2, currentHash, prev, next, head);
                    RemoveNode(p1, currentHash, prev, next, head);
                    RemoveNode(p0, currentHash, prev, next, head);
                    win[idx] = val;
                    InsertNode(p2, win, next, head, prev, currentHash);
                    InsertNode(p1, win, next, head, prev, currentHash);
                    InsertNode(p0, win, next, head, prev, currentHash);
                }

                // 開始執行壓縮程序
                while (src < len) {
                    int matchKey = 0;
                    int matchLen = 0;
                    int limit = Math.Min(F, len - src); // 當前剩餘最大比對長度限制
                    if (limit >= 3) {
                        // 計算當前待匹配資料的雜湊值
                        int hash = ((input[src] << 8) ^ (input[src + 1] << 4) ^ input[src + 2]) & 0xFFFF;
                        int i = head[hash];
                        int depth = 0;
                        // 在雜湊鏈中搜尋重複字串，限制最大搜尋深度 256 以避免效能卡頓
                        while (i != -1 && depth < 256) {
                            depth++;
                            int distToR = (r - i) & 4095;
                            if (distToR == 0) distToR = 4096;
                            if (distToR >= 3) {
                                // 若首 3 字元匹配成功，進行更深層的比對
                                if (win[i] == input[src] && win[(i + 1) & 4095] == input[src + 1] && win[(i + 2) & 4095] == input[src + 2]) {
                                    int match = 3;
                                    while (match < limit) {
                                        byte expectedByte;
                                        if (match < distToR) {
                                            expectedByte = win[(i + match) & 4095];
                                        } else {
                                            // 支援自我重疊匹配（類似 RLE 壓解縮機制）
                                            expectedByte = input[src + (match % distToR)];
                                        }
                                        if (expectedByte != input[src + match]) break;
                                        match++;
                                    }
                                    // 若比對長度更長，更新最佳匹配位置與長度
                                    if (match > matchLen) {
                                        matchKey = i;
                                        matchLen = match;
                                        if (matchLen == limit) break; // 達到最大匹配長度則提早結束搜尋
                                    }
                                }
                            }
                            i = next[i];
                        }
                        // 針對極短匹配進行 1~2 位移的比對優化
                        if (matchLen < limit) {
                            for (int d = 1; d <= 2; d++) {
                                int idx = (r - d) & 4095;
                                if (win[idx] == input[src]) {
                                    int match = 1;
                                    int distToR = d;
                                    while (match < limit) {
                                        byte expectedByte;
                                        if (match < distToR) {
                                            expectedByte = win[(idx + match) & 4095];
                                        } else {
                                            expectedByte = input[src + (match % distToR)];
                                        }
                                        if (expectedByte != input[src + match]) break;
                                        match++;
                                    }
                                    if (match > matchLen) {
                                        matchKey = idx;
                                        matchLen = match;
                                        if (matchLen == limit) break;
                                    }
                                }
                            }
                        }
                    }
                    // 若匹配長度未達門檻，則直接輸出原始字元
                    if (matchLen <= threshold) {
                        matchLen = 1;
                        codeBuf[0] |= (byte)mask; // 將控制旗標的對應位元設為 1
                        codeBuf[codePtr++] = input[src];
                        UpdateWindow(r, input[src], win, currentHash, prev, next, head);
                        r = (r + 1) & 4095;
                        src++;
                    } else {
                        // 若匹配成功，寫入字典指標（2 位元組，內含位移與匹配長度）
                        int offset = matchKey;
                        int length = matchLen;
                        byte b1 = (byte)(offset & 0xFF);
                        byte b2 = (byte)(((offset >> 4) & 0xF0) | ((length - 3) & 0x0F));
                        codeBuf[codePtr++] = b1;
                        codeBuf[codePtr++] = b2;
                        for (int k = 0; k < length; k++) {
                            UpdateWindow(r, input[src + k], win, currentHash, prev, next, head);
                            r = (r + 1) & 4095;
                        }
                        src += length;
                    }
                    mask <<= 1;
                    // 若控制旗標累積滿 8 個，將緩衝區寫入輸出串流並重設旗標
                    if (mask == 256) {
                        for (int i = 0; i < codePtr; i++) output.Add(codeBuf[i]);
                        codeBuf[0] = 0;
                        codePtr = 1;
                        mask = 1;
                    }
                }
                // 處理最後殘留未滿 8 個的控制旗標緩衝區
                if (mask != 1) {
                    for (int i = 0; i < codePtr; i++) output.Add(codeBuf[i]);
                }
                return output.ToArray();
            } finally {
                ArrayPool<byte>.Shared.Return(win);
                ArrayPool<int>.Shared.Return(head);
                ArrayPool<int>.Shared.Return(next);
                ArrayPool<int>.Shared.Return(prev);
                ArrayPool<int>.Shared.Return(currentHash);
            }
        }

        // 解壓縮 PFIL 自訂格式檔案：檢查是否有 "PFIL" 標頭，有的話呼叫 Decompress 還原
        public static byte[] DecompressPfil(byte[] data) {
            if (data.Length < 64 || data[0] != 'P' || data[1] != 'F' || data[2] != 'I' || data[3] != 'L') {
                return data; // 若無 "PFIL" 標頭，視為未壓縮原始資料直接回傳
            }
            return Decompress(data);
        }

        // 壓縮 PFIL 自訂格式檔案：執行壓縮並在頭部填寫正確的 "PFIL" 64位元組檔案標頭與解壓後檔案大小
        public static byte[] CompressPfil(byte[] inputBytes, byte[] origHeader) {
            ArgumentNullException.ThrowIfNull(inputBytes);
            ArgumentNullException.ThrowIfNull(origHeader);
            if (origHeader.Length < 64) {
                throw new ArgumentException("PFIL header must contain at least 64 bytes.", nameof(origHeader));
            }

            byte[] compressed = Compress(inputBytes);
            byte[] header = new byte[64];
            Array.Copy(origHeader, 0, header, 0, 64);
            // 將解壓縮後的真實大小寫入標頭中（第 16 到 19 位元組）
            byte[] sizeBytes = BitConverter.GetBytes(inputBytes.Length);
            Array.Copy(sizeBytes, 0, header, 16, 4);
            byte[] outBytes = new byte[64 + compressed.Length];
            Array.Copy(header, 0, outBytes, 0, 64);
            Array.Copy(compressed, 0, outBytes, 64, compressed.Length);
            return outBytes;
        }
    }
}
