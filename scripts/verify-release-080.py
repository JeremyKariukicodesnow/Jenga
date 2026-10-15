"""Isolated v0.8 bridge restart/upgrade/downgrade regression checks."""
from pathlib import Path
import subprocess,os,sys,json,queue,threading,time,shutil,signal,uuid
ROOT=Path(__file__).resolve().parent.parent
QA=ROOT/'.cache'/('release-080-'+str(int(time.time())));QA.mkdir(parents=True)
BINARY=ROOT/'target/debug'
ENV=os.environ.copy();ENV.update(JENGA_DATA_DIR=str(QA/'data'),JENGA_CONFIG_DIR=str(QA/'settings'),JENGA_SESSION_DIR=str(QA/'sessions'),USERPROFILE=str(QA/'home'),LOCALAPPDATA=str(QA/'local'))
class Bridge:
 def __init__(self,exe,args):
  env=ENV|{'JENGA_WORKER_EXE':str(exe)}
  self.p=subprocess.Popen([str(BINARY/'jenga-connect.exe'),*map(str,args)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=(QA/('bridge-'+str(time.time_ns())+'.log')).open('w'),text=True,encoding='utf8',env=env,creationflags=subprocess.CREATE_NO_WINDOW)
  self.q=queue.Queue();self.n=0
  threading.Thread(target=lambda:[self.q.put(json.loads(l)) for l in self.p.stdout],daemon=True).start()
 def send(self,method,params=None):
  self.n+=1;self.p.stdin.write(json.dumps({'jsonrpc':'2.0','id':self.n,'method':method,'params':params or {}})+'\n');self.p.stdin.flush();return self.n
 def recv(self,id=None,timeout=20):
  start=time.monotonic()
  while time.monotonic()-start<timeout:
   v=self.q.get(timeout=timeout)
   if id is None or v.get('id')==id:return v
  raise TimeoutError(id)
 def call(self,method,params=None):return self.recv(self.send(method,params))
 def tool(self,name,args=None):return self.call('tools/call',{'name':'jenga_'+name,'arguments':args or {}})
 def initialize(self):return self.call('initialize',{'protocolVersion':'2025-11-25','clientInfo':{'name':'Jenga-QA','version':'1'},'capabilities':{}})
 def close(self):
  if self.p.poll() is None:self.p.stdin.close();self.p.wait(15)
def kill_worker(pid,parent):
 actual=int(subprocess.check_output(['powershell','-NoProfile','-Command',f'(Get-CimInstance Win32_Process -Filter "ProcessId = {pid}").ParentProcessId'],text=True).strip())
 assert actual==parent,(pid,actual,parent)
 os.kill(pid,signal.SIGTERM)
def changed(b):
 for _ in range(8):
  v=b.recv(timeout=20)
  if v.get('method')=='notifications/tools/list_changed':return
 raise AssertionError('missing tools/list_changed')
report={}
# Fake worker proves an interrupted edit is not replayed and keeps its operation ID.
fake=QA/'fake.py';version=QA/'version';marker=QA/'edits.jsonl';version.write_text('old')
fake.write_text('''import sys,json,os,time
from pathlib import Path
v=Path(sys.argv[1]).read_text();marker=Path(sys.argv[2])
for line in sys.stdin:
 r=json.loads(line);m=r['method']
 if 'id' not in r:continue
 if m=='initialize':out={'protocolVersion':'2025-11-25','capabilities':{'tools':{}},'serverInfo':{'version':v},'pid':os.getpid()}
 elif m=='tools/list':out={'tools':[{'name':v}]}
 elif m=='tools/call':
  with marker.open('a') as f:f.write(json.dumps({'pid':os.getpid(),'operation_id':r['params']['arguments'].get('operation_id')})+'\\n')
  time.sleep(60);out={}
 else:out={}
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':out}),flush=True)
''',encoding='utf8')
b=Bridge(sys.executable,[fake,version,marker])
try:
 init=b.initialize();assert init['result']['serverInfo']['version']=='old';pid=init['result']['pid']
 id=b.send('tools/call',{'name':'jenga_edit','arguments':{'actions':[]}})
 for _ in range(100):
  if marker.exists() and marker.read_text().strip():break
  time.sleep(.02)
 data=json.loads(marker.read_text().splitlines()[0]);uuid.UUID(data['operation_id'])
 version.write_text('new');kill_worker(pid,b.p.pid)
 error=b.recv(id);assert error['error']['data']['operation_id']==data['operation_id'];changed(b)
 assert b.call('tools/list')['result']['tools'][0]['name']=='new'
 assert len(marker.read_text().splitlines())==1
 report['interrupted_edit_not_replayed']=True;report['operation_id_retained']=True
finally:b.close()
print('BRIDGE interrupted edit passed',flush=True)
# Actual old/new CLI binaries share the same host connection and durable receipts.
worker=QA/'installed/jenga-cli.exe';worker.parent.mkdir()
old=ROOT/'dist/0.7.1/Jenga/jenga-cli.exe';new=BINARY/'jenga-cli.exe';shutil.copy2(old,worker)
b=Bridge(worker,['mcp','--workspace',str(QA),'--headless'])
try:
 assert b.initialize()['result']['serverInfo']['version']=='0.7.1'
 for label,source,want in [('upgrade',new,'0.8.0'),('downgrade',old,'0.7.1'),('repair',old,'0.7.1')]:
  receipt=str(uuid.uuid4());r=b.tool('edit',{'actions':[],'operation_id':receipt});assert r['result']['isError']
  entry=json.loads((QA/'data/Jenga/operations'/f'{receipt}.json').read_text());pid=entry['pid'];kill_worker(pid,b.p.pid)
  for attempt in range(15):
   try:shutil.copy2(source,worker);break
   except PermissionError:time.sleep(.05)
  else:raise AssertionError('worker image remained locked after process exit')
  changed(b)
  tools=b.call('tools/list')['result']['tools'];has_review=any(t['name']=='jenga_engineering_review' for t in tools)
  assert has_review==(want=='0.8.0'),(label,has_review)
  status=b.tool('operation_status',{'operation_id':receipt});assert not status['result']['isError'];assert status['result']['structuredContent']['result']['status']=='failed'
  report[label]=True;print('BRIDGE',label,'passed',flush=True)
finally:b.close()
(QA/'bridge-report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
print('BRIDGE_QA',QA,flush=True)
