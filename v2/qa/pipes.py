#!/usr/bin/env python3
"""Actual OS JSONL pipes into the real Adapter + pinned SDK, no public HTTP/OS effects."""
import json, os, pathlib, selectors, subprocess, sys, time, uuid
ROOT=pathlib.Path(__file__).resolve().parents[1]
BIN=ROOT/'target/debug/examples/pipe_fixture'
OUT=ROOT/'.work/pipes';OUT.mkdir(parents=True,exist_ok=True)
TRANSCRIPT=[]
class Pipe:
 def __init__(self,mode='',grants=None):
  self.folder=OUT/str(time.time_ns());self.folder.mkdir()
  self.err=open(self.folder/'stderr.txt','wb')
  self.p=subprocess.Popen([str(BIN),mode],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=self.err,env=dict(os.environ,TZ='UTC'),bufsize=0)
  self.sel=selectors.DefaultSelector();self.sel.register(self.p.stdout,selectors.EVENT_READ);self.buf=b'';self.num=0;self.events=[]
  hello=self.recv();assert hello['type']=='hello' and hello['protocol']==1 and hello['sdkVersion']=='0.1.1';assert hello['id']=='com.tezbar.native.node-release-notes'
  self.grants=grants if grants is not None else ['clipboard.write','shell.open']
 def send(self,v):
  TRANSCRIPT.append(['send',v]);self.p.stdin.write((json.dumps(v,ensure_ascii=False)+'\n').encode());self.p.stdin.flush()
 def recv(self,timeout=5,include_events=False):
  deadline=time.monotonic()+timeout
  while b'\n' not in self.buf:
   assert self.sel.select(max(0,deadline-time.monotonic())), 'pipe timeout'
   b=os.read(self.p.stdout.fileno(),65536);assert b,'unexpected EOF';self.buf+=b
  line,self.buf=self.buf.split(b'\n',1);v=json.loads(line);TRANSCRIPT.append(['recv',v])
  if v.get('type')=='event' and not include_events:self.events.append(v);return self.recv(timeout)
  return v
 def request(self,method,params):
  self.num+=1;v={'type':'request','id':'h:'+str(uuid.uuid4()),'method':method,'params':params};self.send(v);return v['id']
 def invoke(self,method,params):
  id=self.request(method,params);reply=self.recv();assert reply['id']==id,(id,reply);return reply
 def init(self):
  p={'protocol':1,'extensionId':'com.tezbar.native.node-release-notes','platform':'macos','packageDir':str(self.folder/'package/versions/verified'),'dataDir':str(self.folder/'data'),'tempDir':str(self.folder/'temp'),'grantedCapabilities':self.grants,'reason':'test','migrationPaths':{}}
  assert self.invoke('initialize',p)['result']['ready'] is True
 def open(self,id='s'):
  return self.invoke('session.open',{'sessionId':id,'commandName':'index','preferences':{},'argumentValues':{}})['result']
 def call(self,op,payload=None,id='s'):
  return self.invoke('session.call',{'sessionId':id,'operation':'extension:'+op,'payload':payload or {}})
 def action(self,v,title):return next(a['id'] for a in v['actions'] if a['title']==title)
 def act(self,v,title):return self.call('invoke-action',{'actionId':self.action(v,title)})['result']
 def finish(self,success=True,shutdown=False):
  if shutdown:assert self.invoke('shutdown',{}).get('result') is not None
  self.p.stdin.close();rc=self.p.wait(timeout=8);self.err.close();self.sel.close();assert (rc==0)==success,(rc,self.folder)
  return (self.folder/'stderr.txt').read_text()
 def kill(self):
  if self.p.poll() is None:self.p.kill();self.p.wait()
 def __del__(self):self.kill()

