#!/usr/bin/env python3
"""All Git-visible bytes, including unstaged/untracked deliverables. No staging."""
import hashlib,json,pathlib,subprocess
R=pathlib.Path(__file__).resolve().parents[2];O='2410143f530d3d6ea7143e25717322acc4af7512';LINK=b'\n[Native Rust v2 implementation and validation](v2/README.md).\n'
assert subprocess.check_output(['git','-C',str(R),'rev-parse','--show-toplevel']).decode().strip()==str(R)
assert subprocess.check_output(['git','-C',str(R),'branch','--show-current']).decode().strip()=='feat/native-v2-rust'
subprocess.run(['git','-C',str(R),'diff','--cached','--exit-code'],check=True)
v1=[]
for file in subprocess.check_output(['git','-C',str(R),'ls-tree','-r','--name-only',O]).decode().splitlines():
 before=subprocess.check_output(['git','-C',str(R),'show',O+':'+file]);after=(R/file).read_bytes();assert after==(before+LINK if file=='README.md' else before),file
 v1.append({'path':file,'sourceBytes':len(before),'sourceSha256':hashlib.sha256(before).hexdigest(),'currentBytes':len(after),'currentSha256':hashlib.sha256(after).hexdigest(),'exception':'one discovery link' if file=='README.md' else None})
(R/'v2/qa/v1-integrity.json').write_text(json.dumps({'source':O,'unchangedExceptReadmeLink':True,'files':v1},indent=2)+'\n')
files=subprocess.check_output(['git','-C',str(R),'ls-files','--cached','--others','--exclude-standard','-z']).decode().split('\0');rows=[]
for file in sorted(set(files)):
 if not file or file=='v2/INDEX.json':continue
 assert not file.startswith(('v2/.work/','v2/target/')),file
 assert file.startswith('v2/') or file in {f['path'] for f in v1},file
 p=R/file;assert p.is_file() and not p.is_symlink(),file;b=p.read_bytes();rows.append({'path':file,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
(R/'v2/INDEX.json').write_text(json.dumps({'source':O,'branch':'feat/native-v2-rust','sdk':'e955911f0bf5b558f9d3eb5183df0adc95c22af2','host':'ea100cee3f69711de12485ba061950c5fc65258f','scope':'All Git-visible tracked/untracked files except v2/INDEX.json itself; Git-ignored target/.work are disposable. No staging.','files':rows},indent=2)+'\n')
print('V1 exact except discovery link;',len(rows),'Git-visible deliverable files inventoried; nothing staged')
