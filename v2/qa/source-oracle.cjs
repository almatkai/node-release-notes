/* Controller may read/write this clone. Evaluated source has a closed import
 * graph and captures every fetch/Raycast/fs/os/process boundary BEFORE loading. */
const fs=require('node:fs');const path=require('node:path');const vm=require('node:vm');
const assert=require('node:assert/strict');const crypto=require('node:crypto');
const root=path.resolve(__dirname,'../..');const v1=path.join(root,'v2/.work/v1');
const ts=require(path.join(v1,'node_modules/typescript'));
const fixture=JSON.parse(fs.readFileSync(path.join(__dirname,'fixtures/feed.json')));
const release=JSON.parse(fs.readFileSync(path.join(__dirname,'fixtures/release.json')));
const boundaries=[],requests=[],replies=[],toasts=[],warnings=[];
const realProducer=()=>{throw new Error('REAL PRODUCER TRIPWIRE');};
// Every real-producer import is denied, including transitive dependencies.
const forbidden=/^(?:node:)?(?:fs(?:\/promises)?|os|child_process|process|http|https|net|tls|dns|worker_threads|module)$/;
class Hooks {
 constructor(){this.slots=[];this.effects=[];this.cursor=0;}
 next(make){const i=this.cursor++;if(!(i in this.slots))this.slots[i]=make();return this.slots[i];}
 render(fn){this.cursor=0;active=this;const value=fn();active=null;const effects=this.effects.splice(0);for(const e of effects)e();return value;}
 unmount(){for(const s of this.slots)if(s.cleanup)s.cleanup();}
}
let active=null;
const same=(a,b)=>a&&b&&a.length===b.length&&a.every((v,i)=>Object.is(v,b[i]));
const react={
 useRef:v=>active.next(()=>({current:v})),
 useState:init=>{const slot=active.next(()=>{const s={value:typeof init==='function'?init():init};s.set=v=>s.value=typeof v==='function'?v(s.value):v;return s;});return [slot.value,slot.set];},
 useMemo:(fn,deps)=>{const slot=active.next(()=>({}));if(!same(slot.deps,deps)){slot.value=fn();slot.deps=deps;}return slot.value;},
 useCallback:(fn,deps)=>react.useMemo(()=>fn,deps),
 useEffect:(fn,deps)=>{const h=active,slot=h.next(()=>({}));if(!same(slot.deps,deps)){slot.deps=deps;h.effects.push(()=>{slot.cleanup?.();slot.cleanup=fn();});}},
 useSyncExternalStore:(_subscribe,snapshot)=>snapshot(),
};
const token=name=>Object.assign(()=>{}, {tag:name});
const List=Object.assign(token('List'),{Item:token('List.Item'),Section:token('List.Section'),EmptyView:token('List.EmptyView'),Dropdown:Object.assign(token('List.Dropdown'),{Item:token('List.Dropdown.Item')})});
const Detail=Object.assign(token('Detail'),{Metadata:Object.assign(token('Detail.Metadata'),{Label:token('Detail.Metadata.Label'),Link:token('Detail.Metadata.Link'),Separator:token('Detail.Metadata.Separator'),TagList:Object.assign(token('Detail.Metadata.TagList'),{Item:token('Detail.Metadata.TagList.Item')})})});
const Action=Object.assign(token('Action'),Object.fromEntries(['Push','CopyToClipboard','OpenInBrowser'].map(n=>[n,token('Action.'+n)])));
const stores=new Map();
class Cache{constructor({namespace}){this.namespace=namespace;this.subscribe=()=>()=>{};}get(k){return stores.get(this.namespace+':'+k);}set(k,v){stores.set(this.namespace+':'+k,v);boundaries.push(['cache.set',this.namespace,k]);}}
const api={List,Detail,Action,ActionPanel:Object.assign(token('ActionPanel'),{Section:token('ActionPanel.Section')}),
 Color:{Blue:'blue',Yellow:'yellow',Green:'green',SecondaryText:'secondary'},Icon:{Calendar:'Icon.Calendar',Eye:'Icon.Eye'},Cache,
 environment:{launchType:'userInitiated',commandMode:'view',supportPath:path.join(root,'v2/.work/fake-home')},LaunchType:{Background:'background'}};
