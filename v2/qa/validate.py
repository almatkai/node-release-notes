#!/usr/bin/env python3
"""Re-run finite QA in this clone. Dependencies must already be prepared.
Does not install into Tezbar, invoke real OS services, query public product APIs,
modify host/SDK sibling sources, stage, commit, push or publish.
"""
import hashlib,json,os,pathlib,subprocess,time
R=pathlib.Path(__file__).resolve().parents[1];logs=R/'.work/validation';logs.mkdir(parents=True,exist_ok=True)
tmp=logs/'temp';tmp.mkdir(exist_ok=True);env=dict(os.environ,TZ='UTC',LANG='en_US.UTF-8',TMPDIR=str(tmp));results=[]
def run(name,args,extra=None,expected=0):
 start=time.monotonic();p=subprocess.run(args,cwd=R.parent,env=dict(env,**(extra or {})),stdout=subprocess.PIPE,stderr=subprocess.STDOUT);output=p.stdout.decode('utf-8','replace');(logs/(name+'.txt')).write_text(output)
 entry={'name':name,'command':args,'exitCode':p.returncode,'status':'passed' if p.returncode==0 else 'failed','log':str((logs/(name+'.txt')).relative_to(R)),'logSha256':hashlib.sha256(p.stdout).hexdigest()};results.append(entry);print(name,entry['status'],flush=True)
 if p.returncode!=expected:raise RuntimeError(name+'\n'+output[-5000:])
run('source-oracle-utc',['node','v2/qa/source-oracle.cjs'])
run('source-oracle-los-angeles',['node','v2/qa/source-oracle.cjs'],{'TZ':'America/Los_Angeles'})
run('stable-tests',['cargo','test','--locked','--manifest-path','v2/Cargo.toml','--all-targets'])
run('msrv-tests',['cargo','+1.85.0','test','--locked','--manifest-path','v2/Cargo.toml','--all-targets'])
run('timezone-parity',['cargo','test','--locked','--manifest-path','v2/Cargo.toml','--test','parity'],{'TZ':'America/Los_Angeles'})
run('fmt',['cargo','fmt','--manifest-path','v2/Cargo.toml','--check'])
for name,version in [('stable-clippy',[]),('msrv-clippy',['+1.85.0'])]:run(name,['cargo']+version+['clippy','--locked','--manifest-path','v2/Cargo.toml','--all-targets','--','-D','warnings'])
run('sdk-tests',['cargo','test','--locked','--manifest-path','v2/.work/sdk/Cargo.toml'])
run('v1-tests',['npm','test','--prefix','v2/.work/v1'])
run('v1-typecheck',['npm','run','typecheck','--prefix','v2/.work/v1'])
run('fixture-executable',['cargo','build','--locked','--manifest-path','v2/Cargo.toml','--examples'])
run('jsonl-pipes',['python3','v2/qa/pipes.py'])
run('actual-renderer',['node','v2/qa/renderer.cjs'])
run('actual-json-schema',['node','v2/qa/schema.cjs'])
run('host-patch-check',['git','apply','--check','--directory=v2/.work/host','v2/integration/host-prerequisites.patch'])
run('arm64-release',['cargo','build','--locked','--manifest-path','v2/Cargo.toml','--release','--target','aarch64-apple-darwin'])
run('deterministic-package',['python3','v2/qa/package.py'])
run('actual-extracted-inventory',['cargo','test','--locked','--manifest-path','v2/Cargo.toml','--test','host_contract','extracted_package','--','--ignored'])
# Cross-compiler failure was genuinely attempted separately; retain exact logs.
windows=[]
for name in ['windows-check','windows-clippy']:
 raw=(R/'.work'/ (name+'.txt')).read_bytes();assert b'x86_64-w64-mingw32-gcc' in raw and b'No such file or directory' in raw
 (R/'qa'/(name+'.txt')).write_bytes(raw);windows.append({'name':name,'status':'blocked','reason':'ring C compilation: x86_64-w64-mingw32-gcc absent; no tools installed','sha256':hashlib.sha256(raw).hexdigest()})
report={'source':'2410143f530d3d6ea7143e25717322acc4af7512','sdk':'e955911f0bf5b558f9d3eb5183df0adc95c22af2','host':'ea100cee3f69711de12485ba061950c5fc65258f','tools':{tool:subprocess.check_output([tool,'--version']).decode().strip() for tool in ['rustc','cargo','node','npm','python3']},'checks':results,'windowsGNU':windows,'nativeMacGuiBrowserClipboard':'not-run','nativeWindowsMSVCGuiBrowserClipboard':'not-run','publicProductEndpointQAQueries':0,'realClipboardBrowserEffects':0,'installed':False,'finiteCoverage':True}
(R/'qa/validation-results.json').write_text(json.dumps(report,indent=2)+'\n');print('Finite QA complete; platform GUI and Windows native acceptance remain not-run')
