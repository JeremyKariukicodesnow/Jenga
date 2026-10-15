"""Real desktop tab/window, lighting exports and desktop-independent calculation QA."""
from pathlib import Path
import subprocess,os,json,time,uuid
from PIL import Image,ImageStat
ROOT=Path(__file__).resolve().parent.parent
QA=ROOT/'.cache'/('viewer-080-'+str(int(time.time())));QA.mkdir(parents=True)
BIN=ROOT/'target/debug';BOARD=Path(os.environ.get('JENGA_QA_BOARD', str(ROOT/'docs/examples/assembly.kicad_pcb')))
assert BOARD.is_file(), 'Set JENGA_QA_BOARD to a saved PCB for viewer QA'
env=os.environ.copy();env.update(JENGA_DATA_DIR=str(QA/'data'),JENGA_CONFIG_DIR=str(QA/'settings'),JENGA_SESSION_DIR=str(QA/'sessions'))
startup=subprocess.STARTUPINFO();startup.dwFlags|=subprocess.STARTF_USESHOWWINDOW;startup.wShowWindow=0
app=subprocess.Popen([str(BIN/'Jenga.exe'),str(BOARD)],env=env,startupinfo=startup,stdout=(QA/'desktop.log').open('w'),stderr=subprocess.STDOUT)
client=subprocess.Popen([str(BIN/'jenga-cli.exe'),'json','--workspace',str(QA),'--connect','auto'],env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=(QA/'client.log').open('w'),text=True,encoding='utf8',creationflags=subprocess.CREATE_NO_WINDOW)
def call(m,p=None):
 p=dict(p or {});p.setdefault('actor','ai:release-qa');client.stdin.write(json.dumps({'method':m,'params':p})+'\n');client.stdin.flush();line=client.stdout.readline();assert line,client.poll();r=json.loads(line);assert 'error' not in r,(m,r);return r['result']
def wait(id):
 start=time.monotonic()
 while time.monotonic()-start<120:
  r=call('operation_status',{'operation_id':id})
  if r['status']!='pending':assert r['status']=='completed',r;return r['result']
  time.sleep(.1)
 raise AssertionError('job did not finish')
report={}
try:
 for _ in range(300):
  state=call('ui_state')
  if state.get('pid')==app.pid and state.get('pcb'):break
  time.sleep(.1)
 assert state.get('pid')==app.pid,state
 call('ui_action',{'action':'show_3d'})
 for _ in range(600):
  state=call('ui_state')
  if state.get('assembly',{}).get('loaded') and not state.get('pcb_pending'):break
  time.sleep(.1)
 assert state['assembly']['loaded'],state
 call('screenshot',{'path':str(QA/'3D-tab.png'),'interface':True})
 call('ui_action',{'action':'set_3d_lighting','brightness':2.0,'ambient':.8})
 call('screenshot',{'path':str(QA/'3D-brighter.png'),'interface':True})
 state=call('ui_state');assert state['assembly']['brightness']==2.0 and state['assembly']['mode']=='tab';report['tab_and_brightness']=True
 call('ui_action',{'action':'detach_3d'});time.sleep(.5);state=call('ui_state');assert state['assembly']['mode']=='window';report['separate_window']=True
 call('ui_action',{'action':'dock_3d'});time.sleep(.2)
 # This calculation belongs to the CLI, not the desktop process.
 pending=call('pcb_export_3d_view',{'background':True,'path':str(QA/'low.png'),'view':'isometric','format':'png','width':1200,'height':1000,'brightness':.5,'ambient':.7,'operation_id':str(uuid.uuid4())})
 assert pending.get('worker_owner')=='assistant_connection',pending
 state=call('ui_state');assert not state['pcb']['dirty'] and not (state.get('document') or {}).get('dirty')
 app.terminate();app.wait(15);wait(pending['operation_id']);report['calculation_survives_desktop_close']=True
 pending=call('pcb_export_3d_view',{'background':True,'path':str(QA/'high.png'),'view':'isometric','format':'png','width':1200,'height':1000,'brightness':2.0,'ambient':.7,'operation_id':str(uuid.uuid4())});wait(pending['operation_id'])
 low=sum(ImageStat.Stat(Image.open(QA/'low.png')).mean);high=sum(ImageStat.Stat(Image.open(QA/'high.png')).mean);assert high>low*1.10,(low,high)
 report['export_lighting']={'low_rgb_mean_sum':low,'high_rgb_mean_sum':high}
 print('VIEWER_QA',QA,json.dumps(report),flush=True)
 (QA/'report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
finally:
 if app.poll() is None:app.terminate();app.wait(15)
 client.stdin.close();client.wait(20)
