// Ghidra headless script for cl_script.ini and script command callbacks.
// Run with analyzeHeadless -postScript GhidraScriptIniAnalysis.java
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
import java.util.HashSet;
import java.util.Set;

public class GhidraScriptIniAnalysis extends GhidraScript {
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

    private void decompile(Function f) {
        try {
            DecompInterface ifc = new DecompInterface();
            ifc.openProgram(currentProgram);
            DecompileResults res = ifc.decompileFunction(f, 60, monitor);
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
        Set<Function> funcs = new HashSet<>();
        String[] strings = {
            "SYSTEM/cl_script.ini", "cl_script.ini",
            "Radius", "CiviDelay",
            "MoralsDecLostMem", "MoralsDecFlee", "MoralsDecOverPop", "MoralsIncIdle",
            "Spell01", "Spell1"
        };
        for (String label : strings) refsForString(label, funcs);

        println("\nCandidate functions: " + funcs.size());
        for (Function f : funcs) println("  " + f.getName() + " @ " + f.getEntryPoint());
        for (Function f : funcs) decompile(f);
    }
}
