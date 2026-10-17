// Ghidra headless post-script used to reverse engineer Hopmon.exe.
//
// 1. Rebuilds every X*/J* class structure from the flattened PDB layouts
//    produced by pdb_class_layouts.py (Ghidra's PDB importer leaves derived
//    classes with an opaque `_padding_` base, which loses field offsets).
// 2. Decompiles all application functions (below 0x42d000, i.e. the game and
//    DirectX framework code, not the CRT) into <out>/decomp2.c.
//
// Usage (Hopmon.pdb must sit next to Hopmon.exe):
//   analyzeHeadless <proj_dir> hop -import Hopmon.exe -scriptPath tools/re \
//       -postScript FixAndExport.java <out_dir> <layouts.json>

import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.data.*;
import com.google.gson.*;
import java.io.*;
import java.util.*;
public class FixAndExport extends GhidraScript {
  DataTypeManager dtm;
  DataType findDT(String name){
    List<DataType> l=new ArrayList<>(); dtm.findDataTypes(name,l);
    for (DataType d: l) if (d instanceof Structure) return d;
    return l.isEmpty()?null:l.get(0);
  }
  DataType prim(String p){
    switch(p){
      case "float": return FloatDataType.dataType;
      case "double": return DoubleDataType.dataType;
      case "unsigned": case "unsigned long": case "unsigned int": return UnsignedIntegerDataType.dataType;
      case "unsigned short": case "wchar_t": return UnsignedShortDataType.dataType;
      case "short": return ShortDataType.dataType;
      case "unsigned char": return UnsignedCharDataType.dataType;
      case "char": case "signed char": return CharDataType.dataType;
      case "__int64": case "unsigned __int64": return LongLongDataType.dataType;
      default: return IntegerDataType.dataType;
    }
  }
  DataType conv(JsonObject t){
    if (t.has("prim")) return prim(t.get("prim").getAsString());
    if (t.has("ptr")) { JsonElement e=t.get("ptr"); DataType tgt = e.isJsonNull()?null:findDT(e.getAsString()); return new PointerDataType(tgt==null?VoidDataType.dataType:tgt, 4, dtm); }
    if (t.has("struct")) { DataType d=findDT(t.get("struct").getAsString()); return d==null?IntegerDataType.dataType:d; }
    if (t.has("array")) { DataType el=conv(t.getAsJsonObject("array")); int n=t.get("bytes").getAsInt()/Math.max(1,el.getLength()); return new ArrayDataType(el,Math.max(1,n),el.getLength(),dtm); }
    return IntegerDataType.dataType;
  }
  public void run() throws Exception {
    String[] a=getScriptArgs(); String out=a[0];
    dtm=currentProgram.getDataTypeManager();
    JsonObject root=JsonParser.parseReader(new FileReader(a[1])).getAsJsonObject();
    int tx=currentProgram.startTransaction("fix");
    // pass 1 ensure structs exist with size
    for (String n: root.keySet()){
      DataType d=findDT(n);
      if (!(d instanceof Structure)) { d=dtm.addDataType(new StructureDataType(n,root.getAsJsonObject(n).get("size").getAsInt()),DataTypeConflictHandler.REPLACE_HANDLER); }
    }
    for (String n: root.keySet()){
      JsonObject c=root.getAsJsonObject(n); int size=c.get("size").getAsInt();
      Structure old=(Structure)findDT(n);
      StructureDataType s=new StructureDataType(old.getCategoryPath(),n,size,dtm);
      for (JsonElement fe: c.getAsJsonArray("fields")){
        JsonObject f=fe.getAsJsonObject(); int off=f.get("off").getAsInt();
        DataType dt=conv(f.getAsJsonObject("type"));
        if (off+dt.getLength()>size) continue;
        try { s.replaceAtOffset(off,dt,dt.getLength(),f.get("name").getAsString(),null);} catch(Exception ex){ println("fail "+n+"."+f.get("name")+": "+ex); }
      }
      try { old.replaceWith(s); } catch(Exception ex){ println("replace fail "+n+": "+ex); }
      println("STRUCT "+n+" len="+((Structure)findDT(n)).getLength());
    }
    currentProgram.endTransaction(tx,true);
    DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
    PrintWriter pw=new PrintWriter(new FileWriter(out+"/decomp2.c"));
    for (Function f: currentProgram.getFunctionManager().getFunctions(true)){
      String n=f.getName(true);
      long ea=f.getEntryPoint().getOffset(); if (ea >= 0x42d000L && !n.contains("WinMain")) continue;
      DecompileResults r=d.decompileFunction(f,120,monitor);
      pw.println("// ===== "+n+" @ "+f.getEntryPoint());
      pw.println(r.decompileCompleted()? r.getDecompiledFunction().getC() : "// FAILED "+r.getErrorMessage());
    }
    pw.close();
  }
}
