// Ghidra headless script: trace the village food-consumption HP healing chain.
// Round 1: find "LPIncIdle" (and related) string refs, decompile the parser to
// learn the per-tribe storage globals. Optional args: additional hex addresses
// of globals/functions to xref+decompile in the same run.
// Run with analyzeHeadless -postScript GhidraFoodHealAnalysis.java [addr...]
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

import java.nio.charset.StandardCharsets;
import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraFoodHealAnalysis extends GhidraScript {
    private Address findBytes(String value) throws Exception {
        Memory memory = currentProgram.getMemory();
        byte[] needle = value.getBytes(StandardCharsets.US_ASCII);
        for (MemoryBlock block : memory.getBlocks()) {
            if (!block.isInitialized()) continue;
            Address found = memory.findBytes(block.getStart(), block.getEnd(), needle, null, true, monitor);
            if (found != null) return found;
        }
        return null;
    }

    private void refsForString(String label, Set<Function> funcs) throws Exception {
        Address addr = findBytes(label);
        println("\nSTRING " + label + " -> " + addr);
        if (addr == null) return;
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(addr);
        while (refs.hasNext()) {
            Reference ref = refs.next();
            Function f = getFunctionContaining(ref.getFromAddress());
            println("  ref " + ref.getFromAddress() + " type=" + ref.getReferenceType() +
                " func=" + (f == null ? "<none>" : f.getName() + "@" + f.getEntryPoint()));
            if (f != null) funcs.add(f);
        }
    }

    private void refsForAddress(Address addr, Set<Function> funcs) {
        println("\nADDR " + addr);
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(addr);
        while (refs.hasNext()) {
            Reference ref = refs.next();
            Function f = getFunctionContaining(ref.getFromAddress());
            println("  ref " + ref.getFromAddress() + " type=" + ref.getReferenceType() +
                " func=" + (f == null ? "<none>" : f.getName() + "@" + f.getEntryPoint()));
            if (f != null) funcs.add(f);
        }
    }

    private void decompile(Function f) {
        try {
            DecompInterface ifc = new DecompInterface();
            ifc.openProgram(currentProgram);
            DecompileResults res = ifc.decompileFunction(f, 90, monitor);
            println("\n===== DECOMPILE " + f.getName() + " @ " + f.getEntryPoint() + " =====");
            if (res != null && res.decompileCompleted() && res.getDecompiledFunction() != null) {
                println(res.getDecompiledFunction().getC());
            } else {
                println("<decompile failed>");
            }
            ifc.dispose();
        } catch (Exception ex) {
            println("<decompile exception: " + ex.getMessage() + ">");
        }
    }

    @Override
    protected void run() throws Exception {
        Set<Function> funcs = new LinkedHashSet<>();
        String[] strings = { "LPIncIdle", "ManaIncIdle", "MoralsIncIdle" };
        for (String label : strings) refsForString(label, funcs);

        String[] args = getScriptArgs();
        for (String arg : args) {
            Address addr = toAddr(Long.parseLong(arg.replace("0x", ""), 16));
            Function f = getFunctionAt(addr);
            if (f != null) {
                funcs.add(f);
            } else {
                refsForAddress(addr, funcs);
            }
        }

        println("\nCandidate functions: " + funcs.size());
        for (Function f : funcs) println("  " + f.getName() + " @ " + f.getEntryPoint());
        for (Function f : funcs) decompile(f);
    }
}
