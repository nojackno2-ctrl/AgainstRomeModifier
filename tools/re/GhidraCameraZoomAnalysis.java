// Focused Ghidra headless script for the native camera zoom path.
// Run with analyzeHeadless -postScript GhidraCameraZoomAnalysis.java <output-file>.
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.util.NumericUtilities;

import java.io.PrintWriter;

public class GhidraCameraZoomAnalysis extends GhidraScript {
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
            emit("Missing function for " + purpose + " at " + address);
            return;
        }
        emit("\n===== " + purpose + " " + function.getName() + " @ " + function.getEntryPoint() + " =====");
        InstructionIterator instructions = currentProgram.getListing().getInstructions(function.getBody(), true);
        while (instructions.hasNext()) {
            Instruction instruction = instructions.next();
            emit(instruction.getAddress() + "  " +
                NumericUtilities.convertBytesToString(instruction.getBytes(), " ") + "  " + instruction);
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

    private void printReferences(String address, String purpose) {
        Address target = toAddr(address);
        emit("\n===== references to " + purpose + " @ " + target + " =====");
        ReferenceIterator references = currentProgram.getReferenceManager().getReferencesTo(target);
        while (references.hasNext()) {
            Reference reference = references.next();
            Function function = getFunctionContaining(reference.getFromAddress());
            emit(reference.getFromAddress() + "  " + reference.getReferenceType() + "  " +
                (function == null ? "<no function>" : function.getName() + " @ " + function.getEntryPoint()));
        }
    }

    private void printBytes(String address, int length, String purpose) throws Exception {
        Address start = toAddr(address);
        byte[] bytes = new byte[length];
        currentProgram.getMemory().getBytes(start, bytes);
        emit("\n===== " + purpose + " @ " + start + " length " + length + " =====");
        for (int offset = 0; offset < length; offset += 16) {
            int count = Math.min(16, length - offset);
            byte[] row = new byte[count];
            System.arraycopy(bytes, offset, row, 0, count);
            emit(start.add(offset) + "  " + NumericUtilities.convertBytesToString(row, " "));
        }
    }

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("Expected output-file path");
        output = new PrintWriter(args[0], "UTF-8");
        try {
            emit("Program: " + currentProgram.getName());
            emit("Image base: " + currentProgram.getImageBase());
            printFunction("00498a30", "camera zoom setter and clamp");
            printFunction("00480850", "engine initialization including default zoom");
            printFunction("0048d6e0", "game-state load including saved zoom");
            printFunction("004933f0", "camera projection consuming zoom");
            printFunction("0054c1f0", "mission-script zoom setter wrapper");
            printReferences("00498a30", "camera zoom setter");
            printReferences("00771800", "camera zoom scalar");
            printFunction("00419cc0", "positive float to integer zoom conversion");
            printBytes("00612c70", 4, "zoom integer-conversion bias constant");
            String[] consumers = {
                "004af270", "004c5f60", "00487f70", "00491cd0", "00491d80",
                "00498a90", "0049d710", "0049dde0", "0049e7b0", "004b4260",
                "004b50c0", "00495710", "004c3aa0", "004c31d0", "004c90c0",
                "004ca010", "004ca340", "004ca740", "004cab80"
            };
            for (String consumer : consumers)
                printFunction(consumer, "camera zoom scalar consumer");
            printBytes("00562570", 256, "existing executable code-cave neighborhood");
        } finally {
            output.close();
        }
    }
}
