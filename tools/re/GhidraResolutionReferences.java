// Ghidra headless script for locating every code reference to display-mode globals.
// Run after normal analysis with analyzeHeadless -postScript GhidraResolutionReferences.java <output-file>.
// @category AgainstRome

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

import java.io.PrintWriter;
import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraResolutionReferences extends GhidraScript {
    private PrintWriter output;

    private void emit(String value) {
        println(value);
        output.println(value);
    }

    private void printReferences(String address, String purpose) {
        Address target = toAddr(address);
        emit("\n===== " + purpose + " @ " + target + " =====");
        ReferenceIterator references = currentProgram.getReferenceManager().getReferencesTo(target);
        Set<Address> functions = new LinkedHashSet<>();
        while (references.hasNext()) {
            Reference reference = references.next();
            Address from = reference.getFromAddress();
            Function function = getFunctionContaining(from);
            emit(from + "  " + reference.getReferenceType() + "  " +
                (function == null ? "<no function>" : function.getName() + " @ " + function.getEntryPoint()));
            if (function != null) functions.add(function.getEntryPoint());
        }
        emit("Functions: " + functions);
    }

    private void printSymbolsMatching(String needle) {
        emit("\n===== symbols containing " + needle + " =====");
        SymbolIterator symbols = currentProgram.getSymbolTable().getAllSymbols(true);
        while (symbols.hasNext()) {
            Symbol symbol = symbols.next();
            if (!symbol.getName().toLowerCase().contains(needle.toLowerCase())) continue;
            emit(symbol.getName() + " @ " + symbol.getAddress());
            ReferenceIterator references = currentProgram.getReferenceManager().getReferencesTo(symbol.getAddress());
            while (references.hasNext()) {
                Reference reference = references.next();
                Function function = getFunctionContaining(reference.getFromAddress());
                emit("  " + reference.getFromAddress() + "  " + reference.getReferenceType() + "  " +
                    (function == null ? "<no function>" : function.getName() + " @ " + function.getEntryPoint()));
            }
        }
    }

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("Expected output-file path");
        output = new PrintWriter(args[0], "UTF-8");
        try {
            emit("Program: " + currentProgram.getName());
            printReferences("0061bf48", "selected display width");
            printReferences("0061bf4c", "selected display height");
            printReferences("00424560", "display width getter");
            printReferences("00424580", "display height getter");
            printReferences("00424760", "display mode switch");
            printReferences("00424c50", "dialog viewport adjustment");
            printReferences("00424b80", "persisted display mode getter used by IGM");
            printReferences("00441dc0", "IGM dialog-name chooser");
            printReferences("00443f60", "IGM path chooser");
            printReferences("006580bc", "selected display mode id");
            printReferences("006580b0", "persisted display mode id");
            printReferences("0064aa7c", "command-line default camera zoom");
            printReferences("02a28b40", "renderer/display object");
            printReferences("0065e0d4", "IGM 640x480 path");
            printReferences("0065e0d8", "IGM 800x600 path");
            printReferences("0065e0dc", "IGM 1024x768 path");
            printReferences("0065e0e0", "IGM 1280x1024 path");
            printReferences("0065e0e4", "IGM 1600x1200 path");
            printSymbolsMatching("igm16001200");
            printSymbolsMatching("igm12801024");
            printSymbolsMatching("resolution");
        } finally {
            output.close();
        }
    }
}
