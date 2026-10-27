/* Executes the actual committed List/Detail/Surface/ActionOverlay/Markdown
 * modules in jsdom. Only unrelated form/grid/color and cosmetic primitives are
 * stubs. All host clipboard/open services are capture-only. */
const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const ROOT=path.resolve(__dirname,'..');const deps=path.join(ROOT,'.work/renderer-deps/node_modules');
const req=n=>require(path.join(deps,n));const {JSDOM}=req('jsdom');const ts=req('typescript');
(async()=>{
 const dom=new JSDOM('<div id="root"></div>',{url:'https://qa.invalid/',runScripts:'outside-only'});
 for(const k of ['window','document','navigator','Element','HTMLElement','DOMParser','Node','Event','MouseEvent','KeyboardEvent'])Object.defineProperty(global,k,{value:dom.window[k],configurable:true});
 global.IS_REACT_ACT_ENVIRONMENT=true;global.React=req('react');window.HTMLElement.prototype.scrollIntoView=()=>{};
 const React=global.React,{act}=React,{createRoot}=req('react-dom/client');
 const effects=[];window.tezbar={clipboardWriteText:async text=>{effects.push(['copy',text]);return {ok:true};},openExternalUrl:async url=>{effects.push(['open',url]);return {ok:true};},getAppIconDataUrl:async()=>null};
 window.fetch=()=>{throw Error('uncaptured renderer fetch');};
 const modules=Object.assign({}, {react:React,'react/jsx-runtime':req('react/jsx-runtime'),marked:await import(path.join(deps,'marked/lib/marked.esm.js')),'marked-highlight':await import(path.join(deps,'marked-highlight/src/index.js')),'highlight.js/lib/common':req('highlight.js/lib/common'),'dompurify':req('dompurify')});
 const cosmetics={cx:(...x)=>x.filter(Boolean).join(' '),ViewHeader:({title,onBack})=>React.createElement('header',null,React.createElement('button',{onClick:onBack},'Back'),title),Hint:({children})=>children,HintBar:({children})=>children,Kbd:({children})=>children};
 const node=(type,props,children=[])=>({type,props,children});const fixture=JSON.parse(fs.readFileSync(path.join(ROOT,'.work/pipes/detail-view.json')));
 const malicious=JSON.parse(fs.readFileSync(path.join(ROOT,'.work/pipes/malicious-view.json')));
 async function run(patched){
  const cache=new Map();
  function load(file){
   if(cache.has(file))return cache.get(file).exports;
   const replacement=path.join(ROOT,'.work/patched-host',file);const original=path.join(ROOT,'.work/host',file);
   const source=fs.readFileSync(patched&&fs.existsSync(replacement)?replacement:original,'utf8');
   const code=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022,esModuleInterop:true,jsx:ts.JsxEmit.ReactJSX}}).outputText;
   const module={exports:{}};cache.set(file,module);
   function resolve(id){
    if(modules[id])return modules[id];
    if(id.endsWith('ui/primitives'))return cosmetics;
    if(id.endsWith('colorConverter'))return {buildColorConversionResults:()=>[],buildColorFormatRows:()=>[]};
    if(id==='./form-runtime')return {FormRuntime:()=>null};
    if(id==='./grid-runtime')return {GridRuntime:()=>null};
    if(id==='./hooks')return {NavigationContext:React.createContext({}),setRuntimeContext:()=>{}};
    if(id.startsWith('.')){let p=path.normalize(path.join(path.dirname(file),id));for(const ext of ['.tsx','.ts'])if(fs.existsSync(path.join(ROOT,'.work/host',p+ext)))return load(p+ext);}
    throw Error('Uncaptured renderer import '+id);
   }
   vm.runInThisContext('(function(require,module,exports){'+code+'\n})',{filename:file})(resolve,module,module.exports);return module.exports;
  }
  const {ExtensionRuntimeSurface}=load('src/renderer/src/raycast-api/index.tsx');
  const {ListRuntime}=load('src/renderer/src/raycast-api/list-runtime.tsx');
  let root=createRoot(document.getElementById('root'));
  const invoked=[],search=[],back=[];
  const render=async view=>act(async()=>root.render(React.createElement(ExtensionRuntimeSurface,{sessionId:'s',title:view.title,extensionId:'com.tezbar.native.node-release-notes',commandName:'index',root:view.root,actions:view.actions,onBack:()=>back.push('host'),onSearchTextChanged:q=>search.push(q),onLoadMore:()=>{throw Error('unexpected load more');},onInvokeAction:(id,values)=>invoked.push([id,values])})));
  const click=async text=>{const button=[...document.querySelectorAll('button')].find(b=>b.textContent===text);assert(button,'missing '+text);await act(async()=>button.dispatchEvent(new window.MouseEvent('click',{bubbles:true})));};
  await render(fixture);await click('Enter');
  const mouseEventBug=typeof invoked[0][0]!=='string';assert.equal(mouseEventBug,!patched);
  await click('Back');assert.equal(invoked.at(-1)[0],fixture.root.props.backActionId);
  assert(document.body.textContent.includes('Published'));assert(document.body.textContent.includes('Current'));
  await click('Cmd+K');const visible=[...document.querySelectorAll('li button')].map(b=>b.textContent);assert.deepEqual(visible,fixture.actions.map(a=>a.title));
  await act(async()=>window.dispatchEvent(new window.KeyboardEvent('keydown',{key:'Escape',bubbles:true})));
  const metadataLink=[...document.querySelectorAll('a')].find(a=>a.textContent==='https://nodejs.org/docs/v24.1.0/api/');
  await act(async()=>metadataLink.dispatchEvent(new window.MouseEvent('click',{bubbles:true,cancelable:true})));assert.deepEqual(effects.at(-1),['open','https://nodejs.org/docs/v24.1.0/api/']);
  await render(malicious);
  assert.equal(document.querySelectorAll('script,iframe,img,a[href^="javascript"],a[href^="file"],a[href^="tezbar"],button[onclick]').length,0);
  const article=document.querySelector('article');assert(article.textContent.includes('<script>globalThis.pwned=1</script>'));
  assert.equal(article.querySelectorAll('a,img').length,0);assert.equal(article.querySelectorAll('button').length,1);
  const codeCopy=article.querySelector('button.tezbar-md-code-copy');assert(codeCopy);const before=effects.length;
  await act(async()=>codeCopy.dispatchEvent(new window.MouseEvent('click',{bubbles:true})));
  assert.equal(effects.length,before+1);assert.deepEqual(effects.at(-1),['copy','<script>copy-this-only</script>\n']);
  assert.equal(global.pwned,undefined);assert.equal(invoked.filter(x=>x[0]===fixture.actions[2].id).length,0); // primary Copy Version is independent
  await act(async()=>root.unmount());root=createRoot(document.getElementById('root'));
  const accessory=node('List.Dropdown',{value:'lts',actionId:'filter'},[node('List.Dropdown.Item',{title:'Latest',value:'latest'}),node('List.Dropdown.Item',{title:'LTS',value:'lts'}),node('List.Dropdown.Item',{title:'All',value:''})]);
  const list=node('List',{searchText:'',searchBarPlaceholder:'Search Node Version',searchBarAccessory:accessory,__hasServerSearch:true},[node('List.Section',{title:'v24'},[node('List.Item',{id:'r',title:'v24.1.0',icon:'file:///verified/resources/nodejs-icon.svg',accessories:[{tag:{value:'Current',color:'yellow'}},{text:{value:'1/1/25'},tooltip:'January 1, 2025'}],actionIds:['show','copy']})])]);
  await act(async()=>root.render(React.createElement(ListRuntime,{root:list,title:'Node Versions',commandName:'index',onBack:()=>{},onRunPrimaryAction:(id,values)=>invoked.push([id,values]),actions:[{id:'show',title:'Show Release Notes for v24.1.0'}],onOpenActions:()=>{},onSearchTextChanged:q=>search.push(q),onLoadMore:()=>{}})));
  const lts=[...document.querySelectorAll('button')].find(b=>b.textContent==='LTS');const initialControlled=lts.className.includes('bg-white/[0.14]');assert.equal(initialControlled,patched);
  assert.equal(!!document.querySelector('[title="January 1, 2025"]'),patched);
  assert(document.body.textContent.includes('v24'));assert.equal(document.querySelector('img').getAttribute('src'),'file:///verified/resources/nodejs-icon.svg');
  await click('All');assert.deepEqual(invoked.at(-1),['filter',{value:''}]);
  await click('v24.1.0Current1/1/25');assert.equal(invoked.at(-1)[0],'show');
  const input=document.querySelector('input');const setter=Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value').set;
  await act(async()=>{setter.call(input,'Current');input.dispatchEvent(new window.Event('input',{bubbles:true}));await new Promise(r=>setTimeout(r,250));});
  await act(async()=>new Promise(r=>setTimeout(r,250)));assert(search.includes('Current'));
  await act(async()=>root.unmount());
  return {baseline:!patched,mouseEventBug,controlledDropdown:initialControlled,dateTooltip:patched,actualMarkdownSanitized:true,codeCopyBytes:'<script>copy-this-only</script>\\n',stubs:['unrelated form/grid/color','cosmetic primitives','capture-only host services'],nativeGui:false};
 }
 const before=await run(false),after=await run(true);fs.writeFileSync(path.join(__dirname,'renderer-results.json'),JSON.stringify({host:'ea100cee3f69711de12485ba061950c5fc65258f',before,after},null,2)+'\n');
 console.log('Actual renderer before/after passed; Markdown malicious fixture, code-copy, metadata, actions, Back, sections, Dropdown, local search');dom.window.close();
})().catch(e=>{console.error(e);process.exitCode=1;});
