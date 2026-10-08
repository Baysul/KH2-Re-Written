// Authored bounded raw-snapshot inspection. Derived output must stay private.
// @category RewriteResearch
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import com.google.gson.*;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.framework.Application;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.SourceType;

public class NativeMemorySelected extends GhidraScript {
    private String sha(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }
    private Path privatePath(String value) throws Exception {
        Path path=Path.of(value).toRealPath();
        Path root=Path.of(System.getProperty("rewrite.research.root")).toRealPath();
        Path source=Path.of(System.getProperty("rewrite.source.root")).toRealPath();
        if (!path.startsWith(root) || path.startsWith(source)) throw new IllegalArgumentException("Private research path required");
        for (Path p=path.getParent();p!=null;p=p.getParent())
            if (Files.exists(p.resolve(".git"))) throw new IllegalArgumentException("Research path inside Git");
        return path;
    }
    @Override protected void run() throws Exception {
        String[] args=getScriptArgs();
        if (args.length!=2) throw new IllegalArgumentException("Selection and new private report required");
        Path selected=privatePath(args[0]);Path output=Path.of(args[1]);
        privatePath(output.getParent().toString());
        if (Files.exists(output) || Files.size(selected)>1048576) throw new IllegalArgumentException("Existing output or oversized selection");
        JsonObject selection=JsonParser.parseString(Files.readString(selected)).getAsJsonObject();
        String inputName=currentProgram.getExecutablePath();
        if (inputName.matches("^/[A-Za-z]:/.*")) inputName=inputName.substring(1);
        Path input=privatePath(inputName);
        long length=Files.size(input),start=selection.get("address").getAsLong();
        if (length<1 || length>4096 || start<=0 || start>Long.MAX_VALUE-length)
            throw new IllegalArgumentException("Invalid bounded snapshot");
        byte[] bytes=Files.readAllBytes(input);String hash=sha(bytes);
        if (!hash.equals(selection.get("snapshot_sha256").getAsString()) ||
            !hash.equalsIgnoreCase(currentProgram.getExecutableSHA256()) ||
            !currentProgram.getExecutableFormat().equals("Raw Binary") ||
            !currentProgram.getLanguageID().toString().equals("x86:LE:64:default") ||
            !currentProgram.getCompilerSpec().getCompilerSpecID().toString().equals("windows"))
            throw new IllegalArgumentException("Snapshot/import/profile differs");
        Address entry=currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(start);
        Address end=entry.add(length-1);AddressSet allowed=new AddressSet(entry,end);
        var blocks=currentProgram.getMemory().getBlocks();
        if (blocks.length!=1 || !blocks[0].getStart().equals(entry) || !blocks[0].getEnd().equals(end) ||
            !blocks[0].isInitialized() || !blocks[0].isExecute()) throw new IllegalArgumentException("Import has extra or differently mapped memory");
        byte[] imported=new byte[(int)length];
        if (currentProgram.getMemory().getBytes(entry,imported)!=length || !Arrays.equals(bytes,imported))
            throw new IllegalArgumentException("Imported complete bytes differ");
        DisassembleCommand command=new DisassembleCommand(entry,allowed,true);
        command.enableCodeAnalysis(false);
        if (!command.applyTo(currentProgram,monitor) || getInstructionAt(entry)==null)
            throw new IllegalStateException("Selected entry did not disassemble");
        Function function=currentProgram.getFunctionManager().createFunction("SelectedRange",entry,allowed,SourceType.USER_DEFINED);
        JsonArray listing=new JsonArray();Set<Integer> covered=new HashSet<>();
        for (Instruction instruction:currentProgram.getListing().getInstructions(allowed,true)) {
            if (listing.size()>=256 || !allowed.contains(instruction.getMinAddress(),instruction.getMaxAddress()))
                throw new IllegalStateException("Instruction exceeds bounded range or count");
            int offset=(int)instruction.getAddress().subtract(entry);byte[] encoded=instruction.getBytes();
            if (!Arrays.equals(encoded,Arrays.copyOfRange(bytes,offset,offset+encoded.length)))
                throw new IllegalStateException("Instruction differs from complete snapshot bytes");
            for (int n=offset;n<offset+encoded.length;n++) {
                var info=currentProgram.getMemory().getAddressSourceInfo(entry.add(n));
                if (info==null || info.getFileOffset()!=n || !covered.add(n))
                    throw new IllegalStateException("Instruction mapping overlaps or differs");
            }
            JsonObject line=new JsonObject();line.addProperty("offset",offset);line.addProperty("size",encoded.length);
            line.addProperty("sha256",sha(encoded));line.addProperty("disassembly",instruction.toString());
            line.addProperty("flow_type",instruction.getFlowType().toString());
            JsonArray flows=new JsonArray();
            for (Address flow:instruction.getFlows()) flows.add(flow.toString());
            line.add("flows",flows);
            JsonArray references=new JsonArray();
            for (var reference:instruction.getReferencesFrom()) {
                JsonObject ref=new JsonObject();ref.addProperty("address",reference.getToAddress().toString());
                ref.addProperty("type",reference.getReferenceType().toString());references.add(ref);
            }
            line.add("references",references);listing.add(line);
        }
        DecompInterface decompiler=new DecompInterface();String code;
        try {
            if (!decompiler.openProgram(currentProgram)) throw new IllegalStateException("Decompiler unavailable");
            DecompileResults result=decompiler.decompileFunction(function,30,monitor);
            if (!result.decompileCompleted() || result.getDecompiledFunction()==null)
                throw new IllegalStateException("Bounded decompilation failed: "+result.getErrorMessage());
            code=result.getDecompiledFunction().getC();
        } finally {decompiler.dispose();}
        if (code.getBytes(StandardCharsets.UTF_8).length>1048576 || !hash.equals(sha(Files.readAllBytes(input))))
            throw new IllegalStateException("Oversized pseudocode or changed input");
        JsonObject report=new JsonObject();report.addProperty("qualification","isolated_memory_range_research_only");
        report.addProperty("loader_profile","raw_x64_windows_isolated_range_v1");
        report.addProperty("ghidra_version",Application.getApplicationVersion());report.addProperty("java_version",System.getProperty("java.version"));
        report.addProperty("snapshot_sha256",hash);report.addProperty("address",start);report.addProperty("range_size",length);
        report.addProperty("mapped_blocks",blocks.length);report.addProperty("instruction_bytes",covered.size());
        report.addProperty("uninterpreted_bytes",length-covered.size());report.add("instructions",listing);
        report.addProperty("pseudocode",code);report.addProperty("pseudocode_sha256",sha(code.getBytes(StandardCharsets.UTF_8)));
        report.addProperty("limitations","Explicit captured range only; outside memory absent; pseudocode is a lead; no complete logical function, ABI, active-call or game-rule proof");
        Files.writeString(output,new GsonBuilder().setPrettyPrinting().create().toJson(report),StandardCharsets.UTF_8,StandardOpenOption.CREATE_NEW);
        println("PASS: isolated bounded snapshot report saved privately.");
    }
}
