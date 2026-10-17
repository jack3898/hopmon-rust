# Post-processes Ghidra output: turns `__real_XXXXXXXX` constant symbols into
# float literals, drops the unsigned-to-double conversion idiom and local
# variable declarations to make the decompiled code shorter to read.
#
#   python3 clean_decomp.py decomp2.c decomp_clean.c
import re,struct,sys
s=open(sys.argv[1]).read()
def f32(m):
    v=struct.unpack('>f',bytes.fromhex(m.group(1)))[0]; return repr(round(v,7))+'f'
def f64(m):
    v=struct.unpack('>d',bytes.fromhex(m.group(1)))[0]; return repr(v)
s=re.sub(r'_*_real_([0-9a-f]{16})\b',f64,s)
s=re.sub(r'_*_real_([0-9a-f]{8})\b',f32,s)
s=re.sub(r'\(double\)\(&__xmm_41f0+\)\[-\(\(int\)([\w\->.]+) >> 0x1f\)\]','0 /*u32fix*/',s)
s=re.sub(r'\(uint\)(\w+) \^ ___xmm_80000000800000008000000080000000',r'-\1',s)
s=re.sub(r' \+\s*0 /\*u32fix\*/','',s)
# drop local declarations block
s=re.sub(r'(?m)^  [A-Za-z_][\w ]*\*? ?\*?[a-zA-Z_]\w*(\[\d+\])?;\n','',s)
open(sys.argv[2],'w').write(s)
