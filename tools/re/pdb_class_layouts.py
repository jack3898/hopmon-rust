# Flattens class layouts (including base classes) from `llvm-pdbutil dump
# -types Hopmon.pdb` output into JSON consumed by FixAndExport.java.
#
#   llvm-pdbutil dump -types Hopmon.pdb > types.txt
#   python3 pdb_class_layouts.py types.txt layouts.json
import re,sys,json
lines=open(sys.argv[1]).read().split('\n')
recs={}; order=[]; cur=None
for l in lines:
    m=re.match(r'\s+(0x[0-9A-F]+) \| (LF_\w+) \[size = \d+\](.*)',l)
    if m: cur=m.group(1); recs[cur]={'kind':m.group(2),'head':m.group(3).strip().strip('`'),'body':[]}; order.append(cur); continue
    if cur: recs[cur]['body'].append(l.strip())
def ty(t):
    m=re.match(r'(0x[0-9A-F]+)(?: \((.*)\))?',t)
    if m.group(2):
        n=m.group(2)
        if '*' in n: return {'ptr':None}
        return {'prim':n}
    r=recs.get(m.group(1))
    if not r: return {'prim':'int'}
    b=' '.join(r['body'])
    k=r['kind']
    if k in('LF_CLASS','LF_STRUCTURE','LF_UNION'): return {'struct':r['head']}
    if k=='LF_ENUM': return {'prim':'int'}
    if k=='LF_POINTER':
        mm=re.search(r'referent = (0x[0-9A-F]+(?: \([^)]*\))?)',b); t2=ty(mm.group(1))
        return {'ptr':t2.get('struct')}
    if k=='LF_MODIFIER':
        mm=re.search(r'referent = (0x[0-9A-F]+(?: \([^)]*\))?)',b); return ty(mm.group(1))
    if k=='LF_ARRAY':
        mm=re.search(r'size: (\d+), index type: \S+(?: \([^)]*\))?, element type: (0x[0-9A-F]+(?: \([^)]*\))?)',b)
        return {'array':ty(mm.group(2)),'bytes':int(mm.group(1))}
    return {'prim':'int'}
classes={}
for k in order:
    r=recs[k]
    if r['kind'] not in('LF_CLASS','LF_STRUCTURE'): continue
    b=' '.join(r['body'])
    if 'forward ref' in b: continue
    fl=re.search(r'field list: (0x[0-9A-F]+)',b); sz=re.search(r'sizeof (\d+)',b); vt=re.search(r'vtable: (0x[0-9A-F]+)',b)
    if not fl: continue
    body=recs[fl.group(1)]['body']; mem=[]; bases=[]
    for i,f in enumerate(body):
        if f.startswith('- LF_BCLASS'):
            mm=re.search(r'type = (0x[0-9A-F]+), offset = (\d+)',body[i+1]); bases.append((recs[mm.group(1)]['head'],int(mm.group(2))))
        mm=re.match(r'- LF_MEMBER \[name = `(\w+)`, Type = (0x[0-9A-F]+(?: \([^)]*\))?), offset = (\d+)',f)
        if mm: mem.append((int(mm.group(3)),mm.group(1),ty(mm.group(2))))
    classes.setdefault(r['head'],{'size':int(sz.group(1)),'bases':bases,'members':mem,'vt':bool(vt)})
def flat(n,off=0):
    c=classes[n]; out=[]
    for b,bo in c['bases']: out+=flat(b,off+bo)
    if c['vt'] and not c['bases'] and off==0: out.append((0,'vftable',{'ptr':None}))
    for o,nm,t in c['members']: out.append((off+o,nm,t))
    return out
res={}
for n,c in classes.items():
    if re.match(r'^[XJ][A-Z]',n) and '::' not in n:
        fl=sorted(flat(n)); res[n]={'size':c['size'],'fields':[{'off':o,'name':nm,'type':t} for o,nm,t in fl]}
json.dump(res,open(sys.argv[2],'w'),indent=1)
print(len(res))
