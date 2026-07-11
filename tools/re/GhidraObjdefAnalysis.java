// Ghidra headless script for objdef.dau field and unit-stat code paths.
// Run with analyzeHeadless -postScript GhidraObjdefAnalysis.java
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

public class GhidraObjdefAnalysis extends GhidraScript {
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

    private void addRefsForString(String label, Set<Function> funcs) throws Exception {
        Address addr = findBytes(label);
        println("\nSTRING " + label + " -> " + addr);
        if (addr == null) return;
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(addr);
        int count = 0;
        while (refs.hasNext()) {
            Reference ref = refs.next();
            Function f = getFunctionContaining(ref.getFromAddress());
            println("  ref " + ref.getFromAddress() + " type=" + ref.getReferenceType() +
                " func=" + (f == null ? "<none>" : f.getName() + "@" + f.getEntryPoint()));
            if (f != null) funcs.add(f);
            count++;
        }
        if (count == 0) println("  no refs");
    }

    private void addManual(String hex, String name, Set<Function> funcs) throws Exception {
        Address addr = toAddr(hex);
        Function f = getFunctionContaining(addr);
        if (f == null) {
            try {
                f = createFunction(addr, name);
            } catch (Exception ex) {
                println("Could not create function at " + hex + ": " + ex.getMessage());
            }
        }
        if (f != null) {
            funcs.add(f);
            println("MANUAL " + hex + " -> " + f.getName() + "@" + f.getEntryPoint());
        }
    }

    private void addCallers(Function target, Set<Function> funcs) {
        if (target == null) return;
        println("\nCALLERS " + target.getName() + " @ " + target.getEntryPoint());
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(target.getEntryPoint());
        while (refs.hasNext()) {
            Reference ref = refs.next();
            Function caller = getFunctionContaining(ref.getFromAddress());
            println("  caller ref " + ref.getFromAddress() + " type=" + ref.getReferenceType() +
                " func=" + (caller == null ? "<none>" : caller.getName() + "@" + caller.getEntryPoint()));
            if (caller != null) funcs.add(caller);
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
            "SYSTEM/DATA_MP/DEFAULTS/objdef.dau", "objdef.dau",
            "moves", "movsf", "sirad", "bmovs",
            "s_unitMemsWeaponMax", "s_getNotHorseUnitMems",
            "OD_PRODEQUI", "OD_PRODHORS"
        };
        for (String label : strings) addRefsForString(label, funcs);

        String[][] manual = {
            {"0052a170", "SCRIPT_unitMemsWeaponMax"},
            {"0052a3e0", "SCRIPT_getNotHorseUnitMems"},
            {"005251d0", "Impl_unitMemsWeaponMax"},
            {"00527110", "Impl_getNotHorseUnitMems"},
            {"00525210", "Helper_unitMemsWeaponMax"},
            {"00527160", "Helper_getNotHorseUnitMems"},
            {"00523a00", "UnitCreationFromMembers"},
            {"00538320", "MemberGatherFilter"},
            {"005298a0", "MemberValidityFilter"}
        };
        for (String[] item : manual) addManual(item[0], item[1], funcs);
        for (String[] item : manual) addCallers(getFunctionContaining(toAddr(item[0])), funcs);

        println("\nCandidate functions: " + funcs.size());
        for (Function f : funcs) println("  " + f.getName() + " @ " + f.getEntryPoint());
        for (Function f : funcs) decompile(f);
    }
}
