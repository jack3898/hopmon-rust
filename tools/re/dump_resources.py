# Dumps every Win32 resource of Hopmon.exe (needs `pip install pefile`).
# The game binary provides `hopmon --extract <dir>` for the same purpose.
#
#   python3 dump_resources.py Hopmon.exe out_dir
import pefile,sys,os
pe=pefile.PE(sys.argv[1]); out=sys.argv[2]; os.makedirs(out,exist_ok=True)
for t in pe.DIRECTORY_ENTRY_RESOURCE.entries:
    tn = str(t.name) if t.name else pefile.RESOURCE_TYPE.get(t.id,t.id)
    for e in t.directory.entries:
        en = str(e.name) if e.name else e.id
        for l in e.directory.entries:
            d=l.data.struct; data=pe.get_data(d.OffsetToData,d.Size)
            print(tn,en,l.id,d.Size,data[:8])
            fn=f"{out}/{tn}_{en}".replace('/','_')
            open(os.path.join(out,f"{tn}__{en}".replace(os.sep,'_')),'wb').write(data)
