// Authored research script. All source-derived output must stay private.
// @category RewriteResearch
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import com.google.gson.*;
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.framework.Application;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;

public class NativeSelected extends GhidraScript {
    private String sha(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private void privateOutput(Path output) throws Exception {
        Path parent = output.getParent().toRealPath();
        Path privateRoot = Path.of(System.getProperty("rewrite.research.root")).toRealPath();
        Path sourceRoot = Path.of(System.getProperty("rewrite.source.root")).toRealPath();
        if (!parent.startsWith(privateRoot) || parent.startsWith(sourceRoot))
            throw new IllegalArgumentException("Output outside private research or inside source");
        for (Path p = parent; p != null; p = p.getParent()) {
            if (Files.exists(p.resolve(".git")))
                throw new IllegalArgumentException("Output inside Git checkout");
        }
        if (Files.exists(output)) throw new IllegalArgumentException("Output exists");
    }

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 2) throw new IllegalArgumentException("Selection and new private report required");
        Path selectionPath = Path.of(args[0]);
        Path output = Path.of(args[1]);
        privateOutput(output);
        if (Files.size(selectionPath) > 1048576) throw new IllegalArgumentException("Selection exceeds 1 MiB");
        JsonObject selection = JsonParser.parseString(Files.readString(selectionPath)).getAsJsonObject();
        // Ghidra stores Windows executable paths with a leading slash (/C:/...).
        String executablePath = currentProgram.getExecutablePath();
        if (executablePath.matches("^/[A-Za-z]:/.*")) executablePath = executablePath.substring(1);
        Path executable = Path.of(executablePath);
        if (Files.size(executable) > 134217728) throw new IllegalArgumentException("Input exceeds 128 MiB");
        byte[] snapshot = Files.readAllBytes(executable);
        String inputSha = sha(snapshot);
        if (!inputSha.equals(selection.get("executable_sha256").getAsString()) ||
            !inputSha.equalsIgnoreCase(currentProgram.getExecutableSHA256()))
            throw new IllegalArgumentException("Source/import/selection identity differs");
        if (!currentProgram.getLanguageID().toString().equals("x86:LE:64:default") ||
            !currentProgram.getCompilerSpec().getCompilerSpecID().toString().equals("windows"))
            throw new IllegalArgumentException("Unqualified native language/compiler specification");
        if (!currentProgram.getImageBase().toString().equalsIgnoreCase(System.getProperty("rewrite.input.imagebase")))
            throw new IllegalArgumentException("Native imported base differs from independently parsed PE header");
        JsonArray methods = selection.getAsJsonArray("methods");
        if (methods.size() < 1 || methods.size() > 8) throw new IllegalArgumentException("Select one to eight native leads");
        JsonArray findings = new JsonArray();
        Set<Long> used = new HashSet<>();
        DecompInterface decompiler = new DecompInterface();
        try {
            if (!decompiler.openProgram(currentProgram)) throw new IllegalStateException("Native decompiler unavailable");
            for (JsonElement element : methods) {
                monitor.checkCancelled();
                JsonObject row = element.getAsJsonObject();
                long rva = row.get("rva").getAsLong();
                long fileOffset = row.get("file_offset").getAsLong();
                int prefixSize = row.get("prefix_size").getAsInt();
                if (rva < 0 || !used.add(rva) || fileOffset < 0 || prefixSize < 1 || prefixSize > 64 ||
                    fileOffset + prefixSize > snapshot.length) throw new IllegalArgumentException("Invalid source span");
                Address address = currentProgram.getImageBase().add(rva);
                byte[] prefix = new byte[prefixSize];
                if (!currentProgram.getMemory().getBlock(address).isExecute() ||
                    currentProgram.getMemory().getBytes(address, prefix) != prefixSize ||
                    !sha(prefix).equals(row.get("prefix_sha256").getAsString()) ||
                    !Arrays.equals(prefix, Arrays.copyOfRange(snapshot, (int)fileOffset, (int)fileOffset + prefixSize)))
                    throw new IllegalArgumentException("Native lead does not match source snapshot");
                for (int i = 0; i < prefixSize; i++) {
                    var info = currentProgram.getMemory().getAddressSourceInfo(address.add(i));
                    if (info == null || info.getFileOffset() != fileOffset + i)
                        throw new IllegalArgumentException("Native prefix physical mapping differs");
                }
                Function function = getFunctionAt(address);
                if (function == null) {
                    if (!disassemble(address)) throw new IllegalStateException("Selected address did not disassemble");
                    function = createFunction(address, null);
                }
                if (function == null || !function.getEntryPoint().equals(address) ||
                    function.getBody().getNumAddresses() > 4096)
                    throw new IllegalStateException("Missing or oversized selected function body");
                JsonArray instructions = new JsonArray();
                int count = 0;
                for (Instruction instruction : currentProgram.getListing().getInstructions(function.getBody(), true)) {
                    if (++count > 256) throw new IllegalStateException("Selected function exceeds 256 instructions");
                    JsonObject line = new JsonObject();
                    long instructionRva = instruction.getAddress().subtract(currentProgram.getImageBase());
                    var info = currentProgram.getMemory().getAddressSourceInfo(instruction.getAddress());
                    byte[] bytes = instruction.getBytes();
                    if (info == null || info.getFileOffset() < 0 || info.getFileOffset() + bytes.length > snapshot.length ||
                        !Arrays.equals(bytes, Arrays.copyOfRange(snapshot, (int)info.getFileOffset(), (int)info.getFileOffset() + bytes.length)))
                        throw new IllegalStateException("Instruction differs from physical source bytes");
                    line.addProperty("rva", instructionRva);
                    line.addProperty("file_offset", info.getFileOffset());
                    line.addProperty("size", bytes.length);
                    line.addProperty("sha256", sha(bytes));
                    line.addProperty("disassembly", instruction.toString());
                    instructions.add(line);
                }
                if (count == 0 || getInstructionAt(address) == null)
                    throw new IllegalStateException("Selected native body lacks disassembled entry/instructions");
                DecompileResults result = decompiler.decompileFunction(function, 30, monitor);
                if (!result.decompileCompleted() || result.getDecompiledFunction() == null)
                    throw new IllegalStateException("Selected decompilation failed: " + result.getErrorMessage());
                String code = result.getDecompiledFunction().getC();
                if (code.getBytes(StandardCharsets.UTF_8).length > 1048576)
                    throw new IllegalStateException("Pseudocode exceeds 1 MiB");
                JsonObject finding = new JsonObject();
                finding.addProperty("rva", rva);
                finding.addProperty("entry_file_offset", fileOffset);
                finding.addProperty("function_name", function.getName());
                finding.addProperty("body_size", function.getBody().getNumAddresses());
                finding.addProperty("pseudocode", code);
                finding.addProperty("pseudocode_sha256", sha(code.getBytes(StandardCharsets.UTF_8)));
                finding.add("instructions", instructions);
                findings.add(finding);
            }
        } finally { decompiler.dispose(); }
        if (!inputSha.equals(sha(Files.readAllBytes(executable))))
            throw new IllegalStateException("Input changed during native inspection");
        JsonObject report = new JsonObject();
        report.addProperty("qualification", "quarantined_static_native_research_only");
        report.addProperty("ghidra_version", Application.getApplicationVersion());
        report.addProperty("java_version", System.getProperty("java.version"));
        report.addProperty("executable_sha256", inputSha);
        report.addProperty("image_base", currentProgram.getImageBase().toString());
        report.addProperty("language", currentProgram.getLanguageID().toString());
        report.addProperty("compiler_spec", currentProgram.getCompilerSpec().getCompilerSpecID().toString());
        report.addProperty("executable_format", currentProgram.getExecutableFormat());
        report.add("findings", findings);
        report.addProperty("limitations", "Selected static native bodies only; no target execution, active callers or whole-game correctness claim");
        String json = new GsonBuilder().setPrettyPrinting().create().toJson(report);
        if (json.getBytes(StandardCharsets.UTF_8).length > 8388608)
            throw new IllegalStateException("Report exceeds 8 MiB");
        Files.writeString(output, json, StandardCharsets.UTF_8, StandardOpenOption.CREATE_NEW);
        println("PASS: source-mapped selected native findings saved privately.");
    }
}
