// Ghidra headless script. Run with analyzeHeadless -postScript GhidraRessAnalysis.java
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.address.AddressSetView;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.SourceType;

import java.nio.charset.StandardCharsets;
import java.util.HashSet;
import java.util.Set;

public class GhidraRessAnalysis extends GhidraScript {
    private Address findBytes(byte[] needle) throws Exception {
        Memory memory = currentProgram.getMemory();
        for (MemoryBlock block : memory.getBlocks()) {
            if (!block.isInitialized()) {
                continue;
            }
            Address start = block.getStart();
            Address end = block.getEnd();
            Address cur = start;
            while (cur.compareTo(end) <= 0) {
                Address found = memory.findBytes(cur, end, needle, null, true, monitor);
                if (found == null) {
                    break;
                }
                return found;
            }
        }
        return null;
    }

    private void printRefs(Address target, Set<Function> funcs) {
        ReferenceIterator refs = currentProgram.getReferenceManager().getReferencesTo(target);
        int count = 0;
        while (refs.hasNext()) {
            Reference ref = refs.next();
            count++;
            Function f = getFunctionContaining(ref.getFromAddress());
            if (f != null) {
                funcs.add(f);
            }
            println("  ref from " + ref.getFromAddress() + " type=" + ref.getReferenceType() +
                " func=" + (f == null ? "<none>" : f.getName() + "@" + f.getEntryPoint()));
        }
        if (count == 0) {
            println("  no refs");
        }
    }

    private void scanImmediateRefs(Address target, Set<Function> funcs) {
        long value = target.getOffset();
        AddressSetView exec = currentProgram.getMemory().getExecuteSet();
        InstructionIterator iter = currentProgram.getListing().getInstructions(exec, true);
        while (iter.hasNext()) {
            Instruction ins = iter.next();
            Object[] objs = ins.getOpObjects(0);
            boolean hit = false;
            for (int i = 0; i < ins.getNumOperands(); i++) {
                objs = ins.getOpObjects(i);
                for (Object obj : objs) {
                    if (obj instanceof Scalar) {
                        long scalar = ((Scalar) obj).getUnsignedValue();
                        if (scalar == value) {
                            hit = true;
                        }
                    } else if (obj instanceof Address) {
                        if (((Address) obj).equals(target)) {
                            hit = true;
                        }
                    }
                }
            }
            if (hit) {
                Function f = getFunctionContaining(ins.getAddress());
                if (f != null) {
                    funcs.add(f);
                }
                println("  imm/ref ins " + ins.getAddress() + " " + ins + " func=" +
                    (f == null ? "<none>" : f.getName() + "@" + f.getEntryPoint()));
            }
        }
    }

    private void decompileFunction(Function f) {
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

    private void addFunctionAt(String hexAddress, Set<Function> funcs) throws Exception {
        Address addr = toAddr(hexAddress);
        Function f = getFunctionContaining(addr);
        if (f == null) {
            try {
                f = createFunction(addr, "FUN_" + hexAddress);
            } catch (Exception ex) {
                println("Could not create function at " + hexAddress + ": " + ex.getMessage());
            }
        }
        if (f != null) {
            funcs.add(f);
            println("MANUAL " + hexAddress + " -> " + f.getName() + "@" + f.getEntryPoint());
            if (f.getEntryPoint().equals(addr)) {
                f.setName("RESS_" + hexAddress, SourceType.USER_DEFINED);
            }
        } else {
            println("MANUAL " + hexAddress + " -> <no function>");
        }
    }

    private void scanAddressRefs(String label, String hexAddress, Set<Function> funcs) throws Exception {
        Address addr = toAddr(hexAddress);
        println("\nADDRESS " + label + " -> " + addr);
        printRefs(addr, funcs);
        scanImmediateRefs(addr, funcs);
    }

    @Override
    protected void run() throws Exception {
        String[] labels = new String[] {
            "SYSTEM/ress.ini", "[maxskills]", "[objres]", "[volkres]",
            "resv_res%ld_bau", "resv_res%ld_upg", "resv_res%ld_unit"
        };

        Set<Function> candidateFuncs = new HashSet<>();
        addFunctionAt("0046bd00", candidateFuncs);
        addFunctionAt("0046b200", candidateFuncs);

        for (String label : labels) {
            Address addr = findBytes(label.getBytes(StandardCharsets.US_ASCII));
            println("\nSTRING " + label + " -> " + addr);
            if (addr != null) {
                printRefs(addr, candidateFuncs);
                scanImmediateRefs(addr, candidateFuncs);
            }
        }

        scanAddressRefs("objres callback", "0046bd00", candidateFuncs);
        scanAddressRefs("volkres callback", "0046b200", candidateFuncs);
        scanAddressRefs("volkres formation table", "0073d070", candidateFuncs);
        scanAddressRefs("volkres formation table", "0073d078", candidateFuncs);
        scanAddressRefs("volkres rows", "0073d36c", candidateFuncs);

        println("\nCandidate functions: " + candidateFuncs.size());
        for (Function f : candidateFuncs) {
            println("  " + f.getName() + " @ " + f.getEntryPoint());
        }
        for (Function f : candidateFuncs) {
            decompileFunction(f);
        }
    }
}
