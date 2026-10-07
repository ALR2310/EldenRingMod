import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.Memory;
import java.io.PrintWriter;
import java.io.FileOutputStream;

// Dumps raw hex bytes at a given RVA (relative to image base) so an AOB
// pattern can be hand-built for a function we only know the RVA of from
// prior Ghidra decompiles (avoids hardcoding a raw runtime address in the
// mod, matching how every other hook in this project resolves via AOB).
public class DumpBytes extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        String outPath = args.length > 0 ? args[0] : "D:/tmp/bytes_dump.txt";
        long rva = args.length > 1 ? Long.decode(args[1]) : 0xd23090L;
        int length = args.length > 2 ? Integer.decode(args[2]) : 48;

        PrintWriter out = new PrintWriter(new FileOutputStream(outPath));
        Address base = currentProgram.getImageBase();
        Address entry = base.add(rva);
        Memory mem = currentProgram.getMemory();
        byte[] buf = new byte[length];
        mem.getBytes(entry, buf);

        StringBuilder sb = new StringBuilder();
        for (byte b : buf) {
            sb.append(String.format("%02X ", b));
        }
        out.println("Address: " + entry + " (rva 0x" + Long.toHexString(rva) + ")");
        out.println(sb.toString().trim());
        out.flush();
        out.close();
        println("Dump written to " + outPath);
    }
}
