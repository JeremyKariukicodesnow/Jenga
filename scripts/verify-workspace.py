from pathlib import Path
import json,os,subprocess,time,shutil
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'.cache/qa-0.6.1';BASE.mkdir(exist_ok=True)
QA=BASE/('run-'+str(int(time.time())));QA.mkdir(exist_ok=True)
env=os.environ.copy();env.update(APPDATA=str(QA/'appdata'),JENGA_DATA_DIR=str(QA/'data'),JENGA_CONFIG_DIR=str(QA/'settings'),JENGA_SESSION_DIR=str(QA/'sessions'))
profile=os.environ.get('JENGA_QA_PROFILE','release');cli=ROOT/'target'/profile/'jenga-cli.exe';app=ROOT/'target'/profile/'Jenga.exe'
p=subprocess.Popen([str(cli),'json','--headless','--workspace',str(QA)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf8',env=env)
def call(method,params={}):
 p.stdin.write(json.dumps({'id':1,'method':method,'params':params})+'\n');p.stdin.flush();v=json.loads(p.stdout.readline());assert 'error' not in v,(method,v);return v['result']
name='WorkspaceQA'+str(int(time.time()));root=call('create_project',{'name':name,'folder':str(QA/'projects')});rootfile=Path(root['file']);report={}
call('create_page',{'name':'Sheet2','hierarchical':True,'expected_revision':root['revision']});call('create_page',{'name':'Sheet3','hierarchical':False})
info=call('project_info');assert len(info['pages'])==3
call('export_pages',{'path':str(QA/'all-pages.pdf')});call('export_pages',{'path':str(QA/'selected-pages.pdf'),'pages':['Sheet2','Sheet3']});report['three_native_pages']=True;print('Native page exports passed',flush=True)
pins=[{'number':'1','name':'VCC','kind':'passive','at':[-12.7,5.08]},{'number':'2','name':'GND','kind':'passive','at':[-12.7,-5.08]}]
sym=call('save_symbol',{'shapes':[{'kind':'circle','center':[0,0],'radius_mm':1.27},{'kind':'arc','start':[0,0],'mid':[1,1],'end':[2,0]},{'kind':'polygon','points':[[0,0],[2,0],[1,2]]},{'kind':'text','at':[0,12.7],'text':'Room Sensor'}],'name':'RoomSensor','library':'ProjectSensors','scope':'project','description':'Reusable room sensor interface','pins':pins,'footprint':'TerminalBlock_Phoenix:TerminalBlock_Phoenix_MKDS-1,5-2_1x02_P5.00mm_Horizontal'})
copy_args={'id':'ProjectSensors:RoomSensor','library':'PersonalSensors','scope':'personal','license':'CC0-1.0','attribution':'QA original'}
personal=QA/'appdata/Jenga/libraries/PersonalSensors.kicad_sym'
if personal.exists():copy_args['expected_revision']=call('read_library',{'path':str(personal)})['revision']
copy=call('copy_symbol',copy_args)
second=call('create_project',{'name':name+'Other','folder':str(QA/'projects')})
found=call('search_symbols',{'query':'RoomSensor'});assert any(x['id']=='PersonalSensors:RoomSensor' for x in found['results']);report['symbol_searchable_in_another_project']=True
call('edit',{'actions':[{'op':'add_symbol','library_id':'PersonalSensors:RoomSensor','at':[60.96,60.96]},{'op':'text','text':'Shared sensor / imported from another project','at':[30.48,25.4],'size_mm':2.54}]});call('save');report['shared_symbol_placed']=True;print('Cross-project symbol reuse passed',flush=True)
call('open',{'path':str(rootfile)})
# Real regression circuit, edited only in an isolated copy.
original=Path('C:/Users/Jeremy/Documents/SmartHome_UNO/exports/SmartHome_UNO_12V_original.kicad_sch')
if original.exists():
 test=QA/'routing-regression.kicad_sch';shutil.copy2(original,test);call('open',{'path':str(test)});before=call('connectivity');t=time.monotonic()
 for i in range(1,8):
  print('Routing relay',i,flush=True)
  call('edit',{'actions':[{'op':'connect_pins','from':{'reference':'U1','number':str(17-i)},'to':{'reference':'K'+str(i),'number':'2' if i<6 else '9'}}]})
 after=call('connectivity');print('Seven explicit relay routes completed',flush=True)
 def partition(nets):return sorted(sorted((p['reference'],p['number']) for p in n['pins']) for n in nets['nets'] if n['pins'])
 assert partition(before)==partition(after);report['seven_full_relay_routes_preserve_all_nets']=True;report['routing_seconds']=round(time.monotonic()-t,2);call('save')
call('open',{'path':str(rootfile)})
report['skill_contains_explicit_wiring']='Full explicit wiring is the default' in call('view_skill')['text']
call('export_skill',{'path':str(QA/('skill-'+name+'.md'))})
p.stdin.close();p.wait(20);assert p.returncode==0
kicad=Path('C:/Program Files/KiCad/10.0/bin/kicad-cli.exe')
if kicad.exists():
 result=subprocess.run([str(kicad),'sch','export','netlist','--format','kicadxml','-o',str(QA/'native-project-netlist.xml'),str(rootfile)],capture_output=True,text=True);assert result.returncode==0,result.stderr;report['kicad_parses_native_hierarchy']=True
 result=subprocess.run([str(kicad),'sch','export','netlist','--format','kicadxml','-o',str(QA/'custom-symbol-netlist.xml'),second['file']],capture_output=True,text=True);assert result.returncode==0,result.stderr;report['kicad_parses_custom_symbol_shapes']=True
settings=QA/'settings';settings.mkdir(exist_ok=True);(settings/'settings.json').write_text(json.dumps({'recent':[str(rootfile),second['file']],'grid':1.27,'reduced_motion':False}))
for tag,args in [('home',[]),('project',[str(rootfile),'--capture-project']),('symbols',['--capture-symbol-editor']),('browser',['--capture-symbol-browser']),('delete',[str(rootfile),'--capture-delete']),('editor',[second['file']]),('pages',[str(rootfile),'--capture-page-manager'])]:
 shot=QA/(tag+'.png');cmd=[str(app),*args,'--capture='+str(shot)]
 command="$p=Start-Process -FilePath '"+str(app)+"' -ArgumentList "+','.join("'"+a.replace("'","''")+"'" for a in cmd[1:])+" -WindowStyle Hidden -PassThru; $p.Id"
 print('Capturing '+tag,flush=True)
 pid=int(subprocess.check_output(['powershell','-NoProfile','-Command',command],env=env,text=True).strip());started=time.monotonic()
 while not shot.exists() and time.monotonic()-started<120:time.sleep(.2)
 assert shot.exists(),tag+' screenshot not produced'
 subprocess.run(['powershell','-NoProfile','-Command',f'Wait-Process -Id {pid} -Timeout 30 -ErrorAction SilentlyContinue; exit 0'],env=env,check=True)
 report[tag+'_screenshot']=str(shot)
(QA/'report.json').write_text(json.dumps(report,indent=2));(BASE/'report.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
