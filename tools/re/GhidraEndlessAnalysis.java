// Ghidra headless script for Against Rome endless-mode spawn analysis.
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
import ghidra.program.model.symbol.SourceType;

import java.nio.charset.StandardCharsets;
import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraEndlessAnalysis extends GhidraScript {
    private Address findBytes(byte[] needle) throws Exception {
        Memory memory = currentProgram.getMemory();
        for (MemoryBlock block : memory.getBlocks()) {
            if (!block.isInitialized()) continue;
            Address cur = block.getStart();
            Address end = block.getEnd();
            while (cur.compareTo(end) <= 0) {
                Address found = memory.findBytes(cur, end, needle, null, true, monitor);
                if (found == null) break;
                return found;
            }
        }
        return null;
    }

    private void collectRefs(String label, Set<Function> funcs) throws Exception {
        Address target = findBytes(label.getBytes(StandardCharsets.US_ASCII));
        println("\nSTRING " + label + " -> " + target);
        if (target == null) return;
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(target);
        int count = 0;
        while (refs.hasNext()) {
            Reference ref = refs.next();
            count++;
            Function f = getFunctionContaining(ref.getFromAddress());
            if (f != null) funcs.add(f);
            println("  ref from " + ref.getFromAddress() + " type=" + ref.getReferenceType()
                + " func=" + (f == null ? "<none>" : f.getName() + "@" + f.getEntryPoint()));
        }
        if (count == 0) println("  no refs");
    }

    private void addFunction(String address, String name, Set<Function> funcs) throws Exception {
        Address addr = toAddr(address);
        Function f = getFunctionContaining(addr);
        if (f == null) {
            try {
                f = createFunction(addr, name);
            } catch (Exception ex) {
                println("Could not create function at " + address + ": " + ex.getMessage());
            }
        }
        if (f != null) {
            funcs.add(f);
            if (name != null && name.length() > 0 && f.getName().startsWith("FUN_")) {
                try {
                    f.setName(name, SourceType.USER_DEFINED);
                } catch (Exception ignored) {
                }
            }
            println("MANUAL " + address + " -> " + f.getName() + "@" + f.getEntryPoint());
        }
    }

    private void decompile(Function f) {
        DecompInterface ifc = new DecompInterface();
        try {
            ifc.openProgram(currentProgram);
            DecompileResults res = ifc.decompileFunction(f, 60, monitor);
            println("\n===== DECOMPILE " + f.getName() + " @ " + f.getEntryPoint() + " =====");
            if (res != null && res.decompileCompleted() && res.getDecompiledFunction() != null) {
                println(res.getDecompiledFunction().getC());
            } else {
                println("<decompile failed>");
            }
        } catch (Exception ex) {
            println("<decompile exception: " + ex.getMessage() + ">");
        } finally {
            ifc.dispose();
        }
    }

    @Override
    protected void run() throws Exception {
        Set<Function> funcs = new LinkedHashSet<>();

        String[] strings = new String[] {
            "mscr_endlosspiel",
            "endl_load",
            "ENDL_%03ld",
            "ak_level.bci",
            "Endlos_Ger_Siedlung1",
            "Endlos_Ger_Siedlung2",
            "Endlos_Kel_Siedlung1",
            "Endlos_Kel_Siedlung2",
            "Endlos_Hun_Siedlung1",
            "Endlos_Hun_Siedlung2",
            "Endlos_Rom_Siedlung1",
            "Endlos_Rom_Siedlung2",
            "s_createUnit",
            "s_createUnitAndMems",
            "s_createBattleUnitsMax",
            "s_createCiviUnitsMax"
        };
        for (String s : strings) collectRefs(s, funcs);

        addFunction("00529f90", "SCRIPT_createUnit", funcs);
        addFunction("0052a020", "SCRIPT_createUnitAndMems", funcs);
        addFunction("0052a110", "SCRIPT_createBattleUnitsMax", funcs);
        addFunction("0052a140", "SCRIPT_createCiviUnitsMax", funcs);
        addFunction("005249d0", "Impl_createBattleUnitsMax", funcs);
        addFunction("00524d70", "Impl_createCiviUnitsMax", funcs);

        println("\nCandidate functions: " + funcs.size());
        for (Function f : funcs) println("  " + f.getName() + " @ " + f.getEntryPoint());
        for (Function f : funcs) decompile(f);
    }
}
