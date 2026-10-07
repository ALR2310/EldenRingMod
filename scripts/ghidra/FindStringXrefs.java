import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.DataType;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import java.io.FileOutputStream;
import java.io.PrintWriter;

// Scans all defined string data in the binary for a case-insensitive
// substring match, then dumps every xref to each match plus the enclosing
// function - used to locate ini-key lookups (e.g. "SCALING") without
// needing any prior CE table or symbol.
public class FindStringXrefs extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        String needle = args.length > 0 ? args[0].toLowerCase() : "scaling";
        String outPath = args.length > 1 ? args[1] : "D:/tmp/string_xrefs.txt";
        PrintWriter out = new PrintWriter(new FileOutputStream(outPath));

        DataIterator iter = currentProgram.getListing().getDefinedData(true);
        int strHits = 0;
        while (iter.hasNext()) {
            Data data = iter.next();
            DataType dt = data.getDataType();
            String dtName = dt.getName().toLowerCase();
            if (!dtName.contains("string") && !dtName.contains("char")) continue;
            Object value = data.getValue();
            if (value == null) continue;
            String s = value.toString();
            if (s == null || !s.toLowerCase().contains(needle)) continue;

            strHits++;
            Address addr = data.getAddress();
            out.println("STRING @ " + addr + ": \"" + s + "\"");
            Reference[] refs = getReferencesTo(addr);
            for (Reference ref : refs) {
                Address from = ref.getFromAddress();
                Function fn = getFunctionContaining(from);
                String fnDesc = fn != null ? (fn.getName() + "@" + fn.getEntryPoint()) : "<no function>";
                out.println("    xref from " + from + "  [" + fnDesc + "]");
            }
            out.println();
        }

        out.println("Total string hits: " + strHits);
        out.flush();
        out.close();
        println("Wrote results to " + outPath);
    }
}
