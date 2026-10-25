#!/usr/bin/env python3
"""Fresh Cargo home + isolated source; HTTPS pinned SDK, no sibling build inputs."""
import hashlib,json,os,pathlib,shutil,subprocess,time
R=pathlib.Path(__file__).resolve().parents[1];base=R/'.work'/('isolated-'+str(time.time_ns()));src=base/'source';home=base/'cargo-home';target=base/'target'
shutil.copytree(R,src,ignore=shutil.ignore_patterns('.work','target','packages','node_modules','__pycache__'))
env=dict(os.environ,CARGO_HOME=str(home),CARGO_TARGET_DIR=str(target),TZ='UTC',TMPDIR=str(base/'temp'));pathlib.Path(env['TMPDIR']).mkdir(parents=True)
assert not home.exists();commands=[]
for args in [['cargo','+1.85.0','build','--locked','--release'],['cargo','+1.85.0','test','--locked']]:
 commands.append(args)
 with open(base/('build.txt' if 'build' in args else 'test.txt'),'w') as log:subprocess.run(args,cwd=src,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
meta=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'],cwd=src,env=env));sdk=next(p for p in meta['packages'] if p['name']=='tezbar-extension-sdk')
assert sdk['version']=='0.1.1' and sdk['source']=='git+https://github.com/almatkai/tezbar-extension-sdk-rust.git?rev=e955911f0bf5b558f9d3eb5183df0adc95c22af2#e955911f0bf5b558f9d3eb5183df0adc95c22af2'
assert pathlib.Path(sdk['manifest_path']).is_relative_to(home)
for p in meta['packages']:assert pathlib.Path(p['manifest_path']).is_relative_to(home) or pathlib.Path(p['manifest_path']).is_relative_to(src)
inputs=[p for pattern in ['src/**/*.rs','tests/**/*.rs','examples/**/*.rs','qa/fixtures/*','qa/oracle-*.json','integration/host_contract.rs','Cargo.toml','Cargo.lock','.cargo/config.toml'] for p in src.glob(pattern) if p.is_file()]
input_hashes={str(p.relative_to(src)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(inputs))}
assert all(hashlib.sha256((R/p).read_bytes()).hexdigest()==h for p,h in input_hashes.items())
report={'buildInputsSha256':input_hashes,'freshCargoHome':True,'isolatedSource':True,'sourceCopiedBeforeBuild':True,'sdkSource':sdk['source'],'sdkVersion':sdk['version'],'commands':commands,'result':'passed','hostOrSiblingPathDependencies':False,'scope':'Independent Rust build/tests; no host app, GUI or OS service acceptance','logs':str(base.relative_to(R))}
(R/'qa/isolated-results.json').write_text(json.dumps(report,indent=2)+'\n');print('Fresh-home HTTPS SDK source build + Rust 1.85 tests passed:',base.relative_to(R))
