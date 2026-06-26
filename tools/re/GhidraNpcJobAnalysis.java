// Ghidra headless helper for endless-mode NPC job callbacks.
// Run with analyzeHeadless and -postScript GhidraNpcJobAnalysis.java.
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.RefType;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

public class GhidraNpcJobAnalysis extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String[] strings = new String[] {
            "s_addNPCJob_createUnit",
            "s_addNPCJob_dissolveUnit",
            "s_removeNPCJob",
            "s_getNPCJob_status",
            "s_getNPCJob_createdObj"
        };

        for (String s : strings) {
            findStringRefs(s);
        }

        String[][] funcs = new String[][] {
            {"0054aa80", "SCRIPT_addNPCJob_createUnit"},
            {"00547f50", "Impl_addNPCJob_createUnit"},
            {"0054ab10", "SCRIPT_addNPCJob_dissolveUnit"},
            {"00548120", "Impl_addNPCJob_dissolveUnit"},
            {"0054ab50", "SCRIPT_removeNPCJob"},
            {"0054ab70", "SCRIPT_getNPCJob_status"},
            {"0054aba0", "SCRIPT_getNPCJob_createdObj"}
        };

        for (String[] f : funcs) {
            addFunction(f[0], f[1]);
        }

        DecompInterface ifc = new DecompInterface();
        ifc.openProgram(currentProgram);
        for (String[] f : funcs) {
            decompile(ifc, f[0], f[1]);
        }
        ifc.dispose();
    }

    private void findStringRefs(String needle) throws Exception {
        println("");
        println("STRING " + needle);
        byte[] bytes = (needle + "\0").getBytes("US-ASCII");
        Memory memory = currentProgram.getMemory();
        for (MemoryBlock block : memory.getBlocks()) {
            if (!block.isInitialized()) {
                continue;
            }
            Address search = block.getStart();
            while (true) {
                Address found = memory.findBytes(search, block.getEnd(), bytes, null, true, monitor);
                if (found == null) {
                    break;
                }
            println("  found " + found);
            ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(found);
            while (refs.hasNext()) {
                Reference ref = refs.next();
                RefType type = ref.getReferenceType();
                Function fn = getFunctionContaining(ref.getFromAddress());
                println("    ref " + ref.getFromAddress() + " type=" + type +
                    " func=" + (fn == null ? "<none>" : fn.getName() + "@" + fn.getEntryPoint()));
            }
                search = found.add(1);
            }
        }
    }

    private void addFunction(String addr, String name) throws Exception {
        Address a = toAddr(addr);
        Function fn = getFunctionAt(a);
        if (fn == null) {
            disassemble(a);
            fn = createFunction(a, name);
        }
        if (fn != null) {
            fn.setName(name, ghidra.program.model.symbol.SourceType.USER_DEFINED);
            println("MANUAL " + addr + " -> " + fn.getName() + "@" + fn.getEntryPoint());
        } else {
            println("MANUAL " + addr + " -> <failed>");
        }
    }

    private void decompile(DecompInterface ifc, String addr, String label) throws Exception {
        Function fn = getFunctionAt(toAddr(addr));
        println("");
        println("DECOMPILE " + label + " " + addr);
        if (fn == null) {
            println("  <no function>");
            return;
        }
        DecompileResults res = ifc.decompileFunction(fn, 30, monitor);
        if (!res.decompileCompleted()) {
            println("  <failed> " + res.getErrorMessage());
            return;
        }
        println(res.getDecompiledFunction().getC());
    }
}
