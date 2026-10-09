// Static export analysis in a disposable Ghidra project; never load the DLL.
// @category AgainstRome
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Parameter;
import ghidra.program.model.listing.ParameterImpl;
import ghidra.program.model.listing.Function.FunctionUpdateType;
import ghidra.program.model.data.DWordDataType;
import ghidra.program.model.symbol.SourceType;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.nio.file.StandardOpenOption;

public class GhidraStreamBoundaryAnalysis extends GhidraScript {
    @Override protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1 || args.length > 2 || (args.length == 2 && !args[1].equals("helpers")))
            throw new IllegalArgumentException("Supply new output path and optional helpers scope");
        Path output = Path.of(args[0]).toAbsolutePath().normalize();
        if (Files.exists(output)) throw new IllegalArgumentException("Output exists");
        if (!currentProgram.getExecutableSHA256().equalsIgnoreCase(
                "dfdcdcf53a778cd5eeab5f53332748e77be97b29f480c36a7d1ca8d18cbbb17f"))
            throw new IllegalArgumentException("Unsupported DLL fingerprint");
        StringBuilder report = new StringBuilder("SHA256: " + currentProgram.getExecutableSHA256() + "\n");
        // Instruction evidence: Play forwards 12 caller arguments; PlaySnd 4;
        // both push 19 arguments to 10001c50 (76-byte argument cleanup).
        String[] signatures = {"10001bd0", "100028c0", "10001c50"};
        int[] counts = {12, 4, 19};
        for (int i = 0; i < signatures.length; i++) {
            Function function = getFunctionAt(toAddr(signatures[i]));
            if (function == null) throw new IllegalStateException("Missing ABI target " + signatures[i]);
            Parameter[] params = new Parameter[counts[i]];
            for (int p = 0; p < params.length; p++)
                params[p] = new ParameterImpl("arg" + (p + 1), DWordDataType.dataType, currentProgram);
            function.setCallingConvention("__cdecl");
            function.replaceParameters(FunctionUpdateType.DYNAMIC_STORAGE_ALL_PARAMS, true,
                SourceType.USER_DEFINED, params);
            report.append("ABI ").append(signatures[i]).append(" arguments=").append(counts[i]).append("\n");
        }
        String[] entries = {"10001260", "10001890", "10001bd0", "100025a0",
            "10002730", "100028c0", "10002920", "10002940", "100029b0",
            "100029d0", "10002a00", "10002a20"};
        if (args.length == 2) entries = new String[] {"10001000", "10001230",
            "10001370", "100018a0", "10001c50", "100025c0", "10002740"};
        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        try {
            for (String entry : entries) {
                Function function = getFunctionAt(toAddr(entry));
                if (function == null) {
                    disassemble(toAddr(entry));
                    function = createFunction(toAddr(entry), null);
                }
                report.append("\nENTRY ").append(entry).append("\n");
                if (function == null) { report.append("UNKNOWN\n"); continue; }
                DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
                if (result.decompileCompleted() && result.getDecompiledFunction() != null)
                    report.append(result.getDecompiledFunction().getC());
                else report.append("FAILED: ").append(result.getErrorMessage()).append("\n");
            }
        } finally { decompiler.dispose(); }
        Files.writeString(output, report.toString(), StandardCharsets.UTF_8, StandardOpenOption.CREATE_NEW);
        println("Saved static stream export evidence: " + output);
    }
}
