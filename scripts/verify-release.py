"""End-to-end Jenga public-tool regression, isolated from personal projects/settings."""
from pathlib import Path
import subprocess,json,os,shutil,time
ROOT=Path(__file__).resolve().parent.parent
QA=ROOT/'.cache/qa-0.5.1';QA.mkdir(parents=True,exist_ok=True)
env=os.environ.copy();env['JENGA_DATA_DIR']=str(QA/'data');env['JENGA_CONFIG_DIR']=str(QA/'settings');env['JENGA_SESSION_DIR']=str(QA/'sessions')
BINARY_DIR=ROOT/'target'/os.environ.get('JENGA_QA_PROFILE','release')
CLI=BINARY_DIR/'jenga-cli.exe';APP=BINARY_DIR/'Jenga.exe'
class Client:
 def __init__(self,headless=False):
  self.p=subprocess.Popen([str(CLI),'json','--workspace',str(QA)]+(['--headless'] if headless else []),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf8',env=env);self.n=0
 def call(self,method,params={}):
  self.n+=1;self.p.stdin.write(json.dumps({'id':self.n,'method':method,'params':params})+'\n');self.p.stdin.flush();result=json.loads(self.p.stdout.readline());assert 'error' not in result,(method,result);return result['result']
 def close(self):self.p.stdin.close();self.p.wait(20);assert self.p.returncode==0,self.p.stderr.read()
def launch():
 command=f"$p=Start-Process -FilePath '{APP}' -WindowStyle Hidden -PassThru; $p.Id"
 pid=int(subprocess.check_output(['powershell','-NoProfile','-Command',command],env=env,text=True).strip())
 for _ in range(100):
  sessions=json.loads(subprocess.check_output([str(CLI),'sessions'],env=env,text=True))
  if any(s['pid']==pid and s['responding'] for s in sessions):return pid
  time.sleep(.2)
 raise AssertionError('desktop failed to start')
def stop(pid):subprocess.run(['powershell','-NoProfile','-Command',f'Stop-Process -Id {pid} -ErrorAction Stop; Wait-Process -Id {pid} -Timeout 20 -ErrorAction SilentlyContinue; exit 0'],env=env,check=True)
def main():
 report={};client=Client();name='QA'+str(int(time.time()));state=client.call('create_project',{'name':name,'folder':str(QA/'projects')});path=Path(state['file'])
 state=client.call('edit',{'expected_revision':state['revision'],'actor':'ai:qa','actions':[{'op':'add_symbol','library_id':'Device:R','at':[55,50]},{'op':'add_symbol','library_id':'Device:C','at':[95,50]}]})
 route=client.call('edit',{'expected_revision':state['revision'],'actions':[{'op':'connect_pins','from':{'reference':'R1','number':'2'},'to':{'reference':'C1','number':'1'}}]})
 nets=client.call('connectivity');assert any({(p['reference'],p['number']) for p in n['pins']}=={('R1','2'),('C1','1')} for n in nets['nets']);report['pin_connection_verified']=True
 client.call('save');client.call('rename',{'name':name+'Renamed','kind':'project','move_folder':True});state=client.call('inspect');path=Path(state['file']);report['rename_saved_path']=str(path)
 copy=client.call('save_project_as',{'folder':str(QA/'copies'),'name':name+'Copy'});assert path.exists();report['copy_exists']=Path(copy['file']).exists()
 removed=client.call('delete_project');assert Path(removed['restore_manifest']).exists();report['recoverable_delete']=True
 client.call('open',{'path':str(path)});report['closed_backend']=client.call('ui_state')['running_app']==False
 pid=launch()
 try:
  state=client.call('ui_state');assert state['running_app']==True;assert state['document']['file']==str(path);report['same_connection_handoff_to_desktop']=True
  for _ in range(100):
   if not state['indexing']:break
   time.sleep(.2);state=client.call('ui_state')
  client.call('edit',{'expected_revision':state['document']['revision'],'actor':'ai:qa','actions':[{'op':'text','text':'Live handoff verified','at':[35,30]}]});client.call('save')
  client.call('ui_action',{'action':'show_connect_assistant'});client.call('screenshot',{'path':str(QA/'connect-assistant.png'),'interface':True})
  client.call('ui_action',{'action':'dismiss_dialogs'});client.call('close');client.call('screenshot',{'path':str(QA/'home.png'),'interface':True})
  client.call('open',{'path':str(path)});client.call('screenshot',{'path':str(QA/'editor.png'),'interface':True})
  # An isolated file session publishes a rename; desktop follows the path.
  external=Client(True);external.call('open',{'path':str(path)});new=external.call('rename',{'name':name+'External','kind':'project','move_folder':True});external.close();path=Path(new['path'])
  for _ in range(25):
   time.sleep(.2)
   if client.call('ui_state')['document']['file']==str(path):break
  else:raise AssertionError('desktop did not follow external project rename within five seconds')
  report['desktop_external_rename_refresh']=True
 finally:stop(pid)
 state=client.call('inspect');assert state['file']==str(path);report['same_connection_handoff_to_closed']=True;client.close()
 # Repair a copy of the real failure case, without changing the user's calculator.
 original=Path(r'C:\Users\Jeremy\Documents\MyCalculator\MyCalculator.kicad_sch')
 if original.exists() and not os.environ.get("JENGA_QA_SKIP_PERSONAL"):

  repaired=QA/'MyCalculator.kicad_sch';shutil.copy2(original,repaired);headless=Client(True);headless.call('open',{'path':str(repaired)});headless.call('save');headless.close()
  kicad=Path(r'C:\Program Files\KiCad\10.0\bin\kicad-cli.exe')
  if kicad.exists():
   result=subprocess.run([str(kicad),'sch','export','netlist',str(repaired),'-o',str(QA/'calculator.net')],capture_output=True,text=True);assert result.returncode==0,(result.stdout,result.stderr);report['calculator_copy_opens_in_KiCad']=True
 (QA/'report.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
if __name__=='__main__':main()
