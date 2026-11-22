import sys, pathlib,struct
root=pathlib.Path(sys.argv[1])
for p in sorted(root.glob('[0-9]*-*')):
 slot=int(p.name.split('-')[0]); b=(p/'shreds.bin').read_bytes(); buf=bytearray()
 for pos in range(0,len(b),1203):
  s=b[pos:pos+1203]; buf.extend(s[88:int.from_bytes(s[86:88],'little')])
  if s[85]&64:
   if buf[:11]==bytes(8)+b'\x01\x00\x00' and buf[13]==1:
    body=buf[13:]; off=42+body[41]
    if body[off]==1:
     target=struct.unpack_from('<Q',body,off+1)[0]
     if list(root.glob(str(target)+'-*')):print(target,slot,flush=True)
   buf.clear()
