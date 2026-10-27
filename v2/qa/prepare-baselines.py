#!/usr/bin/env python3
"""Read committed objects only; never host working files or a sibling SDK default."""
import hashlib,json,pathlib,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];WORK=ROOT/'v2/.work'
HOST=ROOT.parents[1]/'Tezbar';SDK=ROOT.parent/'tezbar-extension-sdk-rust'
H='ea100cee3f69711de12485ba061950c5fc65258f';S='e955911f0bf5b558f9d3eb5183df0adc95c22af2';O='2410143f530d3d6ea7143e25717322acc4af7512'
host=['docs/EXTERNAL_RUST_EXTENSIONS.md','schemas/extension-v3.schema.json','src/shared/extensionRuntime.ts','src-tauri/src/native_core/extensions/process.rs','src-tauri/src/native_core/extensions/native.rs','src-tauri/src/native_core/extensions/v2_runtime.rs','src-tauri/src/native_core/extensions/process_host.rs','src-tauri/src/native_core/extensions/process_install.rs','src-tauri/src/native_core/extensions/registry.rs','src-tauri/src/native_core/extensions/launcher.rs','src-tauri/src/native_core/launcher/clipboard.rs','src-tauri/src/native_core/platform.rs','pnpm-lock.yaml']
host+=['src/renderer/src/raycast-api/'+x for x in ['list-runtime.tsx','detail-runtime.tsx','action-runtime.tsx','action-runtime-overlay.tsx','action-runtime-registry.tsx','index.tsx']]
host+=['src/renderer/ui/Markdown.tsx']
sdk=subprocess.check_output(['git','-C',str(SDK),'ls-tree','-r','--name-only',S]).decode().splitlines()
objects=[]
for repo,rev,folder,files in [(HOST,H,'host',host),(SDK,S,'sdk',sdk),(ROOT,O,'v1',subprocess.check_output(['git','-C',str(ROOT),'ls-tree','-r','--name-only',O]).decode().splitlines())]:
 for file in files:
  data=subprocess.check_output(['git','-C',str(repo),'show',rev+':'+file]);p=WORK/folder/file;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
  blob=subprocess.check_output(['git','-C',str(repo),'rev-parse',rev+':'+file]).decode().strip()
  objects.append({'source':folder,'commit':rev,'path':file,'gitBlob':blob,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
(ROOT/'v2/qa/git-objects.json').write_text(json.dumps(objects,indent=2)+'\n')
print('Captured',len(objects),'committed Git objects; host/SDK trees unchanged')
