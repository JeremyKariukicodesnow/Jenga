"""0.6.2 public API, KiCad and UI regression in isolated user-data directories."""
from pathlib import Path
import json, os, subprocess, time, uuid
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
QA = ROOT / '.cache' / 'qa-0.6.2' / ('run-' + str(int(time.time())))
QA.mkdir(parents=True)
env = os.environ.copy()
env.update(APPDATA=str(QA/'appdata'), JENGA_DATA_DIR=str(QA/'data'),
           JENGA_CONFIG_DIR=str(QA/'settings'), JENGA_SESSION_DIR=str(QA/'sessions'))
cli, app = (ROOT/'target/release'/name for name in ('jenga-cli.exe','Jenga.exe'))
client = subprocess.Popen([str(cli),'json','--headless','--workspace',str(QA)],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    text=True, encoding='utf-8', env=env)
def call(method, params=None):
    client.stdin.write(json.dumps({'method':method,'params':params or {}})+'\n')
    client.stdin.flush()
    result=json.loads(client.stdout.readline())
    assert 'error' not in result, (method,result)
    return result['result']

report={}
try:
    root=call('create_project',{'name':'Engineering062','folder':str(QA/'projects')})
    file=Path(root['file']); library=file.parent/'libraries/Test.kicad_sym'
    for name, numbers, x in [('Old',['1','2'],-12.7),('New',['7','8'],-15.24)] + [
        ('CustomSymbol'+str(i),['1','2'],-12.7) for i in range(8)]:
        params={'name':name,'library':'Test','scope':'project','prefix':'U','draw_body':False,
                'description':'Regression symbol with stable physical pins',
                'pins':[{'number':n,'name':signal,'kind':'passive','at':[x,y],'length_mm':2.54}
                        for n,signal,y in zip(numbers,['A','B'],[5.08,-5.08])],
                'shapes':[{'kind':'rectangle','start':[-5.08,7.62],'end':[5.08,-7.62]}]}
        if library.exists(): params['expected_revision']=call('read_library',{'path':str(library)})['revision']
        call('save_symbol',params)
    call('edit',{'actions':[{'op':'add_symbol','library_id':'Test:Old','at':at,
                            'properties':{'Reference':'U'+str(i+1)}}
                           for i,at in enumerate([[50.8,50.8],[101.6,50.8],[101.6,101.6]])]})
    pairs=[{'from':{'reference':'U1','number':'1'},'to':{'reference':'U2','number':'1'}},
           {'from':{'reference':'U1','number':'2'},'to':{'reference':'U3','number':'2'}}]
    base=call('inspect')['revision']
    preview=call('connect_block',{'connections':pairs,'expected_revision':base,'dry_run':True})
    assert preview['can_commit'],preview
    assert call('inspect')['revision']==base
    params={'preview_id':preview['preview_id'],'operation_id':str(uuid.uuid4())}
    commit=call('commit_preview',params)
    assert call('commit_preview',params)==commit
    assert call('operation_status',{'operation_id':params['operation_id']})['status']=='completed'
    call('edit',{'actions':[{'op':'replace_symbol','reference':'U3','library_id':'Test:New','pin_map':{'1':'7','2':'8'}}]})
    pairs[1]['to']['number']='8'
    verification=call('verify',{'expected_connections':pairs,'forbidden_connections':[
        {'from':{'reference':'U1','number':'1'},'to':{'reference':'U3','number':'8'}}]})
    assert verification['connections']['passed'],verification
    assert not verification['dangling_wires'],verification
    call('save')
    assert len(call('inspect',{'references':['U3'],'include_pins':False})['objects'])==1
    report.update(batch_preview_commit=True,idempotent_commit=True,replacement_preserves_nets=True,
                  combined_verification=True,compact_inspection=True)
    # Test hide/restore with the public recovery API, without touching real projects.
    saved=call('inspect')['file']
    archive=call('delete_project')
    assert any(p.get('restore_manifest')==archive['restore_manifest'] for p in call('list_archived_projects')['projects'])
    restored=call('restore_project',{'restore_manifest':archive['restore_manifest']})
    assert restored['file']==saved
    call('open',{'path':saved})
    report['archive_restore']=True
finally:
    client.stdin.close()
    client.wait(20)
    assert client.returncode==0,client.stderr.read()

kicad=Path('C:/Program Files/KiCad/10.0/bin/kicad-cli.exe')
if kicad.exists():
    netlist=QA/'verified-netlist.xml'
    result=subprocess.run([str(kicad),'sch','export','netlist','--format','kicadxml','-o',str(netlist),str(file)],capture_output=True,text=True)
    assert result.returncode==0,result.stderr
    nets=[{(n.attrib['ref'],n.attrib['pin']) for n in net.findall('node')} for net in ET.parse(netlist).findall('.//net')]
    assert any({('U1','1'),('U2','1')} <= net for net in nets),nets
    assert any({('U1','2'),('U3','8')} <= net for net in nets),nets
    assert not any({('U1','1'),('U3','8')} <= net for net in nets),nets
    report['independent_kicad10_connectivity']=True
    print('KiCad 10 confirms intended connections and isolation',flush=True)

(QA/'settings').mkdir(exist_ok=True)
(QA/'settings/settings.json').write_text(json.dumps({'recent':[str(file)],'grid':1.27,'reduced_motion':True}))
for tag,args in [('project',[str(file),'--capture-project']),
                 ('browser',[str(file),'--capture-symbol-browser']),
                 ('symbol-editor',[str(file),'--capture-symbol-editor'])]:
    shot=QA/(tag+'.png')
    arguments=args+['--capture='+str(shot)]
    command="$p=Start-Process -FilePath '"+str(app)+"' -ArgumentList "+','.join("'"+a.replace("'","''")+"'" for a in arguments)+" -WindowStyle Hidden -PassThru; $p.Id"
    pid=int(subprocess.check_output(['powershell','-NoProfile','-Command',command],env=env,text=True).strip())
    start=time.monotonic()
    while not shot.exists() and time.monotonic()-start<90:time.sleep(.2)
    assert shot.exists(),f'{tag} capture missing; process {pid}'
    subprocess.run(['powershell','-NoProfile','-Command',f'Wait-Process -Id {pid} -Timeout 30 -ErrorAction SilentlyContinue; exit 0'],env=env,check=True)
    report[tag+'_screenshot']=str(shot)
    print('Captured '+tag,flush=True)
(QA/'report.json').write_text(json.dumps(report,indent=2))
(QA.parent/'report.json').write_text(json.dumps(report,indent=2))
print(json.dumps(report,indent=2))
