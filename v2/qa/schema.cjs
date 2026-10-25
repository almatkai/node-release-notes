const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'..'),deps=path.join(root,'.work/renderer-deps/node_modules');
const Ajv=require(path.join(deps,'ajv/dist/2020'));const ajv=new Ajv({allErrors:true,strict:false});require(path.join(deps,'ajv-formats'))(ajv);
const schema=JSON.parse(fs.readFileSync(path.join(root,'.work/host/schemas/extension-v3.schema.json')));const validate=ajv.compile(schema);const manifest=JSON.parse(fs.readFileSync(path.join(root,'extension.json')));
assert(validate(manifest),JSON.stringify(validate.errors));const cases=[];
for(const [key,value] of [['schemaVersion',2],['id','raycast.node-release-notes'],['runtime','javascript'],['execution','compiled'],['protocolVersion',2],['replaces',['bad.id']],['capabilities',['filesystem']],['artifacts',{'bad-target':{}}],['files',{'../escape':'0'.repeat(64)}]]){
 const m=structuredClone(manifest);m[key]=value;assert(!validate(m),key);cases.push(key);
}
const duplicate=structuredClone(manifest);duplicate.replaces.push(duplicate.replaces[0]);assert(!validate(duplicate));
const bad=structuredClone(manifest);bad.artifacts['aarch64-apple-darwin']={url:'file:///fake',sha256:'fake',executable:'bin/test',executableSha256:'fake'};assert(!validate(bad));
fs.writeFileSync(path.join(__dirname,'schema-results.json'),JSON.stringify({host:'ea100cee3f69711de12485ba061950c5fc65258f',sourceManifestAccepted:true,emptyDevelopmentArtifactsAccepted:true,canonicalId:manifest.id,replaces:manifest.replaces,minimalGrants:manifest.capabilities,negativeCases:cases.concat(['duplicate replacement','fake artifact']),acquisition:'Not installable until publication and trusted real URL/digest pins'},null,2)+'\n');console.log('Actual schema-3: source positive, aliases/grants accepted, 11 negatives rejected');
