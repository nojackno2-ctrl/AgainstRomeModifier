// Ghidra headless script for objdef.dau movement speed and weapon/casting-distance paths.
// Run with analyzeHeadless -postScript GhidraUnitRangeSpeedAnalysis.java
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraUnitRangeSpeedAnalysis extends GhidraScript {
    private final Set<Function> candidates = new LinkedHashSet<>();

    private void addFunction(String address, String label) throws Exception {
        Address at = toAddr(address);
        Function function = getFunctionContaining(at);
        if (function == null) {
            try {
                function = createFunction(at, label);
            } catch (Exception ex) {
                println("CREATE FAILED " + label + " @ " + at + ": " + ex.getMessage());
            }
        }
        println("FUNCTION " + label + " @ " + at + " -> " +
            (function == null ? "<none>" : function.getName() + " @ " + function.getEntryPoint()));
        if (function != null) candidates.add(function);
    }

    private void addDataReferences(String address, String label) throws Exception {
        Address at = toAddr(address);
        println("\nDATA " + label + " @ " + at);
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(at);
        int count = 0;
        while (refs.hasNext()) {
            Reference ref = refs.next();
            Function function = getFunctionContaining(ref.getFromAddress());
            println("  " + ref.getFromAddress() + " " + ref.getReferenceType() + " " +
                (function == null ? "<none>" : function.getName() + " @ " + function.getEntryPoint()));
            if (function != null) candidates.add(function);
            count++;
        }
        if (count == 0) println("  <no direct references>");
    }

    private void addCallers(String address, String label) throws Exception {
        Address at = toAddr(address);
        Function target = getFunctionContaining(at);
        println("\nCALLERS " + label + " @ " + at + " -> " +
            (target == null ? "<none>" : target.getName()));
        if (target == null) return;
        candidates.add(target);
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(target.getEntryPoint());
        while (refs.hasNext()) {
            Reference ref = refs.next();
            Function caller = getFunctionContaining(ref.getFromAddress());
            println("  " + ref.getFromAddress() + " " + ref.getReferenceType() + " " +
                (caller == null ? "<none>" : caller.getName() + " @ " + caller.getEntryPoint()));
            if (caller != null) candidates.add(caller);
        }
    }

    private void decompile(Function function, DecompInterface decompiler) {
        println("\n===== " + function.getName() + " @ " + function.getEntryPoint() + " =====");
        DecompileResults results = decompiler.decompileFunction(function, 60, monitor);
        if (results != null && results.decompileCompleted() && results.getDecompiledFunction() != null) {
            println(results.getDecompiledFunction().getC());
        } else {
            println("<decompile failed>");
        }
    }

    @Override
    protected void run() throws Exception {
        // Runtime objdef table is 0x2a4 bytes per object definition.
        // These base addresses are established by FUN_004b0300's objdef.dau serializer:
        // moves is a scalar float; weapon fields are eight-element arrays.
        addDataReferences("00c648c8", "moves");
        addDataReferences("00c649ac", "w1_rad1 array base (minimum/inner weapon distance)");
        addDataReferences("00c649cc", "w1_rad2 array base (maximum/outer weapon distance)");

        addFunction("004b0300", "objdef serializer/field layout");
        addFunction("004bb480", "movement speed selection");
        addFunction("004c0e60", "weapon rad1 getter");
        addFunction("004e0bd0", "weapon distance calculation");
        addFunction("00542730", "script weapon-distance bridge");
        addFunction("00542800", "script rad1 bridge");
        addFunction("00546c40", "s_objInWeaponDist thunk");
        addFunction("00546ca0", "s_posInWeaponDist thunk");

        addCallers("004bb480", "movement speed selection");
        addCallers("004c0e60", "weapon rad1 getter");
        addCallers("004e0bd0", "weapon distance calculation");
        addCallers("00542730", "script weapon-distance bridge");

        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        for (Function function : candidates) decompile(function, decompiler);
        decompiler.dispose();
    }
}
