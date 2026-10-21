// 以明確 VA 反編譯指定功能；只修改暫存分析資料庫，不寫入輸入檔。
// @category AgainstRome
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class GhidraFeatureVerification extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String[] addresses = getScriptArgs();
        if (addresses.length == 0) {
            throw new IllegalArgumentException("必須提供至少一個明確 VA");
        }
        DecompInterface decompiler = new DecompInterface();
        try {
            if (!decompiler.openProgram(currentProgram)) {
                throw new IllegalStateException("無法開啟分析程式");
            }
            for (String address : addresses) {
                if (!address.matches("[0-9a-fA-F]{6,8}")) {
                    throw new IllegalArgumentException("VA 格式錯誤：" + address);
                }
                Address target = toAddr(address);
                Function function = getFunctionContaining(target);
                if (function == null) {
                    disassemble(target);
                    function = createFunction(target, "FUN_" + address);
                }
                if (function == null) {
                    throw new IllegalStateException("無法建立函式：" + address);
                }
                DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
                if (!result.decompileCompleted() || result.getDecompiledFunction() == null) {
                    throw new IllegalStateException("反編譯失敗：" + address + " " + result.getErrorMessage());
                }
                println("功能驗證 VA=" + address + " ENTRY=" + function.getEntryPoint());
                println(result.getDecompiledFunction().getC());
            }
            println("功能驗證全部完成：" + addresses.length);
        } finally {
            decompiler.dispose();
        }
    }
}
