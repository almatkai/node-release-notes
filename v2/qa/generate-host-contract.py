#!/usr/bin/env python3
"""Exact pure-function excerpts, checked against the committed host, no app build."""
import pathlib,subprocess
R=pathlib.Path(__file__).resolve().parents[1];base=R/'.work/host';patched=R/'.work/patched-host'
p=(base/'src-tauri/src/native_core/extensions/process_install.rs').read_text();l=(base/'src-tauri/src/native_core/extensions/launcher.rs').read_text()
def between(s,start,end):return s[s.index(start):s.index(end,s.index(start))]
parts=['//! Mechanically extracted host ea100cee3f69711de12485ba061950c5fc65258f excerpts (rustfmt only). Regenerate with qa/generate-host-contract.py.\n#![allow(dead_code, clippy::all)]\n']
parts+=['mod launcher {\nuse serde_json::Value;use std::collections::HashSet;use super::process_install;\n',between(l,'fn bounded_text','pub(super) fn selected'),'}\n']
parts+=['pub mod process_install {\nuse serde_json::Value;use std::{collections::HashSet,path::{Path,PathBuf},io::Read};use sha2::{Digest,Sha256};const MAX_FILES:usize=4096;\n',between(p,'const CAPABILITY_GROUPS','// Shared by installer'),between(p,'pub(super) fn component','pub(super) fn artifact('),between(p,'fn artifact_for','pub(super) fn root'),between(p,'fn hash_file','#[derive(Clone)]')]
parts+=['pub fn check(m:&Value)->Result<(),String>{validate(m)}\npub fn acquisition(m:&Value)->Result<&Value,String>{artifact_for(m,"aarch64-apple-darwin")}\npub fn inventory(dir:&Path,m:&Value,a:&Value)->Result<PathBuf,String>{verify_files(dir,m,a)}\n}\n']
for name,folder,field in [('before',base,''),('after',patched,'pub resource_dir:Option<PathBuf>,')]:
 s=(folder/'src-tauri/src/native_core/extensions/registry.rs').read_text()
 parts += ['pub mod registry_'+name+' {\nuse serde_json::Value;use std::path::{Path,PathBuf,Component};\npub struct Installed{pub id:String,pub slug:String,pub path:PathBuf,pub manifest:Value,'+field+'}\n',between(s,'fn fallback','fn icon_data'),between(s,'fn command_icon_path','fn command_icon_data'),
 'pub fn icon(e:&Installed)->Option<String>{icon_path(e)}\npub fn command(e:&Installed,c:&Value)->Option<String>{command_icon_path(e,c)}\n}\n']
for name,folder in [('before',base),('after',patched)]:
 s=(folder/'src-tauri/src/native_core/extensions/v2_runtime.rs').read_text()
 parts += ['pub mod routing_'+name+' {\nmod v2_catalog {pub fn source()->Vec<serde_json::Value>{vec![serde_json::from_str(include_str!("../extension.json")).unwrap()]}}\nmod v2_runtime {\n',s.replace('//! Metadata-only compatibility names. No extension implementation is linked here.','// Pinned compatibility function.'),'\n} pub fn slug(id:&str)->Option<&str>{v2_runtime::slug(id)}\n}\n']
(R/'integration/host_contract.rs').write_text(''.join(parts))
subprocess.run(['rustfmt','--edition','2021',str(R/'integration/host_contract.rs')],check=True)
print('Mechanically extracted validator/inventory and before/after registry functions captured')
