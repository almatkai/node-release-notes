#!/usr/bin/env python3
"""Generate narrow unapplied patches from the pinned, committed host snapshot."""
import difflib,pathlib
ROOT=pathlib.Path(__file__).resolve().parents[1];BASE=ROOT/'.work/host'
changes={
'src-tauri/src/native_core/extensions/v2_runtime.rs': [
('        .or_else(|| id.strip_prefix("raycast."))','''        .or_else(|| id.strip_prefix("raycast."))
        .or_else(|| {
            let candidate = id.strip_prefix("com.tezbar.")?;
            super::v2_catalog::source().iter().any(|m| {
                m["slug"] == candidate && m["replaces"].as_array().is_some_and(|ids| {
                    ids.iter().any(|alias| alias == id)
                })
            }).then_some(candidate)
        })'''),
],
'src/renderer/src/raycast-api/detail-runtime.tsx': [('onClick={onRunPrimaryAction}', 'onClick={() => onRunPrimaryAction()}')],
'src/renderer/src/raycast-api/list-runtime.tsx': [
("accessories?: Array<{ text?: unknown; tag?: unknown; date?: unknown; icon?: unknown }>","accessories?: Array<{ text?: unknown; tag?: unknown; date?: unknown; icon?: unknown; tooltip?: unknown }>"),
('icon: entry.icon,','icon: entry.icon,\n      tooltip: entry.tooltip,'),
('      className={\n        isTag','      title={textValue(accessory.tooltip) || undefined}\n      className={\n        isTag'),
("  const [accessoryValue, setAccessoryValue] = useState(\n    searchAccessory?.options[0]?.value ?? '',\n  )", "  const controlledAccessoryValue = (() => {\n    const value = (root.props?.searchBarAccessory as ExtensionRuntimeNode | undefined)?.props?.value\n    return typeof value === 'string' ? value : undefined\n  })()\n  const [accessoryValue, setAccessoryValue] = useState(\n    controlledAccessoryValue ?? searchAccessory?.options[0]?.value ?? '',\n  )"),
("      if (!searchAccessory || searchAccessory.options.length === 0) return ''", "      if (!searchAccessory || searchAccessory.options.length === 0) return ''\n      if (controlledAccessoryValue !== undefined && searchAccessory.options.some((option) => option.value === controlledAccessoryValue)) return controlledAccessoryValue"),
('  }, [searchAccessory])','  }, [searchAccessory, controlledAccessoryValue])'),
],
'src-tauri/src/native_core/extensions/registry.rs':[
('    manifest: Value,\n    data_dir: PathBuf,','    manifest: Value,\n    resource_dir: Option<PathBuf>,\n    data_dir: PathBuf,'),
('            let mut manifest = if native {','            let mut resource_dir = None;\n            let mut manifest = if native {'),
('                    Ok(package) => package.manifest,','                    Ok(package) => {\n                        resource_dir = Some(package.dir);\n                        package.manifest\n                    },'),
('                manifest,\n                data_dir: ctx.paths.data_dir.clone(),','                manifest,\n                resource_dir,\n                data_dir: ctx.paths.data_dir.clone(),'),
('        manifest: Value::Null,\n        data_dir: ctx.paths.data_dir.clone(),','        manifest: Value::Null,\n        resource_dir: None,\n        data_dir: ctx.paths.data_dir.clone(),'),
('    let mut candidates = vec![\n        extension.path.join(relative),', '    if extension.id.starts_with("com.tezbar.native.") {\n        return extension.resource_dir.as_ref()?.join(relative).is_file().then(|| {\n            extension.resource_dir.as_ref().unwrap().join(relative).to_string_lossy().into_owned()\n        });\n    }\n    let mut candidates = vec![\n        extension.path.join(relative),'),
]}
# The registry adds a field: update every struct literal (including tests) by
# inspecting the full pinned source; there is one production literal here.
allpatch=[]
for filename,replacements in changes.items():
 before=(BASE/filename).read_text();after=before
 for old,new in replacements:
  count=after.count(old)
  assert count>0,(filename,old)
  after=after.replace(old,new)
 out=ROOT/'.work/patched-host'/filename;out.parent.mkdir(parents=True,exist_ok=True);out.write_text(after)
 allpatch.extend(difflib.unified_diff(before.splitlines(True),after.splitlines(True),fromfile='a/'+filename,tofile='b/'+filename))
(ROOT/'integration/host-prerequisites.patch').write_text(''.join(allpatch))
print('Unapplied patch generated; no host checkout files changed')
