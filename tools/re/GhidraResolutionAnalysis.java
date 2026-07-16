// Ghidra headless script for native display-mode and resolution-specific UI analysis.
// Run with analyzeHeadless -postScript GhidraResolutionAnalysis.java
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.util.NumericUtilities;

import java.io.PrintWriter;

public class GhidraResolutionAnalysis extends GhidraScript {
    private PrintWriter output;

    private void emit(String value) {
        println(value);
        output.println(value);
    }

    private void printFunction(String address, String purpose) throws Exception {
        Address entry = toAddr(address);
        Function function = getFunctionAt(entry);
        if (function == null) function = getFunctionContaining(entry);
        if (function == null) {
            disassemble(entry);
            function = createFunction(entry, "ResolutionAnalysis_" + address);
        }
        if (function == null) {
            emit("Missing function for " + purpose + " at " + address);
            return;
        }

        emit("\n===== " + purpose + " " + function.getName() + " @ " + function.getEntryPoint() + " =====");
        InstructionIterator instructions = currentProgram.getListing().getInstructions(function.getBody(), true);
        while (instructions.hasNext()) {
            Instruction instruction = instructions.next();
            byte[] bytes = instruction.getBytes();
            emit(instruction.getAddress() + "  " + NumericUtilities.convertBytesToString(bytes, " ") +
                "  " + instruction);
        }

        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
        emit("\n--- DECOMPILE ---");
        if (result != null && result.decompileCompleted() && result.getDecompiledFunction() != null)
            emit(result.getDecompiledFunction().getC());
        else
            emit("<decompile failed>");
        decompiler.dispose();
    }

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("Expected output-file path");
        output = new PrintWriter(args[0], "UTF-8");
        try {
            emit("Program: " + currentProgram.getName());
            emit("Image base: " + currentProgram.getImageBase());
            printFunction("00424590", "display-mode identification and selected width/height");
            printFunction("00424760", "display-mode switch");
            printFunction("00424c50", "resolution-specific dialog viewport adjustment");
            printFunction("00426450", "resolution-specific IGM resource selection");
            printFunction("005a55e0", "renderer resize called with selected width and height");
            printFunction("00424480", "post-mode global display refresh");
            printFunction("00443b10", "post-mode scene refresh");
            printFunction("004555a0", "post-mode UI refresh");
        } finally {
            output.close();
        }
    }
}
