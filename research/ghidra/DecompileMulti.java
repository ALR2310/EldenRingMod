import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.io.FileOutputStream;
import java.io.PrintWriter;

// Decompiles several functions (by entry address, comma-separated in arg 0)
// into one output file, each clearly delimited - for batch inspection of
// SetHp's callers.
public class DecompileMulti extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        String outPath = args[0];
        PrintWriter out = new PrintWriter(new FileOutputStream(outPath));

        DecompInterface decomp = new DecompInterface();
        decomp.openProgram(currentProgram);

        for (int i = 1; i < args.length; i++) {
            String addrStr = args[i].trim();
            Address addr = currentProgram.getAddressFactory().getAddress(addrStr);
            Function fn = getFunctionContaining(addr);
            out.println("=================================================================");
            if (fn == null) {
                out.println("No function at " + addrStr);
                continue;
            }
            out.println("=== " + fn.getName() + " @ " + fn.getEntryPoint() + " ===");
            out.println("Callers: ");
            for (Function caller : fn.getCallingFunctions(monitor)) {
                out.println("  " + caller.getName() + " @ " + caller.getEntryPoint());
            }
            out.println();
            DecompileResults res = decomp.decompileFunction(fn, 60, monitor);
            if (res != null && res.decompileCompleted()) {
                out.println(res.getDecompiledFunction().getC());
            } else {
                out.println("Decompile failed: " + (res != null ? res.getErrorMessage() : "null result"));
            }
            out.println();
        }

        out.flush();
        out.close();
        println("Wrote decompile of " + (args.length - 1) + " functions to " + outPath);
    }
}
