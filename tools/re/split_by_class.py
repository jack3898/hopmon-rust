# Splits a decompiled C file into one file per class (`Class.c`).
#
#   python3 split_by_class.py decomp_clean.c out_dir
import re,sys,os,collections
src=open(sys.argv[1]).read(); out=sys.argv[2]; os.makedirs(out,exist_ok=True)
parts=re.split(r'(?m)^// ===== ',src)[1:]
by=collections.defaultdict(list)
for p in parts:
    name=p.split(' @ ')[0]
    if 'scalar_deleting' in name: continue
    cls=name.split('::')[0] if '::' in name else 'free'
    by[cls].append('// ===== '+p)
for c,v in by.items():
    open(os.path.join(out,re.sub(r'[^A-Za-z0-9_]','_',c)+'.c'),'w').write(''.join(v))
