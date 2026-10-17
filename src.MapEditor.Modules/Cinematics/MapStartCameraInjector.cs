using System.Buffers.Binary;
using AgainstRomeModifier.Core.Services;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Cinematics;

/// <summary>
/// 將開局相機設定編譯為 IPR 位元碼並注入地圖的 ak_level.bci 腳本。
/// </summary>
public static class MapStartCameraInjector
{
    private const int OpJump = 112;

    /// <summary>
    /// 在 BciImage 中注入開局相機 shim，並將 MainAddress 指向 shim。
    /// shim 依序呼叫開局相機 API 後，無條件跳躍回原 MainAddress。
    /// </summary>
    public static void Inject(BciImage image, MapStartCamera camera)
    {
        ArgumentNullException.ThrowIfNull(image);
        ArgumentNullException.ThrowIfNull(camera);
        if (!camera.Enabled) return;

        byte[] cameraCalls = CinematicBciCompiler.CompileStartCameraCalls(image, camera);
        int originalMain = image.MainAddress;
        int start = image.Code.Length;
        int jumpAt = start + cameraCalls.Length;
        int jumpOperand = originalMain - (jumpAt + 8);

        using var ms = new MemoryStream();
        ms.Write(cameraCalls);
        Span<byte> jumpBytes = stackalloc byte[8];
        BinaryPrimitives.WriteInt32LittleEndian(jumpBytes[..4], OpJump);
        BinaryPrimitives.WriteInt32LittleEndian(jumpBytes[4..], jumpOperand);
        ms.Write(jumpBytes);

        image.AppendCode(ms.ToArray());
        image.MainAddress = start;
    }

    /// <summary>
    /// 將開局相機注入至地圖的 ak_level.bci 檔案，並儲存 arm_start_camera.json。
    /// 若原版腳本備份 ak_level.arm_original 不存在，則先行備份。
    /// </summary>
    public static void Apply(string mapDirectory, MapStartCamera camera, FileRollbackScope? rollback = null)
    {
        ArgumentNullException.ThrowIfNull(mapDirectory);
        ArgumentNullException.ThrowIfNull(camera);

        string scriptDir = LevelScriptInjector.ScriptDirectory(mapDirectory);
        string scriptPath = Path.Combine(scriptDir, LevelScriptInjector.ScriptFile);
        string backupPath = Path.Combine(scriptDir, LevelScriptInjector.OriginalBackupFile);

        if (!File.Exists(backupPath))
        {
            if (!camera.Enabled)
            {
                camera.Save(mapDirectory, rollback);
                return;
            }
            if (!File.Exists(scriptPath))
                throw new FileNotFoundException("地圖缺少 ak_level.bci，無法注入開局相機。", scriptPath);
            if (rollback != null)
                SafeFileWriter.WriteAllBytes(backupPath, File.ReadAllBytes(scriptPath), rollback);
            else
                File.WriteAllBytes(backupPath, File.ReadAllBytes(scriptPath));
        }

        byte[] scriptBytes = File.ReadAllBytes(scriptPath);
        bool pfil = scriptBytes.Length >= 64 && scriptBytes.AsSpan(0, 4).SequenceEqual("PFIL"u8);
        byte[] raw = pfil ? GameLZSS.DecompressPfil(scriptBytes) : scriptBytes;
        BciImage image = BciImage.Parse(raw);

        if (camera.Enabled)
        {
            Inject(image, camera);
            byte[] serialized = image.Serialize();
            byte[] output = pfil ? GameLZSS.CompressPfil(serialized, scriptBytes.AsSpan(0, 64).ToArray()) : serialized;
            if (rollback != null)
                SafeFileWriter.WriteAllBytes(scriptPath, output, rollback);
            else
                File.WriteAllBytes(scriptPath, output);
        }

        camera.Save(mapDirectory, rollback);
    }
}
