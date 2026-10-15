"""Native PCB engineering, exports and live background receipts; isolated test data."""
from pathlib import Path
import json, os, subprocess, time, uuid
ROOT=Path(__file__).resolve().parent.parent
QA=ROOT/'.cache/qa-0.7.1'/('run-'+str(int(time.time())))
QA.mkdir(parents=True)
env=os.environ.copy();env.update(JENGA_DATA_DIR=str(QA/'data'),JENGA_CONFIG_DIR=str(QA/'settings'),JENGA_SESSION_DIR=str(QA/'sessions'),APPDATA=str(QA/'appdata'))
binary_root=Path(os.environ.get('JENGA_QA_BIN',str(ROOT/'target/release')))
cli=binary_root/'jenga-cli.exe';app=binary_root/'Jenga.exe'
# Exercise bundled discovery without this machine's installed KiCad paths.
env['ProgramFiles']=str(QA/'no-installed-kicad')
for version in ['10','9','8','7','6']:env['KICAD'+version+'_3DMODEL_DIR']=str(QA/'no-installed-models')
report={};pid=None
class Client:
 def __init__(self,headless=True):
  self.p=subprocess.Popen([str(cli),'json','--workspace',str(QA)]+(['--headless'] if headless else []),stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf-8',env=env)
 def call(self,method,params=None,allow_error=False):
  self.p.stdin.write(json.dumps({'method':method,'params':params or {}})+'\n');self.p.stdin.flush();v=json.loads(self.p.stdout.readline())
  if not allow_error:assert 'error' not in v,(method,v)
  return v if allow_error else v['result']
 def close(self):self.p.stdin.close();self.p.wait(30)
c=Client()
try:
 bundled=c.call('footprint_3d',{'id':'Resistor_SMD:R_0805_2012Metric','project_folder':str(QA)})
 assert all(m['status']=='available' and 'libraries' in m['resolved_path'] for m in bundled['backend']['models']),bundled
 report['bundled_models_without_kicad']=True
 project=c.call('create_project',{'name':'Pcb070','folder':str(QA/'projects')});sch=Path(project['file']);folder=sch.parent
 c.call('save_symbol',{'name':'Dual','library':'Test','scope':'project','prefix':'U','draw_body':True,'pins':[{'number':'1','name':'A','kind':'passive','at':[-10.16,2.54],'length_mm':2.54},{'number':'2','name':'B','kind':'passive','at':[-10.16,-2.54],'length_mm':2.54}]})
 c.call('footprint_save',{'name':'Dual','library':'Test','pads':[{'number':'1','kind':'thru_hole','shape':'circle','at':[-2,0],'size':[2,2],'drill_mm':0.8},{'number':'2','kind':'thru_hole','shape':'circle','at':[2,0],'size':[2,2],'drill_mm':0.8}],'graphics':[{'kind':'rectangle','start':[-3,-1.5],'end':[3,1.5],'layer':'F.Fab'}]})
 model=(binary_root/'libraries/3dmodels/Resistor_SMD.3dshapes/R_0805_2012Metric.step') if (binary_root/'libraries/3dmodels').is_dir() else ROOT/'resources/libraries/3dmodels/Resistor_SMD.3dshapes/R_0805_2012Metric.step'
 assert model.exists(),'Bundled component models are required'
 if model.exists():
  imported=c.call('model_import',{'path':str(model)})
  resource=c.call('read_library',{'path':str(folder/'libraries/Test.pretty/Dual.kicad_mod')})
  source=resource['source'].rstrip();source=source[:-1]+'(model "'+imported['reference']+'" (offset (xyz 0 0 0)) (scale (xyz 1 1 1)) (rotate (xyz 0 0 0))))'
  c.call('footprint_save',{'name':'Dual','library':'Test','native_source':source,'expected_revision':resource['revision']})
  fp3d=c.call('footprint_3d',{'id':'Test:Dual'});assert fp3d['mesh']['triangle_count']>12;report['native_footprint_3d']=True
 c.call('edit',{'actions':[{'op':'add_symbol','library_id':'Test:Dual','at':at,'properties':{'Reference':'U'+str(i+1),'Value':'Terminal'}} for i,at in enumerate([[50.8,50.8],[101.6,50.8],[152.4,50.8]])]})
 pairs=[{'from':{'reference':'U1','number':'1'},'to':{'reference':'U2','number':'1'}},{'from':{'reference':'U1','number':'2'},'to':{'reference':'U3','number':'2'}}]
 c.call('connect_block',{'connections':pairs})
 inspect=c.call('inspect');unused=[{'op':'no_connect','at':p['at_mm']} for o in inspect['objects'] for p in o.get('pins',[]) if (o.get('reference'),p['number']) in [('U2','2'),('U3','1')]]
 c.call('edit',{'actions':unused});c.call('save')
 pages=c.call('create_page',{'name':'Independent'});assert not pages['hierarchical'];assert not any(o['kind']=='sheet' for o in c.call('inspect')['objects']);report['default_pages_independent']=True
 c.call('pcb_assign_footprints',{'footprints':{r:'Test:Dual' for r in ['U1','U2','U3']}});c.call('save')
 preview=c.call('build_pcb_from_schematic',{'path':str(folder/'Pcb070.kicad_pcb'),'width_mm':80,'height_mm':60,'dry_run':True})
 assert preview['can_commit'],preview
 commit=c.call('pcb_commit',{'preview_id':preview['preview_id'],'operation_id':str(uuid.uuid4())});board=folder/'Pcb070.kicad_pcb'
 verify=c.call('pcb_verify',{'expected_connections':pairs,'external':True});(QA/'verification.json').write_text(json.dumps(verify,indent=2),encoding='utf8');assert verify['external_kicad_checked'],verify
 assert verify['connections']['passed'];assert not verify['unrouted'];report['one_call_build']=True
 c.call('pcb_save');c.call('pcb_export',{'path':str(QA/'manufacturing'),'format':'manufacturing','allow_unverified':True})
 assert list((QA/'manufacturing').glob('*.gbr'));assert list((QA/'manufacturing').glob('*.drl'));assert (QA/'manufacturing/placement.csv').exists();report['manufacturing_artifacts']=True
 for format in ['native','png','svg','pdf','bom','step','glb','stl','vrml','brep','ply']:
  out=QA/('export-'+format+('.kicad_pcb' if format=='native' else '.'+format));c.call('pcb_export',{'path':str(out),'format':format});assert out.exists(),(format,out)
  print('export',format,flush=True)
 report['all_exports']=True
 c.call('pcb_fill_zones');c.call('pcb_save');reopened=c.call('pcb_open',{'path':str(board)});assert len([o for o in reopened['objects'] if o['kind']=='footprint'])==3
 assert c.call('pcb_verify',{'expected_connections':pairs})['connections']['passed'];report['kicad_latest_roundtrip']=True
 mesh=c.call('pcb_3d',{'path':str(QA/'assembly.glb')});bounds=mesh['mesh']['bounds_mm'];assert 75<bounds[1][0]-bounds[0][0]<85,bounds
 assert 55<bounds[1][1]-bounds[0][1]<65 and bounds[1][2]-bounds[0][2]<5,bounds
 report['engineering_z_up']=True
 (QA/'assembly-info.json').write_text(json.dumps(mesh,indent=2),encoding='utf8');c.close()
 command=f"$p=Start-Process -FilePath '{app}' -ArgumentList '\"{board}\"' -WindowStyle Hidden -PassThru; $p.Id"
 pid=int(subprocess.check_output(['powershell','-NoProfile','-Command',command],env=env,text=True).strip())
 live=Client(False)
 for _ in range(100):
  state=live.call('ui_state');
  if state.get('running_app') and state.get('pcb'):break
  time.sleep(.1)
 operation=live.call('pcb_3d');assert operation['status']=='pending',operation
 before=time.monotonic();live.call('pcb_inspect',{'references':['U1']});assert time.monotonic()-before<3
 for _ in range(1000):
  status=live.call('operation_status',{'operation_id':operation['operation_id']})
  if status['status']!='pending':break
  time.sleep(.1)
 assert status['status']=='completed',status;report['live_background_receipts']=True
 live.call('ui_action',{'action':'show_3d'});live.call('screenshot',{'path':str(QA/'pcb-3d.png'),'interface':True})
 live.call('ui_action',{'action':'dismiss_dialogs'});live.call('ui_action',{'action':'show_project_dashboard'});live.call('screenshot',{'path':str(QA/'project.png'),'interface':True})
 live.call('ui_action',{'action':'show_footprint_browser'});live.call('screenshot',{'path':str(QA/'footprints.png'),'interface':True})
 live.call('ui_action',{'action':'dismiss_dialogs'});live.call('ui_action',{'action':'show_symbol_browser'});live.call('screenshot',{'path':str(QA/'symbols.png'),'interface':True})
 live.close()
 (QA/'report.json').write_text(json.dumps(report,indent=2),encoding='utf8');print('PCB_QA',QA,report,flush=True)
finally:
 if c.p.poll() is None:c.close()
 if pid:subprocess.run(['powershell','-NoProfile','-Command',f'Stop-Process -Id {pid} -ErrorAction SilentlyContinue'],env=env)
