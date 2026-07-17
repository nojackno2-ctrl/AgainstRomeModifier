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
            printFunction("00422130", "display size consumer A");
            printFunction("0045a620", "display size consumer B");
            printFunction("0045ff20", "display size consumer C");
            printFunction("00470300", "display size consumer D");
            printFunction("00424730", "display width-height helper");
            printFunction("004251c0", "display mode dependent resource selection");
            printFunction("00429350", "resolution-specific IGM path setup");
            printFunction("00443f60", "resolution-specific IGM path chooser");
            printFunction("00443720", "IGM dialog registration callback");
            printFunction("00441e80", "shared IGM/dialog viewport setup");
            printFunction("00443ac0", "IGM reload check");
            printFunction("00443ad0", "IGM reload trigger");
            printFunction("00427ff0", "resolution dialog construction and callbacks");
            printFunction("00424700", "current display mode getter");
            printFunction("00427ee0", "resolution dialog initializer");
            printFunction("00427d60", "resolution dialog mode callbacks");
            printFunction("00463350", "options resolution persistence");
            printFunction("00424b80", "persisted resolution mode getter");
            printFunction("00424bd0", "persisted resolution mode setter");
        } finally {
            output.close();
        }
    }
}
