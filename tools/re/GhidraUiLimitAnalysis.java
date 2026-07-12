// Ghidra headless script for unit selection and UI limit candidates.
// Run with analyzeHeadless -postScript GhidraUiLimitAnalysis.java
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

import java.util.HashSet;
import java.util.Set;

public class GhidraUiLimitAnalysis extends GhidraScript {
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

    private void addCallers(String hex, Set<Function> funcs) throws Exception {
        Function target = getFunctionContaining(toAddr(hex));
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
        String[][] candidates = {
            {"0052a110", "SCRIPT_createBattleUnitsMax"},
            {"0052a140", "SCRIPT_createCiviUnitsMax"},
            {"005249d0", "Impl_createBattleUnitsMax"},
            {"00524d70", "Impl_createCiviUnitsMax"},
            {"0052a170", "SCRIPT_unitMemsWeaponMax"},
            {"005251d0", "Impl_unitMemsWeaponMax"},
            {"0052a3e0", "SCRIPT_getNotHorseUnitMems"},
            {"00527110", "Impl_getNotHorseUnitMems"},
            {"00523a00", "UnitCreationFromMembers"},
            {"00538320", "MemberGatherFilter"}
        };
        for (String[] item : candidates) addManual(item[0], item[1], funcs);
        for (String[] item : candidates) addCallers(item[0], funcs);

        println("\nCandidate functions: " + funcs.size());
        for (Function f : funcs) println("  " + f.getName() + " @ " + f.getEntryPoint());
        for (Function f : funcs) decompile(f);
    }
}
