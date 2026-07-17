// Ghidra headless script for the IGM "select idle villagers" path.
// Run with analyzeHeadless -process Against_Rome.exe -postScript GhidraSelectIdleAnalysis.java <output-file>
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

import java.io.PrintWriter;
import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraSelectIdleAnalysis extends GhidraScript {
    private PrintWriter out;

    private void log(String s) {
        println(s);
        if (out != null) out.println(s);
    }

    private Function ensureFunction(String hex, String name) {
        try {
            Address addr = toAddr(hex);
            Function f = getFunctionContaining(addr);
            if (f == null) {
                f = createFunction(addr, name);
            }
            if (f != null) log("FUNC " + hex + " -> " + f.getName() + " @ " + f.getEntryPoint());
            return f;
        } catch (Exception ex) {
            log("ensureFunction " + hex + " failed: " + ex.getMessage());
            return null;
        }
    }

    private void refsTo(String hex, Set<Function> collect) {
        Address addr = toAddr(hex);
        log("\nREFS to " + hex + ":");
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(addr);
        while (refs.hasNext()) {
            Reference ref = refs.next();
            Function caller = getFunctionContaining(ref.getFromAddress());
            log("  from " + ref.getFromAddress() + " type=" + ref.getReferenceType() +
                " func=" + (caller == null ? "<none>" : caller.getName() + "@" + caller.getEntryPoint()));
            if (caller != null) collect.add(caller);
        }
    }

    private void decompile(Function f) {
        try {
            DecompInterface ifc = new DecompInterface();
            ifc.openProgram(currentProgram);
            DecompileResults res = ifc.decompileFunction(f, 90, monitor);
            log("\n===== DECOMPILE " + f.getName() + " @ " + f.getEntryPoint() + " =====");
            if (res != null && res.decompileCompleted() && res.getDecompiledFunction() != null) {
                log(res.getDecompiledFunction().getC());
            } else {
                log("<decompile failed>");
            }
            ifc.dispose();
        } catch (Exception ex) {
            log("<decompile exception: " + ex.getMessage() + ">");
        }
    }

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length > 0) {
            out = new PrintWriter(args[0], "UTF-8");
        }
        try {
            Set<Function> funcs = new LinkedHashSet<>();

            // Full select-idle chain decoded 2026-07-16 (capstone + pseudocode inventory,
            // see docs/reverse-engineering/exe-functions.md "Unit Selection Subsystem").
            String[][] chain = {
                {"004410b0", "IgmButtonCallback"},      // dispatch: cmp ebx,[0x68b568] @ 0x44138b
                {"0044d110", "SelectionClearAll"},
                {"00451dc0", "SelectIdleVillagers"},    // IdleSelect999 patch target, file 0x51DC0
                {"0044cef0", "SelectFilterDescriptor"},
                {"00421820", "GatherObjectsFiltered"},
                {"00538320", "SearchFilterSetup"},
                {"005388a0", "SearchUnitCategoryLists"},// scratch DAT_0064d65c cap 1000
                {"00419e00", "ScratchCopyToBuffers"},
                {"00421130", "HandlePairToObjectIndex"},
                {"0044d5a0", "SelectionAdd"},           // cmp [0x7286e8],0x3E7 master cap 999
            };
            for (String[] item : chain) {
                Function f = ensureFunction(item[0], item[1]);
                if (f != null) funcs.add(f);
            }

            // Widget-handle global for igm_select_idle.
            refsTo("0068b568", funcs);

            for (Function f : funcs) decompile(f);
        } finally {
            if (out != null) out.close();
        }
    }
}
