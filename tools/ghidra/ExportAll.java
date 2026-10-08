// Exports decompiled C and cross-reference indexes for the whole program.
// Usage (headless postScript): ExportAll.java <output dir>
//
// Writes:
//   functions.tsv  addr, name, size, calling convention, module, #callers, #callees
//   calls.tsv      caller addr, callee addr, call site
//   strings.tsv    string addr, functions referencing it, data words pointing to it, text
//   globals.tsv    data addr, access (R/W/?), function addr, instruction addr
//   decomp/<addr>_<name>.c   one file per function
//   all.c          every function concatenated, in address order
//
// "module" is the source file inferred from `\pinball2\src\...` path strings
// the function references (e.g. pc/display/jpeg.c), or empty.
//@category Timeshock

import java.io.File;
import java.io.PrintWriter;
import java.util.*;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.StringDataInstance;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.RefType;

public class ExportAll extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File out = new File(args.length > 0 ? args[0] : "re/out");
        File decompDir = new File(out, "decomp");
        decompDir.mkdirs();

        Listing listing = currentProgram.getListing();
        FunctionManager fm = currentProgram.getFunctionManager();

        // Many strings (file names especially) are only reachable through
        // pointer tables in the data section. Index every aligned data word
        // that holds the address of a string.
        Map<Long, List<Address>> pointedFrom = new HashMap<>();
        Set<Long> stringStarts = new HashSet<>();
        for (Data d : listing.getDefinedData(true)) {
            if (StringDataInstance.isString(d)) {
                stringStarts.add(d.getAddress().getOffset());
            }
        }
        for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            if (b.isExecute() || !b.isInitialized()) {
                continue;
            }
            for (Address a = b.getStart(); a.compareTo(b.getEnd().subtract(3)) <= 0; a = a.add(4)) {
                long v = currentProgram.getMemory().getInt(a) & 0xffffffffL;
                if (stringStarts.contains(v)) {
                    pointedFrom.computeIfAbsent(v, k -> new ArrayList<>()).add(a);
                }
            }
        }

        // Strings and the functions referencing them; infer modules from paths.
        Map<Function, String> module = new HashMap<>();
        try (PrintWriter w = new PrintWriter(new File(out, "strings.tsv"), "UTF-8")) {
            w.println("addr\tfunctions\tpointed_from\ttext");
            for (Data d : listing.getDefinedData(true)) {
                if (!StringDataInstance.isString(d)) {
                    continue;
                }
                String text = StringDataInstance.getStringDataInstance(d).getStringValue();
                if (text == null) {
                    continue;
                }
                Set<String> users = new TreeSet<>();
                for (Reference r : getReferencesTo(d.getAddress())) {
                    Function f = fm.getFunctionContaining(r.getFromAddress());
                    if (f == null) {
                        continue;
                    }
                    users.add(f.getEntryPoint().toString());
                    String m = moduleFromPath(text);
                    if (m != null) {
                        module.putIfAbsent(f, m);
                    }
                }
                List<String> ptrs = new ArrayList<>();
                for (Address p : pointedFrom.getOrDefault(d.getAddress().getOffset(), List.of())) {
                    ptrs.add(p.toString());
                }
                w.println(d.getAddress() + "\t" + String.join(",", users) + "\t"
                    + String.join(",", ptrs) + "\t" + escape(text));
            }
        }

        // Call graph.
        Map<Function, Integer> callers = new HashMap<>(), callees = new HashMap<>();
        try (PrintWriter w = new PrintWriter(new File(out, "calls.tsv"), "UTF-8")) {
            w.println("caller\tcallee\tsite");
            for (Function f : fm.getFunctions(true)) {
                for (Instruction ins : listing.getInstructions(f.getBody(), true)) {
                    for (Reference r : ins.getReferencesFrom()) {
                        if (!r.getReferenceType().isCall()) {
                            continue;
                        }
                        Function g = fm.getFunctionAt(r.getToAddress());
                        if (g == null) {
                            continue;
                        }
                        w.println(f.getEntryPoint() + "\t" + g.getEntryPoint() + "\t" + ins.getAddress());
                        callees.merge(f, 1, Integer::sum);
                        callers.merge(g, 1, Integer::sum);
                    }
                }
            }
        }

        // Global data accesses (outside the code section).
        try (PrintWriter w = new PrintWriter(new File(out, "globals.tsv"), "UTF-8")) {
            w.println("data\taccess\tfunction\tsite");
            for (Function f : fm.getFunctions(true)) {
                for (Instruction ins : listing.getInstructions(f.getBody(), true)) {
                    for (Reference r : ins.getReferencesFrom()) {
                        RefType t = r.getReferenceType();
                        if (!t.isData()) {
                            continue;
                        }
                        Address to = r.getToAddress();
                        MemoryBlock b = currentProgram.getMemory().getBlock(to);
                        if (b == null || b.isExecute()) {
                            continue;
                        }
                        String acc = t.isWrite() ? "W" : t.isRead() ? "R" : "?";
                        w.println(to + "\t" + acc + "\t" + f.getEntryPoint() + "\t" + ins.getAddress());
                    }
                }
            }
        }

        // Decompile everything.
        DecompInterface ifc = new DecompInterface();
        DecompileOptions opts = new DecompileOptions();
        opts.grabFromProgram(currentProgram);
        ifc.setOptions(opts);
        ifc.openProgram(currentProgram);

        int ok = 0, failed = 0;
        try (PrintWriter all = new PrintWriter(new File(out, "all.c"), "UTF-8");
             PrintWriter idx = new PrintWriter(new File(out, "functions.tsv"), "UTF-8")) {
            idx.println("addr\tname\tsize\tconv\tmodule\tcallers\tcallees");
            for (Function f : fm.getFunctions(true)) {
                monitor.checkCancelled();
                if (f.isExternal()) {
                    continue;
                }
                String addr = f.getEntryPoint().toString();
                idx.println(addr + "\t" + f.getName() + "\t" + f.getBody().getNumAddresses() + "\t"
                    + f.getCallingConventionName() + "\t" + module.getOrDefault(f, "") + "\t"
                    + callers.getOrDefault(f, 0) + "\t" + callees.getOrDefault(f, 0));
                if (f.isThunk()) {
                    continue;
                }
                DecompileResults res = ifc.decompileFunction(f, 60, monitor);
                String header = "// " + addr + " " + f.getName()
                    + (module.containsKey(f) ? "  [" + module.get(f) + "]" : "") + "\n";
                String c;
                if (res.decompileCompleted() && res.getDecompiledFunction() != null) {
                    c = res.getDecompiledFunction().getC();
                    ok++;
                } else {
                    c = "/* decompilation failed: " + res.getErrorMessage() + " */\n";
                    failed++;
                }
                try (PrintWriter w = new PrintWriter(new File(decompDir, addr + "_" + f.getName() + ".c"), "UTF-8")) {
                    w.print(header);
                    w.print(c);
                }
                all.print(header);
                all.print(c);
                all.println();
            }
        }
        ifc.dispose();
        println("ExportAll: decompiled " + ok + ", failed " + failed + " -> " + out.getAbsolutePath());
    }

    /** `\pinball2\src\pc\display\jpeg.c(%d): ...` -> `pc/display/jpeg.c` */
    private static String moduleFromPath(String s) {
        int i = s.toLowerCase().indexOf("\\src\\");
        if (i < 0) {
            return null;
        }
        String rest = s.substring(i + 5);
        int end = rest.toLowerCase().indexOf(".c");
        if (end < 0) {
            return null;
        }
        return rest.substring(0, end + 2).replace('\\', '/').toLowerCase();
    }

    private static String escape(String s) {
        return s.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n").replace("\r", "\\r");
    }
}
