#!/usr/bin/env python3
"""Deterministic development ZIP. No fabricated URL/catalog pins or installation."""
import hashlib,json,pathlib,stat,struct,subprocess,zipfile
R=pathlib.Path(__file__).resolve().parents[1];target='aarch64-apple-darwin'
exe=R/'target'/target/'release/node-release-notes-native'
b=exe.read_bytes();assert b[:4]==b'\xcf\xfa\xed\xfe' and struct.unpack('<I',b[4:8])[0]==0x100000c,'ARM64 Mach-O required'
files={'bin/node-release-notes-native':b,'LICENSE':(R/'LICENSE').read_bytes(),'NOTICE':(R/'NOTICE').read_bytes()}
for directory in ['resources','licenses']:
 for p in sorted((R/directory).rglob('*')):
  if p.is_file():assert not p.is_symlink();files[str(p.relative_to(R))]=p.read_bytes()
assert len(files['resources/app-icon.png'])==14999
assert hashlib.sha256(files['resources/app-icon.png']).hexdigest()=='4e778503f4f5c797a1b811b1b37bba7b7aeaa8d632b77e9f0b710996a9c48c81'
assert len(files['resources/nodejs-icon.svg'])==2509
assert hashlib.sha256(files['resources/nodejs-icon.svg']).hexdigest()=='060358f37b5a2ba741c888adc3b191c0c9f57409a9b823d92a59eaa56d93b4b4'
manifest=json.loads((R/'extension.json').read_text());manifest['files']={p:hashlib.sha256(b).hexdigest() for p,b in sorted(files.items())}
assert manifest['artifacts']=={},'Source/dev ZIP must not pretend to be a catalog release'
files['extension.json']=(json.dumps(manifest,sort_keys=True,indent=2)+'\n').encode()
output=R/'packages';output.mkdir(exist_ok=True);name=f'node-release-notes-2.0.0-{target}.zip'
def pack(path):
 with zipfile.ZipFile(path,'w',compression=zipfile.ZIP_STORED) as archive:
  for p,data in sorted(files.items()):
   assert not any(c in p.split('/') for c in ['target','node_modules','.work','.git','dist']);assert not p.startswith('/')
   info=zipfile.ZipInfo(p,date_time=(2020,1,1,0,0,0));info.create_system=3;info.external_attr=((stat.S_IFREG|(0o755 if p.startswith('bin/') else 0o644))<<16);archive.writestr(info,data)
pack(output/name);duplicate=R/'.work/package-repeat.zip';pack(duplicate);assert (output/name).read_bytes()==duplicate.read_bytes()
extract=R/'.work/package-extracted';extract.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(output/name) as archive:
 assert archive.namelist()==sorted(files);assert len(set(archive.namelist()))==len(files)
 for member in archive.infolist():
  p=extract/member.filename;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(archive.read(member));p.chmod(0o755 if member.filename.startswith('bin/') else 0o644)
for path,expected in manifest['files'].items():assert hashlib.sha256((extract/path).read_bytes()).hexdigest()==expected
assert set(p.relative_to(extract).as_posix() for p in extract.rglob('*') if p.is_file())==set(files)
extracted=extract/'bin/node-release-notes-native';probes=[]
for flag in ['--protocol-info','--self-test']:
 value=json.loads(subprocess.check_output([str(extracted),flag]));assert value['id']==manifest['id'] and value['protocol']==1 and value['sdkVersion']=='0.1.1';probes.append({'flag':flag,'result':value})
# Real extracted production process: no authorized product HTTP is triggered in QA.
p=subprocess.Popen([str(extracted)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
hello=json.loads(p.stdout.readline());assert hello['type']=='hello';import uuid
frames=[('initialize',{'protocol':1,'extensionId':manifest['id'],'platform':'macos','packageDir':str(extract),'dataDir':str(R/'.work/extracted-data'),'tempDir':str(R/'.work/extracted-temp'),'grantedCapabilities':['clipboard.write','shell.open'],'reason':'qa','migrationPaths':{}}),('session.open',{'sessionId':'rejected','commandName':'uncaptured-command'}),('shutdown',{})]
smoke=[hello]
for method,params in frames:
 id='h:'+str(uuid.uuid4());request={'type':'request','id':id,'method':method,'params':params};p.stdin.write(json.dumps(request)+'\n');p.stdin.flush();reply=json.loads(p.stdout.readline());assert reply['id']==id;smoke.append(reply)
 if method=='session.open':assert 'error' in reply
 else:assert 'result' in reply
p.stdin.close();assert p.wait(timeout=8)==0;assert p.stderr.read()==''
report={'kind':'development verification ZIP; unpublished; no acquisition URL/pins','target':target,'source':'2410143f530d3d6ea7143e25717322acc4af7512','sdk':'e955911f0bf5b558f9d3eb5183df0adc95c22af2','archive':name,'bytes':(output/name).stat().st_size,'sha256':hashlib.sha256((output/name).read_bytes()).hexdigest(),'executableSha256':manifest['files']['bin/node-release-notes-native'],'deterministicZipRepeatIdentical':True,'closedInventory':manifest['files'],'manifestSha256':hashlib.sha256(files['extension.json']).hexdigest(),'probes':probes,'extractedJSONLSmoke':['hello','initialize','unknown command denied without HTTP','shutdown'],'liveProductHttp':False,'installed':False}
(output/'package-report.json').write_text(json.dumps(report,indent=2)+'\n');print(name,report['bytes'],report['sha256'],'members',len(files))
