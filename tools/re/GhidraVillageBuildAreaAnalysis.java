// Ghidra headless script for village/build-area candidates.
// Run with analyzeHeadless -postScript GhidraVillageBuildAreaAnalysis.java
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

import java.nio.charset.StandardCharsets;
import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraVillageBuildAreaAnalysis extends GhidraScript {
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

    private void dumpInstructions(String hex, int count) throws Exception {
        Address addr = toAddr(hex);
        println("\n===== INSTRUCTIONS @ " + hex + " =====");
        Instruction instr = currentProgram.getListing().getInstructionAt(addr);
        for (int i = 0; i < count && instr != null; i++) {
            println(instr.getAddress() + "  " + instr);
            instr = instr.getNext();
        }
    }

    private void dumpBytes(String hex, int count) throws Exception {
        Address addr = toAddr(hex);
        byte[] bytes = new byte[count];
        currentProgram.getMemory().getBytes(addr, bytes);
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < bytes.length; i++) {
            if (i > 0) sb.append(' ');
            sb.append(String.format("%02x", bytes[i] & 0xff));
        }
        println("\n===== BYTES @ " + hex + " =====");
        println(sb.toString());
    }

    @Override
    protected void run() throws Exception {
        Set<Function> funcs = new LinkedHashSet<>();
        String[] strings = {
            "village aera",
            "BUILD--------",
            "LEAVEVILLAGE-",
            "TAKEVILLAGE-",
            "ResTpVillage",
            "BuildPrios",
            "BldType",
            "s_getNPCBldPos",
            "s_getNPCPalPos",
            "s_createNPCVillageObj",
            "s_tstCreateVillage",
            "s_tstMoveAllowed",
            "s_setVillageTemplate",
            "OD_BAUHAU",
            "OD_BAUWOH"
        };
        for (String label : strings) addRefsForString(label, funcs);

        String[][] manual = {
            {"0054acd0", "SCRIPT_createNPCVillageObj"},
            {"00548d40", "Impl_createNPCVillageObj"},
            {"0054ad80", "SCRIPT_getNotBuildNPCBld"},
            {"00548de0", "Impl_getNotBuildNPCBld"},
            {"0054adc0", "SCRIPT_getNotBuildNPCPal"},
            {"00548ed0", "Impl_getNotBuildNPCPal"},
            {"0054ae00", "SCRIPT_getNPCBldPos"},
            {"00549070", "Impl_getNPCBldPos"},
            {"0054ae50", "SCRIPT_getNPCPalPos"},
            {"00549150", "Impl_getNPCPalPos"},
            {"0054aea0", "SCRIPT_setNPCBldObj"},
            {"00549260", "Impl_setNPCBldObj"},
            {"0054aed0", "SCRIPT_setNPCPalObj"},
            {"00549310", "Impl_setNPCPalObj"},
            {"0054af80", "SCRIPT_setVillageTemplate"},
            {"00549500", "Impl_setVillageTemplate"},
            {"00473f10", "FindVillageTemplate"},
            {"005495d0", "LoadVillageBuildingsForTeam"},
            {"00549910", "LoadVillagePalisadesForTeam"},
            {"005499b0", "GetVillageFallbackPosition"},
            {"0054bd90", "SCRIPT_tstCreateVillage"},
            {"0054b660", "Impl_tstCreateVillage"},
            {"00536340", "HasActiveVillageForTeam"},
            {"00536310", "GetTeamVillageObject"},
            {"005363e0", "GetTeamVillagePosition"},
            {"00536450", "SetTeamVillageOffsetOrState"},
            {"00536510", "VillageStateCandidate1"},
            {"00536580", "VillageStateCandidate2"},
            {"00536630", "GetVillageBounds"},
            {"0053bb00", "DrawVillageBoundsCandidate"},
            {"0053bb27", "DrawVillageBoundsCallSite"},
            {"005367c0", "TileInVillageBounds"},
            {"005368c0", "ClampPosToVillageBounds"},
            {"00536b60", "VillageStateCandidate3"},
            {"00537050", "VillageListGet"},
            {"00537080", "VillageListHas"},
            {"00539600", "HasPendingVillageForTeam"},
            {"00539620", "PendingVillageCandidate2"},
            {"00539150", "PendingVillageCandidate"},
            {"005391c0", "PendingVillageFindOrAdd"},
            {"0054b430", "VillageTestCandidate"},
            {"0054be00", "SCRIPT_tstMoveAllowed"},
            {"0054b730", "Impl_tstMoveAllowed"},
            {"0054b770", "Impl_tileMoveAllowed"},
            {"00421c00", "DrawOverlayLineCandidate"},
            {"00520320", "DrawOverlayDispatchCandidate"},
            {"005203a0", "DrawOverlayCreateCandidate"},
            {"0044f4b0", "DrawBuildOverlayCandidate"},
            {"0044f7b0", "DrawObjectOverlayCandidate"},
            {"00536250", "SetTeamVillageObject"},
            {"0054aa80", "SCRIPT_addNPCJob_createUnit"}
        };
        for (String[] item : manual) addManual(item[0], item[1], funcs);
        for (String[] item : manual) addCallers(getFunctionContaining(toAddr(item[0])), funcs);

        println("\nCandidate functions: " + funcs.size());
        for (Function f : funcs) println("  " + f.getName() + " @ " + f.getEntryPoint());
        for (Function f : funcs) decompile(f);
        dumpInstructions("00536630", 120);
        dumpInstructions("00536820", 70);
        dumpInstructions("0053b900", 260);
        dumpInstructions("0054b770", 80);
        dumpBytes("00602da0", 64);
    }
}
