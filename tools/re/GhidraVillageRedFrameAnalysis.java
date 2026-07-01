// Focused headless analysis for the selected-village red dashed frame.
// Run with analyzeHeadless -postScript GhidraVillageRedFrameAnalysis.java
// @category AgainstRome

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

import java.util.LinkedHashSet;
import java.util.Set;

public class GhidraVillageRedFrameAnalysis extends GhidraScript {
    private final Set<Function> functions = new LinkedHashSet<>();

    private void addFunction(String hex, String label) throws Exception {
        Address address = toAddr(hex);
        Function function = getFunctionContaining(address);
        if (function == null) {
            try {
                function = createFunction(address, label);
            } catch (Exception exception) {
                println("CREATE FUNCTION FAILED " + label + " " + hex + ": " + exception.getMessage());
            }
        }
        println("FUNCTION " + label + " " + hex + " -> " +
            (function == null ? "<none>" : function.getName() + "@" + function.getEntryPoint()));
        if (function != null) {
            functions.add(function);
        }
    }

    private void addReferencesTo(String hex, String label) throws Exception {
        Address address = toAddr(hex);
        println("\nREFERENCES TO " + label + " @ " + address);
        ReferenceIterator references = currentProgram.getReferenceManager().getReferencesTo(address);
        int count = 0;
        while (references.hasNext()) {
            Reference reference = references.next();
            Function function = getFunctionContaining(reference.getFromAddress());
            println("  " + reference.getFromAddress() + " " + reference.getReferenceType() + " " +
                (function == null ? "<no function>" : function.getName() + "@" + function.getEntryPoint()));
            if (function != null) {
                functions.add(function);
            }
            count++;
        }
        println("  count=" + count);
    }

    private void dumpInstructions(String hex, int count) throws Exception {
        println("\nINSTRUCTIONS @ " + hex);
        Instruction instruction = currentProgram.getListing().getInstructionAt(toAddr(hex));
        for (int i = 0; i < count && instruction != null; i++) {
            println(instruction.getAddress() + "  " + instruction);
            instruction = instruction.getNext();
        }
    }

    private void dumpAround(String hex, int before, int after) throws Exception {
        Instruction instruction = currentProgram.getListing().getInstructionAt(toAddr(hex));
        if (instruction == null) {
            println("\nAROUND @ " + hex + " <no instruction>");
            return;
        }
        for (int i = 0; i < before && instruction.getPrevious() != null; i++) {
            instruction = instruction.getPrevious();
        }
        println("\nAROUND @ " + hex);
        for (int i = 0; i < before + after + 1 && instruction != null; i++) {
            println(instruction.getAddress() + "  " + instruction);
            instruction = instruction.getNext();
        }
    }

    private void decompileAll() {
        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        for (Function function : functions) {
            println("\n===== DECOMPILE " + function.getName() + " @ " + function.getEntryPoint() + " =====");
            DecompileResults result = decompiler.decompileFunction(function, 60, monitor);
            if (result != null && result.decompileCompleted() && result.getDecompiledFunction() != null) {
                println(result.getDecompiledFunction().getC());
            } else {
                println("<decompile failed>");
            }
        }
        decompiler.dispose();
    }

    @Override
    protected void run() throws Exception {
        addFunction("00451650", "CreateSelectedVillageFrameOverlay");
        addFunction("00421c00", "OverlayWorldCoordinateWrapper");
        addFunction("00520320", "OverlayCoordinateToTile");
        addFunction("005203a0", "CreateOverlayRecord");
        addFunction("00521230", "CommitOverlayRecordsToTiles");
        addFunction("005146e0", "WriteTileActionRecord");
        addFunction("00514740", "ReadTileActionRecord");
        addFunction("0053ba20", "SCRIPT_setVillageAreaDeltas");
        addFunction("0053ba60", "SCRIPT_villageAreaDeltas");
        addFunction("0053ba80", "SCRIPT_getVillageAreaDeltas");
        addFunction("0053bad0", "SCRIPT_getTeamVillageArea");
        addFunction("0053bb40", "SCRIPT_inTeamVillage");
        addFunction("0053c140", "SCRIPT_setShowTeamVillageArea");
        addFunction("0053c170", "SCRIPT_showTeamVillageArea");
        addFunction("00536450", "SetVillageAreaDeltasCandidate");
        addFunction("00536510", "VillageAreaDeltasCandidate");
        addFunction("00536580", "GetVillageAreaDeltasCandidate");
        addFunction("00536630", "GetTeamVillageAreaCandidate");
        addFunction("004d7160", "DrawTeamVillageAreaFrame");
        addFunction("00539700", "InitializePendingVillage");
        addFunction("00544fd0", "FindCandidatePositionInVillage");
        addFunction("0044f4b0", "PlayerOrderPreviewCandidate");
        addFunction("0044f7b0", "PlayerTargetPreviewCandidate");
        addFunction("004c0970", "PointInsideObjectTypeRectangle");
        addFunction("004c0900", "SetObjectTypeRectangle");

        // Overlay record pool and the per-tile action record copied by 005146e0.
        addReferencesTo("02578124", "OverlayRecordPool");
        addReferencesTo("02146074", "TileRecordArrayBase");
        addReferencesTo("021460f4", "TileActionRecordBase");

        // Also collect direct callers of the frame creator and action-record accessors.
        addReferencesTo("00451650", "CreateSelectedVillageFrameOverlay");
        addReferencesTo("005146e0", "WriteTileActionRecord");
        addReferencesTo("00514740", "ReadTileActionRecord");
        addReferencesTo("0053c140", "SCRIPT_setShowTeamVillageArea");
        addReferencesTo("0053c170", "SCRIPT_showTeamVillageArea");
        addReferencesTo("00536820", "PointInsideVillageBounds");
        addReferencesTo("00544fd0", "FindCandidatePositionInVillage");
        addReferencesTo("004c0970", "PointInsideObjectTypeRectangle");
        addReferencesTo("004c0900", "SetObjectTypeRectangle");
        addReferencesTo("0056258f", "VillageRangePatchCodeCave");

        decompileAll();
        dumpInstructions("00451650", 50);
        dumpAround("004418dc", 50, 50);
        dumpAround("004418ee", 50, 50);
        dumpInstructions("005203a0", 120);
        dumpInstructions("00521230", 180);
        dumpInstructions("005146e0", 40);
        dumpInstructions("00514740", 40);
        dumpInstructions("0053ba20", 220);
        dumpInstructions("0053c140", 80);
        dumpInstructions("004d7160", 180);
        dumpInstructions("00539700", 420);
        dumpInstructions("00544fd0", 180);
        dumpInstructions("0044f4b0", 180);
        dumpInstructions("0044f7b0", 140);
        dumpInstructions("004c0970", 100);
        dumpInstructions("004c0900", 80);
        dumpInstructions("00536450", 100);
        dumpAround("0056258f", 20, 20);
    }
}
