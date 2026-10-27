#!/usr/bin/env python3
"""Copy actual license/notice bytes from every locked Cargo package (all targets)."""
import hashlib,json,pathlib,subprocess
R=pathlib.Path(__file__).resolve().parents[1]
meta=json.loads(subprocess.check_output(['cargo','metadata','--locked','--manifest-path',str(R/'Cargo.toml'),'--format-version','1']))
rows=[];missing=[]
for pkg in sorted(meta['packages'],key=lambda p:(p['name'],p['version'])):
 if pkg['name']=='node-release-notes-native':continue
 directory=pathlib.Path(pkg['manifest_path']).parent
 files=[]
 for p in sorted(directory.rglob('*')):
  if not p.is_file() or p.is_symlink():continue
  name=p.name.upper()
  if (name.startswith(('LICENSE','COPYING','NOTICE','UNLICENSE','LICENCE','AUTHORS','COPYRIGHT')) or any(part.upper() in ('LICENSES','LICENSE') for part in p.relative_to(directory).parts[:-1])) and p.suffix not in ['.rs','.html','.c','.h']:
   files.append(p)
 if pkg.get('license_file'):
  p=directory/pkg['license_file']
  if p.is_file() and p not in files:files.append(p)
 captures=[]
 for p in files:
  b=p.read_bytes();rel=p.relative_to(directory);out=R/'licenses/cargo'/f"{pkg['name']}-{pkg['version']}"/rel;out.parent.mkdir(parents=True,exist_ok=True);out.write_bytes(b)
  captures.append({'path':str(out.relative_to(R)),'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
 if not captures:missing.append(pkg['name'])
 rows.append({'name':pkg['name'],'version':pkg['version'],'source':pkg['source'],'license':pkg['license'],'files':captures})
(R/'licenses/cargo-licenses.json').write_text(json.dumps(rows,indent=2)+'\n')
print('Locked package license captures:',len(rows),'packages;',sum(len(r['files']) for r in rows),'files; missing:',missing)
assert not missing,missing
