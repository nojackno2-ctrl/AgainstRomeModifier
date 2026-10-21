// 明確 VA、listing:VA 或 data-xref:VA[:十六進位長度]；只修改暫存分析資料庫。
// @category AgainstRome
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressIterator;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.Reference;
import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraFeatureVerification extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String[] addresses = getScriptArgs();
        if (addresses.length == 0) {
            throw new IllegalArgumentException("必須提供至少一個明確 VA");
        }
        Set<Function> functions = new LinkedHashSet<>();
        Set<Function> listings = new LinkedHashSet<>();
        for (String argument : addresses) {
            // Windows 批次檔會拆開未跳脫的等號；冒號形式可直接傳入 analyzeHeadless.bat。
            boolean dataXref = argument.startsWith("data-xref=") || argument.startsWith("data-xref:");
            boolean listing = argument.startsWith("listing:");
            String addressArgument = dataXref ? argument.substring("data-xref=".length())
                : listing ? argument.substring("listing:".length()) : argument;
            String[] parts = addressArgument.split(":", -1);
            String address = parts[0];
            if (parts.length > (dataXref ? 2 : 1)) {
                throw new IllegalArgumentException("參數格式錯誤：" + argument);
            }
            if (!address.matches("[0-9a-fA-F]{6,8}")) {
                throw new IllegalArgumentException("VA 格式錯誤：" + argument);
            }
            Address target = toAddr(address);
            if (dataXref) {
                int count = 0;
                long length = parts.length == 2 ? Long.parseLong(parts[1], 16) : 1;
                if (length < 1 || length > 0x100000) {
                    throw new IllegalArgumentException("資料引用範圍錯誤：" + argument);
                }
                AddressIterator destinations = currentProgram.getReferenceManager()
                    .getReferenceDestinationIterator(new AddressSet(target, target.add(length - 1)), true);
                while (destinations.hasNext()) {
                    Address destination = destinations.next();
                    for (Reference reference : getReferencesTo(destination)) {
                        Function function = getFunctionContaining(reference.getFromAddress());
                        println("資料引用 TARGET=" + destination + " FROM=" + reference.getFromAddress()
                            + " TYPE=" + reference.getReferenceType() + " ENTRY="
                            + (function == null ? "無函式" : function.getEntryPoint())
                            + " INSTRUCTION=" + getInstructionAt(reference.getFromAddress()));
                        if (function != null) functions.add(function);
                        count++;
                    }
                }
                // Ghidra 未必替暫存器索引的絕對位移建立 data reference；補列實際指令運算元。
                InstructionIterator instructions = currentProgram.getListing().getInstructions(true);
                while (instructions.hasNext()) {
                    monitor.checkCancelled();
                    Instruction instruction = instructions.next();
                    boolean matches = false;
                    for (int operand = 0; operand < instruction.getNumOperands(); operand++) {
                        for (Object object : instruction.getOpObjects(operand)) {
                            if (object instanceof Scalar) {
                                long value = ((Scalar) object).getUnsignedValue();
                                matches |= value >= target.getOffset() && value < target.getOffset() + length;
                            }
                        }
                    }
                    if (matches) {
                        Function function = getFunctionContaining(instruction.getAddress());
                        println("資料位移 FROM=" + instruction.getAddress() + " ENTRY="
                            + (function == null ? "無函式" : function.getEntryPoint())
                            + " INSTRUCTION=" + instruction);
                        if (function != null) functions.add(function);
                        count++;
                    }
                }
                if (count == 0) throw new IllegalStateException("查無資料引用：" + address);
                println("資料引用與位移命中數量（可重疊）：" + count);
            } else {
                Function function = getFunctionContaining(target);
                if (function == null) {
                    disassemble(target);
                    function = createFunction(target, "FUN_" + address);
                }
                if (function == null) throw new IllegalStateException("無法建立函式：" + address);
                functions.add(function);
                if (listing) listings.add(function);
            }
        }
        DecompInterface decompiler = new DecompInterface();
        try {
            if (!decompiler.openProgram(currentProgram)) {
                throw new IllegalStateException("無法開啟分析程式");
            }
            for (Function function : functions) {
                String address = function.getEntryPoint().toString();
                DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
                if (!result.decompileCompleted() || result.getDecompiledFunction() == null) {
                    throw new IllegalStateException("反編譯失敗：" + address + " " + result.getErrorMessage());
                }
                println("功能驗證 VA=" + address + " ENTRY=" + function.getEntryPoint());
                println(result.getDecompiledFunction().getC());
                if (listings.contains(function)) {
                    InstructionIterator instructions = currentProgram.getListing().getInstructions(function.getBody(), true);
                    while (instructions.hasNext()) {
                        Instruction instruction = instructions.next();
                        println("指令 VA=" + instruction.getAddress() + " " + instruction);
                    }
                }
            }
            println("功能驗證全部完成：" + functions.size());
        } finally {
            decompiler.dispose();
        }
    }
}
