"""Isolated desktop checks for the 555 board, fullscreen 3D and footprint repair UI."""
from pathlib import Path
import json,os,subprocess,time,sys
root=Path(__file__).resolve().parent.parent
qa=Path(sys.argv[1]).resolve()
board=qa/'555 Pulse Generator/555 Pulse Generator.kicad_pcb'
env=os.environ.copy();env.update(JENGA_DATA_DIR=str(qa/'ui-data'),JENGA_CONFIG_DIR=str(qa/'ui-config'),JENGA_SESSION_DIR=str(qa/'ui-sessions'))
pid=int(subprocess.check_output(['powershell','-NoProfile','-Command',f"$p=Start-Process -FilePath '{root/'target/release/Jenga.exe'}' -ArgumentList '\"{board}\"' -WindowStyle Hidden -PassThru; $p.Id"],env=env,text=True).strip())
client=subprocess.Popen([str(root/'target/release/jenga-cli.exe'),'json','--workspace',str(qa)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf8',env=env)
def call(method,params=None):
 client.stdin.write(json.dumps({'method':method,'params':params or {}})+'\n');client.stdin.flush();v=json.loads(client.stdout.readline());assert 'error' not in v,(method,v);return v['result']
def complete(v):
 for _ in range(1000):
  r=call('operation_status',{'operation_id':v['operation_id']})
  if r['status']!='pending':assert r['status']=='completed',r;return r['result']
  time.sleep(.1)
 raise AssertionError('Timed out waiting for operation receipt')
try:
 for _ in range(300):
  s=call('ui_state')
  if s.get('pid')==pid and s.get('pcb') and not s.get('indexing'):break
  time.sleep(.1)
 assert s.get('version')=='0.7.1' and s.get('pid')==pid
 call('ui_action',{'action':'dismiss_dialogs'})
 call('screenshot',{'path':str(qa/'pcb-editor-071.png'),'interface':True})
 mesh=complete(call('pcb_3d'));assert all(m['status'] in ['available','not_required'] for m in mesh['models'])
 call('ui_action',{'action':'fullscreen_3d'})
 call('screenshot',{'path':str(qa/'fullscreen-3d-071.png'),'interface':True})
 call('ui_action',{'action':'dismiss_dialogs'})
 call('ui_action',{'action':'show_project_dashboard'})
 call('screenshot',{'path':str(qa/'project-dashboard-071.png'),'interface':True})
 assert call('ui_state')['dialogs']['project_dashboard']
 maize=Path.home()/'Documents/Maize Factory/Maize Factory.kicad_sch'
 call('open',{'path':str(maize)})
 complete(call('pcb_preflight'))
 call('screenshot',{'path':str(qa/'footprint-repair-071.png'),'interface':True})
 print('UI_071_QA',qa,flush=True)
finally:
 client.stdin.close();client.wait(30)
 subprocess.run(['powershell','-NoProfile','-Command',f'Stop-Process -Id {pid} -ErrorAction SilentlyContinue'],env=env)
