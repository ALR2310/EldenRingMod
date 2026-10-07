import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import java.io.FileOutputStream;
import java.io.PrintWriter;

// Decompiles every function whose name contains any of the given
// plus-separated substrings (arg0), writing to arg1.
public class DecompileByName extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        String[] needles = args[0].split("\\+");
        PrintWriter out = new PrintWriter(new FileOutputStream(args[1]));
        DecompInterface decomp = new DecompInterface();
        decomp.openProgram(currentProgram);
        for (Function fn : currentProgram.getFunctionManager().getFunctions(true)) {
            boolean hit = false;
            for (String n : needles) if (fn.getName().contains(n)) hit = true;
            if (!hit) continue;
            out.println("=== " + fn.getName() + " @ " + fn.getEntryPoint() + " ===");
            DecompileResults res = decomp.decompileFunction(fn, 60, monitor);
            out.println(res != null && res.decompileCompleted() ? res.getDecompiledFunction().getC() : "Decompile failed");
        }
        out.close();
    }
}
