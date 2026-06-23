// Ghidra headless script for equipment/unit-selection related functions.
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

public class GhidraEquipmentAnalysis extends GhidraScript {
    private Address findBytes(byte[] needle) throws Exception {
        Memory memory = currentProgram.getMemory();
        for (MemoryBlock block : memory.getBlocks()) {
            if (!block.isInitialized()) {
                continue;
            }
            Address found = memory.findBytes(block.getStart(), block.getEnd(), needle, null, true, monitor);
            if (found != null) {
                return found;
            }
        }
        return null;
    }

    private void decompile(Function f) {
        try {
            DecompInterface ifc = new DecompInterface();
            ifc.openProgram(currentProgram);
            DecompileResults res = ifc.decompileFunction(f, 45, monitor);
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

    private void addCallers(Function target, Set<Function> funcs) {
        if (target == null) {
            return;
        }
        println("\nCALLERS " + target.getName() + " @ " + target.getEntryPoint());
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(target.getEntryPoint());
        int count = 0;
        while (refs.hasNext()) {
            Reference ref = refs.next();
            Function caller = getFunctionContaining(ref.getFromAddress());
            println("  caller ref " + ref.getFromAddress() + " type=" + ref.getReferenceType() +
                " func=" + (caller == null ? "<none>" : caller.getName() + "@" + caller.getEntryPoint()));
            if (caller != null) {
                funcs.add(caller);
            }
            count++;
        }
        if (count == 0) {
            println("  no callers");
        }
    }

    private void addRefsForString(String label, Set<Function> funcs) throws Exception {
        Address addr = findBytes(label.getBytes(StandardCharsets.US_ASCII));
        println("\nSTRING " + label + " -> " + addr);
        if (addr == null) {
            return;
        }
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(addr);
        int count = 0;
        while (refs.hasNext()) {
            Reference ref = refs.next();
            count++;
            Function f = getFunctionContaining(ref.getFromAddress());
            println("  ref " + ref.getFromAddress() + " type=" + ref.getReferenceType() +
                " func=" + (f == null ? "<none>" : f.getName() + "@" + f.getEntryPoint()));
            if (f != null) {
                funcs.add(f);
            }
        }
        if (count == 0) {
            println("  no refs");
        }
    }

    @Override
    protected void run() throws Exception {
        String[] labels = new String[] {
            "s_unitMemsWeaponMax",
            "s_getNotHorseUnitMems",
            "s_createUnit",
            "s_createUnitAndMems",
            "OD_PRODEQUI",
            "OD_PRODHORS",
            "ODOBJ_PACKPFERD",
            "ODOBJ_ZIVILIST",
            "FigTiePac00_Packpferd"
        };

        Set<Function> funcs = new HashSet<>();
        String[] manualCallbacks = new String[] {
            "00529f90", // s_createUnit
            "0052a020", // s_createUnitAndMems
            "0052a110", // s_createBattleUnitsMax
            "0052a140", // s_createCiviUnitsMax
            "0052a170", // s_unitMemsWeaponMax
            "0052a3e0", // s_getNotHorseUnitMems
            "005249d0", // createBattleUnitsMax implementation
            "00524d70", // createCiviUnitsMax implementation
            "005251d0", // unitMemsWeaponMax implementation
            "00527110", // getNotHorseUnitMems implementation
            "00525210", // unitMemsWeaponMax helper
            "00527160", // getNotHorseUnitMems helper
            "00523a00", // unit creation from gathered members
            "00538320", // member gather/filter
            "005298a0", // member validity/filter
            "0050f750", // unit position helper
            "00537e30",
            "00541150",
            "0052e6a0"
        };
        for (String hex : manualCallbacks) {
            Address addr = toAddr(hex);
            Function f = getFunctionContaining(addr);
            if (f == null) {
                try {
                    f = createFunction(addr, "SCRIPT_" + hex);
                } catch (Exception ex) {
                    println("Could not create function at " + hex + ": " + ex.getMessage());
                }
            }
            if (f != null) {
                funcs.add(f);
                println("MANUAL " + hex + " -> " + f.getName() + "@" + f.getEntryPoint());
            }
        }

        for (String label : labels) {
            addRefsForString(label, funcs);
        }

        for (String hex : manualCallbacks) {
            Function f = getFunctionContaining(toAddr(hex));
            addCallers(f, funcs);
        }

        println("\nCandidate functions: " + funcs.size());
        for (Function f : funcs) {
            println("  " + f.getName() + " @ " + f.getEntryPoint());
        }
        for (Function f : funcs) {
            decompile(f);
        }
    }
}
