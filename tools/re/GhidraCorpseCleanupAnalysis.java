// Ghidra headless helper for finding corpse lifecycle and cleanup candidates.
// Run with analyzeHeadless -postScript GhidraCorpseCleanupAnalysis.java
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraCorpseCleanupAnalysis extends GhidraScript {
    private final Set<Function> candidates = new LinkedHashSet<>();

    private void add(Function function, String reason) {
        if (function != null && candidates.add(function)) {
            println("CANDIDATE " + function.getName() + " @ " + function.getEntryPoint() + " :: " + reason);
        }
    }

    private void addManual(String address, String reason) {
        Address target = toAddr(address);
        Function function = getFunctionContaining(target);
        if (function == null) {
            try {
                function = createFunction(target, "Manual_" + address);
            } catch (Exception ignored) {
                // A label may be inside an already-defined function; the caller
                // still gets all recognized function-level evidence.
            }
        }
        add(function, reason);
    }

    private void addCallers(String address, String reason) {
        Function target = getFunctionContaining(toAddr(address));
        if (target == null) return;
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(target.getEntryPoint());
        while (refs.hasNext()) {
            Reference ref = refs.next();
            add(getFunctionContaining(ref.getFromAddress()), reason + " from " + ref.getFromAddress());
        }
    }

    private void addStringRefs(String text) {
        SymbolIterator symbols = currentProgram.getSymbolTable().getSymbolIterator(text, true);
        while (symbols.hasNext()) {
            Symbol symbol = symbols.next();
            println("STRING " + text + " @ " + symbol.getAddress());
            ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(symbol.getAddress());
            while (refs.hasNext()) {
                Reference ref = refs.next();
                add(getFunctionContaining(ref.getFromAddress()), "reference to " + text + " from " + ref.getFromAddress());
            }
        }
    }

    private void scanDeadFlag() {
        InstructionIterator instructions = currentProgram.getListing().getInstructions(true);
        while (instructions.hasNext() && !monitor.isCancelled()) {
            Instruction instruction = instructions.next();
            for (Object object : instruction.getOpObjects(0)) {
                if (object instanceof Scalar && ((Scalar) object).getUnsignedValue() == 0x2000) {
                    add(getFunctionContaining(instruction.getAddress()), "uses death flag 0x2000 at " + instruction.getAddress());
                }
            }
            for (Object object : instruction.getOpObjects(1)) {
                if (object instanceof Scalar && ((Scalar) object).getUnsignedValue() == 0x2000) {
                    add(getFunctionContaining(instruction.getAddress()), "uses death flag 0x2000 at " + instruction.getAddress());
                }
            }
        }
    }

    private void decompile(Function function) {
        DecompInterface decompiler = new DecompInterface();
        try {
            decompiler.openProgram(currentProgram);
            DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
            println("\n===== DECOMPILE " + function.getName() + " @ " + function.getEntryPoint() + " =====");
            println(result != null && result.decompileCompleted() && result.getDecompiledFunction() != null
                ? result.getDecompiledFunction().getC() : "<decompile failed>");
        } finally {
            decompiler.dispose();
        }
    }

    @Override
    protected void run() throws Exception {
        addStringRefs("s_searchDeadObjsPos");
        addManual("0051a560", "s_setDeadTime native binding");
        addCallers("0051a560", "caller of s_setDeadTime native binding");
        addManual("004abb80", "full object-release routine");
        addCallers("004abb80", "caller of full object-release routine");
        addManual("004acb00", "deferred object-release scheduler");
        addCallers("004acb00", "caller of deferred object-release scheduler");
        scanDeadFlag();
        println("\nTotal candidates: " + candidates.size());
        for (Function candidate : candidates) decompile(candidate);
    }
}
