// Authored static research helper. Source-derived output stays outside Git.
// @category RewriteResearch
import ghidra.app.script.GhidraScript;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import com.google.gson.*;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;

public class AbilityNativeAudit extends GhidraScript {
    private String sha(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 4 || args.length > 35) throw new IllegalArgumentException("output, source, SHA256, and function addresses required");
        Path out = Path.of(args[0]).toRealPath();
        Path source = Path.of(args[1]).toRealPath();
        if (out.startsWith(source.getParent())) throw new IllegalArgumentException("output inside installation");
        for (Path p = out; p != null; p = p.getParent()) {
            if (Files.exists(p.resolve(".git"))) throw new IllegalArgumentException("output inside checkout");
        }
        byte[] raw = Files.readAllBytes(source);
        if (!sha(raw).equals(args[2]) || !args[2].equalsIgnoreCase(currentProgram.getExecutableSHA256()))
            throw new IllegalArgumentException("current source / project / expected hash differs");
        if (!currentProgram.getLanguageID().toString().equals("x86:LE:64:default") ||
            !currentProgram.getCompilerSpec().getCompilerSpecID().toString().equals("windows"))
            throw new IllegalArgumentException("wrong language/compiler");
        JsonObject report = new JsonObject();
        report.addProperty("source_sha256", sha(raw));
        report.addProperty("language", currentProgram.getLanguageID().toString());
        report.addProperty("runtime_observed", false);
        JsonArray functions = new JsonArray();
        DecompInterface decompiler = new DecompInterface();
        try {
            if (!decompiler.openProgram(currentProgram)) throw new IllegalStateException("decompiler unavailable");
            for (int i = 3; i < args.length; i++) {
                var entry = toAddr(args[i]);
                if (getInstructionAt(entry) == null && !disassemble(entry)) throw new IllegalStateException("cannot decode " + entry);
                Function f = getFunctionAt(entry);
                if (f == null) f = createFunction(entry, null);
                if (f == null) throw new IllegalStateException("no function " + entry);
                CreateFunctionCmd.fixupFunctionBody(currentProgram, f, monitor);
                if (f.getBody().getNumAddresses() > 32768) throw new IllegalStateException("oversized function");
                JsonObject row = new JsonObject();
                row.addProperty("entry", entry.toString());
                row.addProperty("body_bytes", f.getBody().getNumAddresses());
                JsonArray instructions = new JsonArray();
                for (Instruction ins : currentProgram.getListing().getInstructions(f.getBody(), true)) {
                    byte[] bytes = ins.getBytes();
                    var info = currentProgram.getMemory().getAddressSourceInfo(ins.getAddress());
                    if (info == null || info.getFileOffset() < 0 || info.getFileOffset() + bytes.length > raw.length)
                        throw new IllegalStateException("unmapped instruction");
                    int at = (int)info.getFileOffset();
                    if (!Arrays.equals(bytes, Arrays.copyOfRange(raw, at, at + bytes.length))) throw new IllegalStateException("instruction mismatch");
                    JsonObject line = new JsonObject();
                    line.addProperty("address", ins.getAddress().toString());
                    line.addProperty("file_offset", at);
                    line.addProperty("bytes", HexFormat.of().formatHex(bytes));
                    line.addProperty("assembly", ins.toString());
                    instructions.add(line);
                }
                DecompileResults result = decompiler.decompileFunction(f, 120, monitor);
                if (!result.decompileCompleted() || result.getDecompiledFunction() == null)
                    throw new IllegalStateException("decompile failed: " + result.getErrorMessage());
                String code = result.getDecompiledFunction().getC();
                Files.writeString(out.resolve(args[i] + ".c"), code, StandardOpenOption.CREATE_NEW);
                row.addProperty("pseudocode_sha256", sha(code.getBytes(java.nio.charset.StandardCharsets.UTF_8)));
                row.add("instructions", instructions);
                functions.add(row);
            }
        } finally { decompiler.dispose(); }
        if (!sha(Files.readAllBytes(source)).equals(args[2])) throw new IllegalStateException("source changed during inspection");
        report.add("functions", functions);
        report.addProperty("source_bytes_match", true);
        Files.writeString(out.resolve("native-audit.json"), new GsonBuilder().setPrettyPrinting().create().toJson(report), StandardOpenOption.CREATE_NEW);
    }
}
