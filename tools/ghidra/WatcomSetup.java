// Adds a __watcall prototype (Watcom register calling convention) to the
// program and applies it to every function. Functions whose callers pop the
// stack arguments themselves (`add esp,N` right after the call) get __cdecl.
//
// Watcom passes the first four integer arguments in EAX, EDX, EBX, ECX, the
// rest on the stack (popped by the callee), returns in EAX (ST0 for floats)
// and preserves every other register. EDX:EAX 64-bit returns are left out on
// purpose: they are rare here and make Ghidra misread the preserved EDX as
// part of the return value.
//@category Timeshock

import ghidra.app.script.GhidraScript;
import ghidra.program.database.SpecExtension;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.SourceType;

public class WatcomSetup extends GhidraScript {

    private static final String WATCALL = """
        <prototype name="__watcall" extrapop="unknown" stackshift="4">
          <input>
            <pentry minsize="1" maxsize="4"><register name="EAX"/></pentry>
            <pentry minsize="1" maxsize="4"><register name="EDX"/></pentry>
            <pentry minsize="1" maxsize="4"><register name="EBX"/></pentry>
            <pentry minsize="1" maxsize="4"><register name="ECX"/></pentry>
            <pentry minsize="1" maxsize="500" align="4"><addr offset="4" space="stack"/></pentry>
          </input>
          <output>
            <pentry minsize="4" maxsize="10" metatype="float"><register name="ST0"/></pentry>
            <pentry minsize="1" maxsize="4"><register name="EAX"/></pentry>
          </output>
          <unaffected>
            <register name="EBX"/><register name="ECX"/><register name="EDX"/>
            <register name="ESI"/><register name="EDI"/><register name="EBP"/>
            <register name="ESP"/>
          </unaffected>
        </prototype>
        """;

    @Override
    public void run() throws Exception {
        SpecExtension ext = new SpecExtension(currentProgram);
        ext.addReplaceCompilerSpecExtension(WATCALL, monitor);

        int watcall = 0, cdecl = 0;
        for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
            if (f.isThunk() || f.isExternal()) {
                continue;
            }
            String conv = callerCleansStack(f) ? "__cdecl" : "__watcall";
            f.setCallingConvention(conv);
            if (f.getSignatureSource() == SourceType.DEFAULT) {
                f.setSignatureSource(SourceType.ANALYSIS);
            }
            if (conv.equals("__cdecl")) cdecl++; else watcall++;
        }
        println("WatcomSetup: " + watcall + " __watcall, " + cdecl + " __cdecl");
    }

    /** True if any call site is directly followed by `add esp, imm`. */
    private boolean callerCleansStack(Function f) {
        for (Reference ref : getReferencesTo(f.getEntryPoint())) {
            if (!ref.getReferenceType().isCall()) {
                continue;
            }
            Instruction call = getInstructionAt(ref.getFromAddress());
            if (call == null) {
                continue;
            }
            Instruction next = call.getNext();
            if (next != null && next.getMnemonicString().equals("ADD")
                    && next.getDefaultOperandRepresentation(0).equals("ESP")
                    && next.getScalar(1) instanceof Scalar) {
                return true;
            }
        }
        return false;
    }
}
