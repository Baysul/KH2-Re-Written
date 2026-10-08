using System;

// Authored AMD64 runtime-function metadata selection; no file/process access.
public static class NativeFunctionTable {
    public sealed class Selection {
        public int RecordCount;
        public int Index = -1;
        public uint Begin;
        public uint End;
        public uint Unwind;
        public bool ExactEntry;
    }

    public static Selection Select(byte[] table, uint targetRva, uint imageSize) {
        if (table == null || table.Length == 0 || table.Length % 12 != 0 ||
            table.Length > 131072 * 12 || imageSize == 0 || targetRva == 0 || targetRva >= imageSize)
            throw new ArgumentException("Bounded AMD64 table and image-relative target required");
        var result = new Selection { RecordCount = table.Length / 12 };
        uint previous = 0;
        for (int index = 0; index < result.RecordCount; index++) {
            int offset = index * 12;
            uint begin = BitConverter.ToUInt32(table, offset);
            uint end = BitConverter.ToUInt32(table, offset + 4);
            uint unwind = BitConverter.ToUInt32(table, offset + 8);
            if (begin <= previous || begin >= end || end > imageSize || unwind == 0 ||
                unwind % 4 != 0 || (ulong)unwind + 4 > imageSize)
                throw new ArgumentException("Invalid ordered image-relative runtime-function record");
            previous = begin;
            if (targetRva < begin || targetRva >= end) continue;
            if (result.Index >= 0) throw new ArgumentException("Ambiguous containing runtime-function ranges");
            result.Index = index;
            result.Begin = begin;
            result.End = end;
            result.Unwind = unwind;
            result.ExactEntry = targetRva == begin;
        }
        return result; // Index -1 preserves a missing range without inventing bounds.
    }
}
