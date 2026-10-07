import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import java.io.FileOutputStream;
import java.io.PrintWriter;

// Dumps raw bytes + disassembly of a function's first N bytes, to design a
// safe trampoline patch length (must land on an instruction boundary).
public class DumpFnPrologue extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        Address addr = currentProgram.getAddressFactory().getAddress(args[0]);
        int window = args.length > 1 ? Integer.parseInt(args[1]) : 40;
        String outPath = args.length > 2 ? args[2] : "D:/tmp/fn_prologue.txt";
        PrintWriter out = new PrintWriter(new FileOutputStream(outPath));

        Address end = addr.add(window);
        Address cur = addr;
        int cumulative = 0;
        while (cur.compareTo(end) < 0) {
            Instruction insn = getInstructionAt(cur);
            if (insn == null) {
                disassemble(cur);
                insn = getInstructionAt(cur);
            }
            if (insn == null) {
                out.println(cur + ": <could not decode>");
                break;
            }
            byte[] bytes = insn.getBytes();
            StringBuilder hex = new StringBuilder();
            for (byte b : bytes) hex.append(String.format("%02x ", b));
            out.println(cur + "  (+0x" + Integer.toHexString(cumulative) + ")  " + hex.toString().trim() + "   " + insn.toString());
            cumulative += insn.getLength();
            cur = cur.add(insn.getLength());
        }
        out.println();
        out.println("Cumulative length to reach a safe boundary >= 14 bytes: see above.");

        out.flush();
        out.close();
        println("Wrote prologue dump to " + outPath);
    }
}