def main():
 p=Pipe();p.init();v=p.open();assert v['refreshMs'] is None
 filter_id=v['root']['props']['searchBarAccessory']['props']['actionId'];v=p.call('invoke-action',{'actionId':filter_id,'formValues':{'value':''}})['result']
 stale=p.action(v,'Copy Version');v=p.call('search-text-changed',{'searchText':'Current'})['result']
 assert p.call('invoke-action',{'actionId':stale}).get('error')
 # Two searches dispatched without waiting: older may complete or be superseded,
 # but final committed view always belongs to the newer search.
 ids=[p.request('session.call',{'sessionId':'s','operation':'extension:search-text-changed','payload':{'searchText':q}}) for q in ['Jod','Current']]
 replies=[p.recv(),p.recv()];assert {r['id'] for r in replies}==set(ids)
 v=p.call('refresh-session')['result'];assert v['root']['props']['searchText']=='Current'
 v=p.act(v,'Show Release Notes for v24.1.0');assert v['root']['type']=='Detail'
 pathlib.Path(OUT/'detail-view.json').write_text(json.dumps(v,indent=2)+'\n')
 # Reverse RPC, cancel after the request but before acknowledgement. SDK rolls
 # state back; authoritative pending bytes survive and no old action replays.
 aid=p.action(v,'Copy Version');req=p.request('session.call',{'sessionId':'s','operation':'extension:invoke-action','payload':{'actionId':aid}})
 reverse=p.recv();assert reverse['method']=='host.clipboard.write';assert reverse['params']=={'text':'v24.1.0'}
 p.send({'type':'cancel','requestId':req,'reason':'test cancellation'})
 cancelled=[p.recv(),p.recv()];assert any(r.get('type')=='cancel' and r.get('requestId')==reverse['id'] for r in cancelled)
 event=p.recv(include_events=True);assert event['event']=='session.invalidated' and event['sessionId']=='s'
 p.send({'type':'response','id':reverse['id'],'result':{'ok':True}}) # late ack
 assert p.call('invoke-action',{'actionId':aid}).get('error')
 v=p.call('refresh-session')['result'];assert 'uncertain' in v['message']
 retry=p.action(v,'Retry Copy (may repeat)');req=p.request('session.call',{'sessionId':'s','operation':'extension:invoke-action','payload':{'actionId':retry}})
 reverse2=p.recv();assert reverse2['params']==reverse['params'];p.send({'type':'response','id':reverse2['id'],'result':{'ok':True}})
 v=p.recv()['result'];assert 'message' not in v
 # Contradictory open acknowledgement is failure, not success.
 req=p.request('session.call',{'sessionId':'s','operation':'extension:invoke-action','payload':{'actionId':p.action(v,'Open Docs')}})
 rev=p.recv();assert rev['params']=={'target':'https://nodejs.org/docs/v24.1.0/api/'}
 p.send({'type':'response','id':rev['id'],'result':{'ok':True,'error':'contradiction'}})
 v=p.recv()['result'];assert 'uncertain' in v['message']
 v=p.act(v,'Dismiss uncertain action');v=p.act(v,'Back')
 assert v['root']['props']['searchText']=='Current';assert v['root']['props']['searchBarAccessory']['props']['value']==''
 assert p.call('load-more')['result']['mode']=='unchanged';p.call('refresh-session')
 assert p.invoke('session.dispose',{'sessionId':'s'}).get('result') is not None
 assert p.call('refresh-session').get('error')
 log=p.finish(shutdown=True);assert log.count('CAPTURE feed')==1 and log.count('CAPTURE detail')==1,log
 # Ordinary completed failures remain failed until deliberate retry.
 for mode,title in [('fail-feed','Retry Node Versions'),('fail-detail','Retry Release Notes')]:
  p=Pipe(mode);p.init();v=p.open()
  if mode=='fail-detail':v=p.act(v,'Show Release Notes for v24.1.0')
  assert 'failure' in v['message'];v=p.call('refresh-session')['result'];p.call('load-more');v=p.act(v,title);assert 'message' not in v
  log=p.finish();assert log.count('CAPTURE feed')==(2 if mode=='fail-feed' else 1);assert log.count('CAPTURE detail')==(2 if mode=='fail-detail' else 0)
 p=Pipe(grants=[]);p.init();v=p.open();v=p.act(v,'Copy Version');assert 'not granted' in v['message'];assert p.finish().count('CAPTURE feed')==1
 # Late detail is discarded; following refresh still shows committed list.
 p=Pipe('slow-detail');p.init();v=p.open();req=p.request('session.call',{'sessionId':'s','operation':'extension:invoke-action','payload':{'actionId':p.action(v,'Show Release Notes for v24.1.0')}})
 time.sleep(.05);p.send({'type':'cancel','requestId':req,'reason':'late detail'});assert p.recv().get('error')
 v=p.call('refresh-session')['result'];assert v['root']['type']=='List';p.finish()
 # Cancel opens repeatedly without disposal; SDK-owned authority guards release.
 p=Pipe('slow-open');p.init()
 for i in range(135):
  req=p.request('session.open',{'sessionId':'cancel-'+str(i),'commandName':'index'})
  time.sleep(.002);p.send({'type':'cancel','requestId':req,'reason':'cancel open'});assert p.recv().get('error')
 p.finish()
 # Actual malicious API fixture reaches native adapter, then renderer tests.
 p=Pipe('malicious');p.init();v=p.open();v=p.act(v,'Show Release Notes for v24.1.0');(OUT/'malicious-view.json').write_text(json.dumps(v,indent=2)+'\n');p.finish()
 # Framing: malformed JSON, CRLF, truncated and oversized frames terminate.
 for raw in [b'{no}\n',b'{}\r\n',b'{',b' '+b'x'*(16*1024*1024)+b'\n']:
  p=Pipe()
  try:p.p.stdin.write(raw);p.p.stdin.flush()
  except BrokenPipeError:pass
  p.finish(success=False)
 # Fragmented Unicode request and normal EOF path.
 p=Pipe();p.init();req={'type':'request','id':'h:00000000-0000-4000-8000-000000000001','method':'session.open','params':{'sessionId':'Қазақ 😀','commandName':'index'}}
 raw=(json.dumps(req,ensure_ascii=False)+'\n').encode()
 for i in range(0,len(raw),3):p.p.stdin.write(raw[i:i+3]);p.p.stdin.flush()
 assert p.recv()['id']=='h:00000000-0000-4000-8000-000000000001';p.finish()
 (OUT/'transcript.json').write_text(json.dumps(TRANSCRIPT,ensure_ascii=False,indent=2)+'\n')
 print('JSONL pipes passed: lifecycle, search ownership, late ack/detail, canceled opens, retries, denial, no replay, framing, EOF')
if __name__=='__main__':main()
