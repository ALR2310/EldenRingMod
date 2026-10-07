import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.io.FileOutputStream;
import java.io.PrintWriter;

// Decompiles 1 function by entry address, plus its direct callers (xrefs)
// and callees, for manual inspection - used to identify the real
// "ApplyDamage" function among FindDamagePath.java's candidates.
public class DecompileFn extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        String addrStr = args[0];
        String outPath = args.length > 1 ? args[1] : "D:/tmp/decompiled_fn.txt";
        PrintWriter out = new PrintWriter(new FileOutputStream(outPath));

        Address addr = currentProgram.getAddressFactory().getAddress(addrStr);
        Function fn = getFunctionContaining(addr);
        if (fn == null) {
            out.println("No function at/containing " + addrStr);
            out.flush();
            out.close();
            return;
        }

        DecompInterface decomp = new DecompInterface();
        decomp.openProgram(currentProgram);

        out.println("=== " + fn.getName() + " @ " + fn.getEntryPoint() + " ===");
        out.println("Signature: " + fn.getSignature());
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

        out.flush();
        out.close();
        println("Wrote decompile of " + fn.getName() + " to " + outPath);
    }
}
