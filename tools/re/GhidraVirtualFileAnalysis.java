// Focused analysis of file backend function-pointer targets not recovered by
// default auto-analysis. Only the disposable Ghidra database is modified.
// @category AgainstRome
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraVirtualFileAnalysis extends GhidraScript {
    private StringBuilder report = new StringBuilder();
    private void emit(String value) { report.append(value).append("\n"); }
    @Override protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("Supply one new output file path");
        Path output = Path.of(args[0]).toAbsolutePath().normalize();
        if (Files.exists(output)) throw new IllegalArgumentException("Output already exists");
        emit("Input: " + currentProgram.getExecutablePath());
        emit("Executable SHA256: " + currentProgram.getExecutableSHA256());
        emit("Static decompilation; candidate entry points from EXE backend tables and instruction boundaries.");
        Set<Function> functions = new LinkedHashSet<>();
        // ZIP backend table 0x62ca8c (REA read_bytes/xrefs). Remaining targets
        // come from fileInit 0x57fb40 and its native callback registration.
        String[] entries = {"00552820", "005529d0", "00552a20", "00552b30", "00552b90",
            "00552bc0", "00552bd0", "00552d00", "00552d20", "0057f9a0", "0057fb40",
            "0057ff00", "005810b0", "005814b0", "00581710", "00580db0", "00580dc0",
            "00580df0", "00580e20", "00580ee0", "00582750", "00582940", "005645f0",
            "005801e0", "0057fa30", "00565180", "00572cf0", "005642e0", "005641f0",
            "00564b20", "00564d10", "00564fa0", "00565c00", "00585080"};
        for (String value : entries) {
            Address address = toAddr(value);
            Function function = getFunctionAt(address);
            boolean created = false;
            if (function == null) {
                disassemble(address);
                function = createFunction(address, null);
                created = function != null;
            }
            emit("ENTRY " + value + " created=" + created + " function=" + (function == null ? "unknown" : function.getName()));
            if (function != null) functions.add(function);
        }
        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        try {
            for (Function function : functions) {
                emit("\nFUNCTION " + function.getEntryPoint() + " " + function.getName() + " body=" + function.getBody());
                for (Reference ref : getReferencesTo(function.getEntryPoint()))
                    emit("XREF " + ref.getFromAddress() + " " + ref.getReferenceType());
                DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
                if (result.decompileCompleted() && result.getDecompiledFunction() != null)
                    emit(result.getDecompiledFunction().getC());
                else emit("DECOMPILE FAILED " + result.getErrorMessage());
            }
        } finally { decompiler.dispose(); }
        Files.writeString(output, report.toString(), StandardCharsets.UTF_8, java.nio.file.StandardOpenOption.CREATE_NEW);
        println("Saved focused file backend evidence: " + output);
    }
}
