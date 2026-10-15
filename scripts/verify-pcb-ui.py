"""Visual QA against an already verified board, with isolated application data."""
from pathlib import Path
import json, os, subprocess, sys, time

root = Path(__file__).resolve().parent.parent
qa = Path(sys.argv[1]).resolve()
binary = Path(os.environ.get('JENGA_QA_BIN', str(root/'target/release')))
env = os.environ.copy()
env.update(JENGA_DATA_DIR=str(qa/'data'), JENGA_CONFIG_DIR=str(qa/'settings'),
           JENGA_SESSION_DIR=str(qa/'sessions'), APPDATA=str(qa/'appdata'),
           ProgramFiles=str(qa/'no-installed-kicad'))
board = qa/'projects/Pcb070/Pcb070.kicad_pcb'
assert board.is_file(), board
command = f"$p=Start-Process -FilePath '{binary/'Jenga.exe'}' -ArgumentList '\"{board}\"' -WindowStyle Hidden -PassThru; $p.Id"
pid = int(subprocess.check_output(['powershell','-NoProfile','-Command',command],env=env,text=True).strip())
client = subprocess.Popen([str(binary/'jenga-cli.exe'),'json','--workspace',str(qa)],
    stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf8',env=env)
def call(method, params=None):
    client.stdin.write(json.dumps({'method':method,'params':params or {}})+'\n');client.stdin.flush()
    value = json.loads(client.stdout.readline())
    assert 'error' not in value, (method,value)
    return value['result']
try:
    for _ in range(200):
        state = call('ui_state')
        if state.get('pid') == pid and state.get('pcb') and not state.get('indexing'):break
        time.sleep(.1)
    assert state.get('pid') == pid, state
    # Resource edits must refresh browsing without leaving the project dashboard.
    call('ui_action',{'action':'show_project_dashboard'})
    probe = board.parent/'libraries/RefreshProbe.pretty/RefreshProbe.kicad_mod'
    revision = call('read_library',{'path':str(probe)})['revision'] if probe.exists() else None
    operation = call('footprint_save',{'name':'RefreshProbe','library':'RefreshProbe','scope':'project',
        'expected_revision':revision,'project_folder':str(board.parent),
        'pads':[{'number':'1','kind':'smd','at':[0,0],'size':[1,1]}]})
    for _ in range(1000):
        status = call('operation_status',{'operation_id':operation['operation_id']})
        if status['status'] != 'pending':break
        time.sleep(.1)
    assert status['status'] == 'completed', status
    state = call('ui_state')
    assert state['dialogs']['project_dashboard'],state
    assert state['indexing'] or 'symbols ready' in state['status'],state
    for _ in range(300):
        state = call('ui_state')
        if not state['indexing']:break
        time.sleep(.1)
    assert not state['indexing'] and 'symbols ready' in state['status'],state
    operation = call('pcb_3d')
    for _ in range(1000):
        status = call('operation_status',{'operation_id':operation['operation_id']})
        if status['status'] != 'pending':break
        time.sleep(.1)
    assert status['status'] == 'completed', status
    for name,action in [('pcb-3d','show_3d'),('project','show_project_dashboard'),
                        ('footprints','show_footprint_browser'),('symbols','show_symbol_browser')]:
        call('ui_action',{'action':'dismiss_dialogs'})
        call('ui_action',{'action':action})
        call('screenshot',{'path':str(qa/(name+'-final.png')),'interface':True})
    print('UI_QA',qa,flush=True)
finally:
    client.stdin.close();client.wait(30)
    subprocess.run(['powershell','-NoProfile','-Command',f'Stop-Process -Id {pid} -ErrorAction SilentlyContinue'],env=env)
