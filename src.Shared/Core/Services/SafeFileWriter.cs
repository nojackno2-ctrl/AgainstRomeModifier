using System;
using System.IO;

namespace AgainstRomeModifier.Core.Services
{
    /// <summary>
    /// 交易式檔案寫入的共用工具：寫入 temp 檔後以 File.Replace 原子替換，
    /// 失敗時重試；目標檔內容與待寫入內容完全相同時直接跳過（不碰檔案、不建立回復點），
    /// 避免每次套用都把未變更的檔案全部重寫。
    /// 此類別是 PatchEngine 與 EndlessAiOrchestrator 先前各自持有之相同實作的唯一正本。
    /// </summary>
    public static class SafeFileWriter
    {
        private const int MaxRetries = 3;
        private const int RetryDelayMs = 500;

        public static void WriteAllBytes(string dest, byte[] bytes, FileRollbackScope? rollback = null)
        {
            string? dir = Path.GetDirectoryName(dest);
            if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
            {
                Directory.CreateDirectory(dir);
            }

            if (ContentAlreadyMatches(dest, bytes))
            {
                return;
            }

            for (int i = 0; i < MaxRetries; i++)
            {
                string tempFile = Path.Combine(dir ?? AppContext.BaseDirectory, Path.GetFileName(dest) + "." + Guid.NewGuid().ToString("N") + ".tmp");
                try
                {
                    rollback?.TrackFile(dest);
                    File.WriteAllBytes(tempFile, bytes);

                    if (File.Exists(dest))
                    {
                        File.SetAttributes(dest, FileAttributes.Normal);
                        File.Replace(tempFile, dest, null, true);
                    }
                    else
                    {
                        File.Move(tempFile, dest);
                    }
                    return;
                }
                catch (Exception ex) when (ex is IOException || ex is UnauthorizedAccessException)
                {
                    TryDeleteTempFile(tempFile);
                    if (i == MaxRetries - 1)
                    {
                        throw new IOException(string.Format("寫入檔案失敗，檔案可能被佔用或權限不足：{0}。錯誤訊息：{1}", dest, ex.Message), ex);
                    }
                    System.Threading.Thread.Sleep(RetryDelayMs);
                }
            }
        }

        /// <summary>刪除檔案（先登記回復點、清除唯讀屬性）；檔案不存在時無動作。</summary>
        public static void DeleteFile(string path, FileRollbackScope? rollback = null)
        {
            if (!File.Exists(path)) return;
            rollback?.TrackFile(path);
            File.SetAttributes(path, FileAttributes.Normal);
            File.Delete(path);
        }

        /// <summary>以交易式寫入複製檔案；overwrite 為 false 且目標已存在時拋出。</summary>
        public static void CopyFile(string source, string destination, bool overwrite, FileRollbackScope? rollback = null)
        {
            if (!overwrite && File.Exists(destination))
                throw new IOException("目標檔案已存在: " + destination);
            WriteAllBytes(destination, File.ReadAllBytes(source), rollback);
        }

        private static bool ContentAlreadyMatches(string dest, byte[] bytes)
        {
            try
            {
                var info = new FileInfo(dest);
                if (!info.Exists || info.Length != bytes.Length) return false;
                byte[] existing = File.ReadAllBytes(dest);
                return existing.AsSpan().SequenceEqual(bytes);
            }
            catch (Exception ex) when (ex is IOException || ex is UnauthorizedAccessException)
            {
                // 讀不到就照常走寫入流程，由寫入端處理錯誤。
                return false;
            }
        }

        private static void TryDeleteTempFile(string tempFile)
        {
            try
            {
                if (File.Exists(tempFile))
                {
                    File.SetAttributes(tempFile, FileAttributes.Normal);
                    File.Delete(tempFile);
                }
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine("[SafeFileWriter] 清理暫存檔失敗 " + tempFile + ": " + ex.Message);
            }
        }
    }
}
