// Ghidra headless helper for NPC job runtime execution.
// Run with analyzeHeadless and -postScript GhidraNpcJobRuntimeAnalysis.java.
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.SourceType;

import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraNpcJobRuntimeAnalysis extends GhidraScript {
    @Override
    protected void run() throws Exception {
        Set<Function> funcs = new LinkedHashSet<>();

        addFunction("00548570", "NPCJob_findFreeSlot", funcs);
        addFunction("005482e0", "NPCJob_remove", funcs);
        addFunction("005483e0", "NPCJob_getStatus", funcs);
        addFunction("00548450", "NPCJob_getCreatedObj", funcs);

        collectRefsToAddress("NPC job table", "029e6008", funcs);
        collectRefsToAddress("NPC job table near base", "029e6000", funcs);

        // Known create-unit helpers from previous analysis; include callers around them.
        collectRefsToAddress("Impl create unit", "005242e0", funcs);
        collectRefsToAddress("Impl create unit and mems", "00524530", funcs);

        println("");
        println("Candidate functions: " + funcs.size());
        for (Function f : funcs) {
            println("  " + f.getName() + " @ " + f.getEntryPoint());
        }

        DecompInterface ifc = new DecompInterface();
        ifc.openProgram(currentProgram);
        for (Function f : funcs) {
            decompile(ifc, f);
        }
        ifc.dispose();
    }

    private void addFunction(String addr, String name, Set<Function> funcs) throws Exception {
        Address a = toAddr(addr);
        Function fn = getFunctionAt(a);
        if (fn == null) {
            disassemble(a);
            try {
                fn = createFunction(a, name);
            } catch (Exception ignored) {
            }
        }
        if (fn != null) {
            fn.setName(name, SourceType.USER_DEFINED);
            funcs.add(fn);
            println("MANUAL " + addr + " -> " + fn.getName() + "@" + fn.getEntryPoint());
        } else {
            println("MANUAL " + addr + " -> <failed>");
        }
    }

    private void collectRefsToAddress(String label, String addr, Set<Function> funcs) throws Exception {
        Address target = toAddr(addr);
        println("");
        println("ADDRESS " + label + " -> " + target);

        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(target);
        while (refs.hasNext()) {
            Reference ref = refs.next();
            Function fn = getFunctionContaining(ref.getFromAddress());
            if (fn != null) {
                funcs.add(fn);
            }
            println("  ref " + ref.getFromAddress() + " type=" + ref.getReferenceType() +
                    " func=" + (fn == null ? "<none>" : fn.getName() + "@" + fn.getEntryPoint()));
        }

        for (Instruction ins = getFirstInstruction(); ins != null; ins = getInstructionAfter(ins)) {
            String text = ins.toString();
            if (text.indexOf(addr.toLowerCase()) >= 0 || text.indexOf(addr.toUpperCase()) >= 0) {
                Function fn = getFunctionContaining(ins.getAddress());
                if (fn != null) {
                    funcs.add(fn);
                }
                println("  ins " + ins.getAddress() + " " + text + " func=" +
                        (fn == null ? "<none>" : fn.getName() + "@" + fn.getEntryPoint()));
            }
        }
    }

    private void decompile(DecompInterface ifc, Function fn) throws Exception {
        println("");
        println("DECOMPILE " + fn.getName() + " " + fn.getEntryPoint());
        DecompileResults res = ifc.decompileFunction(fn, 60, monitor);
        if (!res.decompileCompleted() || res.getDecompiledFunction() == null) {
            println("  <failed> " + res.getErrorMessage());
            return;
        }
        println(res.getDecompiledFunction().getC());
    }
}