const jsx=(type,props,key)=>({type:type.tag||type,key,props:props||{}});
const FixedDate=class extends Date{constructor(...args){super(...(args.length?args:['2025-01-02T00:00:00Z']));}static now(){return 1735776000000;}};
const context=vm.createContext({Date:FixedDate,Buffer,AbortController,structuredClone,
 console:{log:()=>{},warn:(...x)=>warnings.push(x.join(' ')),error:()=>{}},
 process:{env:{},platform:'darwin',cwd:()=>path.join(root,'v2/.work/fake-home')},
 fetch:realProducer,setTimeout:realProducer,setInterval:realProducer,
});
const modules=new Map();
const approvedPackages=new Set(['content-type','media-typer','object-hash','dequal/lite']);
function resolve(id,from){
 if(forbidden.test(id)){boundaries.push(['denied',id]);throw new Error('Denied producer import: '+id);}
 if(id==='react')return react;
 if(id==='react/jsx-runtime')return {jsx,jsxs:jsx,Fragment:'Fragment'};
 if(id==='@raycast/api')return api;
 if(id==='@raycast/utils')return load(path.join(v1,'node_modules/@raycast/utils/dist/useFetch.js'));
 if(id==='cross-fetch')return {fetch:async(url,options)=>{
   requests.push({url,signal:!!options.signal});boundaries.push(['fetch',url]);
   if(!replies.length)throw new Error('Uncaptured fetch '+url);
   const reply=replies.shift();if(reply instanceof Error)throw reply;
   return {ok:reply.status===undefined||reply.status===200,statusText:reply.statusText||'',headers:{get:()=>reply.contentType===undefined?'application/json':reply.contentType},json:async()=>reply.data,text:async()=>JSON.stringify(reply.data)};
 }};
 if(id==='./showFailureToast')return {showFailureToast:(error,options)=>{toasts.push({error,options});boundaries.push(['toast',error.message]);}};
 if(id==='crypto')return {createHash:crypto.createHash}; // pure hash only; no random/OS methods
 if(approvedPackages.has(id))return load(require.resolve(id,{paths:[path.join(v1,'node_modules')]}));
 if(id.startsWith('.')){
   const base=path.resolve(path.dirname(from),id);
   const file=[base,base+'.ts',base+'.tsx',base+'.js',path.join(base,'index.ts'),path.join(base,'index.tsx')].find(p=>fs.existsSync(p)&&fs.statSync(p).isFile());
   if(file && (file.startsWith(v1+'/src/')||file.startsWith(v1+'/node_modules/@raycast/utils/dist/')))return load(file);
 }
 throw new Error('Uncaptured import '+id+' from '+from);
}
function load(filename){
 if(modules.has(filename))return modules.get(filename).exports;
 const source=fs.readFileSync(filename,'utf8');const module={exports:{}};modules.set(filename,module);
 const code=/\.tsx?$/.test(filename)?ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022,jsx:ts.JsxEmit.ReactJSX}}).outputText:source;
 const fn=vm.runInContext('(function(require,module,exports){'+code+'\n})',context,{filename,timeout:2000});
 fn(id=>resolve(id,filename),module,module.exports);return module.exports;
}
const own=name=>load(path.join(v1,'src',name));
const plain=v=>JSON.parse(JSON.stringify(v));
const children=node=>[node.props.children].flat(Infinity).filter(Boolean);
function walk(node,out=[]){if(!node||typeof node!=='object')return out;out.push(node);for(const n of children(node))walk(n,out);return out;}
async function settle(h,fn){let value;for(let i=0;i<12;i++){value=h.render(fn);await new Promise(setImmediate);}return h.render(fn);}
(async()=>{
 assert.equal(Intl.DateTimeFormat().resolvedOptions().locale,'en-US');
 assert.equal(Intl.DateTimeFormat().resolvedOptions().timeZone,process.env.TZ);
 // Regression: no uncaptured native module can execute before/after oracle load.
 for(const id of ['fs','node:fs','fs/promises','os','child_process','http','https','net','tls','dns','worker_threads','process','module'])assert.throws(()=>resolve(id,'boundary'),/Denied/);
 assert.throws(()=>resolve('uncaptured-package','boundary'),/Uncaptured/);
 assert.throws(()=>vm.runInContext('fetch("https://example.com")',context),/TRIPWIRE/);
 const helpers=own('helpers/index.ts');const hook=own('hooks/useFetchNodeVersions/index.ts');const validators=own('hooks/useFetchNodeVersions/validators.ts');
 const elements=fixture.map((row,i)=>hook.createNodeVersionElement(row,i,fixture));
 const cases=[null,{},[],[null],[[]],[123],[{}],[{version:1,date:''}],[{version:'v1.0.0',date:1}],[{version:'v1.0.0',date:'',lts:true}],[{version:'v1.0.0',date:'',lts:null}],[{version:'',date:''}],[{version:'v1.0.0',date:'',lts:''}],[{version:'v1.0.0',date:'',lts:false}],[{version:'v1.0.0',date:''}]];
 const validation=cases.map(input=>{try{return {input,result:validators.isNodeVersionsResponseValid(input)};}catch(e){return {input,error:e.message};}});
 // Index identity is captured separately: duplicates must not collapse.
 const grouping=helpers.createVersionsBySection(elements).map(([key,rows])=>[key,rows.map(r=>elements.indexOf(r))]);
 const prototype=[];
 for(const version of ['__proto__','constructor','toString']){try{helpers.createVersionsBySection([{version}]);prototype.push({version,ok:true});}catch(e){prototype.push({version,error:e.message});}}
 const Row=own('components/NodeVersionListItem.tsx').NodeVersionListItem;
 const rows=elements.map(props=>Row(props));
 const rowProjection=rows.map(row=>({title:row.props.title,icon:row.props.icon,keywords:row.props.keywords,accessories:row.props.accessories,
 actions:walk(row.props.actions).filter(x=>x.type.startsWith?.('Action.')).map(a=>({type:a.type,title:a.props.title,content:a.props.content}))}));
 const Command=own('index.tsx').default;const h=new Hooks();replies.push({data:fixture});
 let view=await settle(h,()=>Command());assert.equal(requests.length,1);
 const views={};
 for(const filter of ['latest','lts','']){
  view.props.searchBarAccessory.props.onChange(filter);view=await settle(h,()=>Command());
  const sections=walk(view).filter(x=>x.type==='List.Section');
  views[filter]=sections.map(section=>({title:section.props.title,versions:children(section).map(n=>n.props.version)}));
 }
 view.props.searchBarAccessory.props.onChange('invalid');view=await settle(h,()=>Command());assert.equal(requests.length,1);assert(warnings.some(x=>x.includes('Invalid filter value')));
 // Actual own hooks + complete locked useFetch/useCachedPromise/usePromise chain.
 // API Cache is an in-memory seam, React hook scheduling is finite, no timers.
 h.unmount();const h2=new Hooks();replies.push({status:500,statusText:'Internal Server Error'});
 let failed=await settle(h2,()=>Command());assert.equal(requests.length,2);
 const failNodes=walk(failed);assert(failNodes.some(x=>x.type==='List.EmptyView'&&x.props.title==='Internal Server Error'));
 assert(failNodes.some(x=>x.type==='List.Section')); // prior cached feed retained on failure
 assert.equal(toasts.at(-1).options.primaryAction.title,'Retry');
 replies.push({data:fixture});toasts.at(-1).options.primaryAction.onAction({hide:()=>{}});await settle(h2,()=>Command());assert.equal(requests.length,3);
 const DetailComponent=own('components/NodeReleaseDetail.tsx').NodeReleaseDetail;
 const dh=new Hooks();replies.push({data:release});
 let detail=await settle(dh,()=>DetailComponent({version:fixture[0].version,tagListItems:elements[0].tagList.map(t=>({text:t.label,color:t.color}))}));
 const detailProjection={markdown:detail.props.markdown,navigationTitle:detail.props.navigationTitle,
 metadata:walk(detail.props.metadata).filter(n=>n.type!=='Detail.Metadata').map(n=>({type:n.type,title:n.props.title,text:n.props.text,target:n.props.target})),
 actions:walk(detail.props.actions).filter(n=>n.type.startsWith?.('Action.')).map(n=>({type:n.type,title:n.props.title,url:n.props.url,content:n.props.content}))};
 assert.equal(requests.at(-1).url,'https://api.github.com/repos/nodejs/node/releases/tags/v24.1.0');
 dh.unmount();const eh=new Hooks();replies.push({status:404,statusText:'Not Found'});
 let errorDetail=await settle(eh,()=>DetailComponent({version:'v22.5.0',tagListItems:[]}));
 assert.equal(errorDetail.props.actions,undefined);assert(errorDetail.props.markdown.endsWith('Not Found'));
 const result={source:'2410143f530d3d6ea7143e25717322acc4af7512',utils:'1.12.5',clock:'2025-01-02T00:00:00Z',locale:'en-US',timezone:process.env.TZ,
 validation,grouping:plain(grouping),prototype,tagged:plain(elements),rows:plain(rowProjection),views:plain(views),detail:plain(detailProjection),
 detailError:{markdown:errorDetail.props.markdown,hasActions:!!errorDetail.props.actions,hasMetadata:!!errorDetail.props.metadata},
 docs:fixture.map(r=>helpers.createNodeDocsUrl(r.version)),filterValues:[null,true,'','latest','lts','all'].map(value=>({value,valid:helpers.isFilterValue(value)})),
 searchSeam:'Raycast client matching is not executable here. Rows supply actual title and tag-label keywords; native uses case-insensitive substring over those fields.',
 requests,cacheBehavior:'Cache is cross-mount in this in-memory API seam; every mount revalidates; keepPreviousData keeps prior feed on failure; toast provides deliberate Retry; no TTL or polling in useFetch.',boundaries};
 fs.writeFileSync(path.join(__dirname,`oracle-${process.env.TZ==='UTC'?'utc':'los-angeles'}.json`),JSON.stringify(result,null,2)+'\n');
 console.log(`source oracle: ${cases.length} validator cases, ${elements.length} rows, 3 filters, detail/actions/error, cache/retry, ${boundaries.filter(x=>x[0]==='denied').length} denied producers; ${process.env.TZ}`);
})().catch(e=>{console.error(e);process.exitCode=1;});
