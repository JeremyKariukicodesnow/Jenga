"""Isolated install/upgrade/repair/downgrade checks; never touches the real installation."""
from pathlib import Path
import subprocess,os,json,time,shutil,tomllib
ROOT=Path(__file__).resolve().parent.parent
QA=ROOT/'.cache/installer-0.8.0'/str(int(time.time()));QA.mkdir(parents=True)
env=os.environ.copy();env.update(USERPROFILE=str(QA/'home'),LOCALAPPDATA=str(QA/'local'),JENGA_DATA_DIR=str(QA/'data'),JENGA_CONFIG_DIR=str(QA/'settings'),JENGA_SESSION_DIR=str(QA/'sessions'))
home=QA/'home';(home/'.codex').mkdir(parents=True)
config=home/'.codex/config.toml';config.write_text('[mcp_servers.jenga]\ncommand="C:/old/jenga-cli.exe"\nargs=["mcp","--workspace","C:/preserved","--connect","auto"]\n\n[mcp_servers.unrelated]\ncommand="leave-me"\n',encoding='utf8')
(QA/'data').mkdir();(QA/'data/personal-library-marker').write_text('preserve')
installed=QA/'installed';versions=['0.7.1','0.8.0'];report={};identity='Jenga-QA-080-'+QA.name
for version in versions:
 payload=QA/('payload-'+version);payload.mkdir()
 binaries=ROOT/('target/release' if version=='0.8.0' else 'dist/'+version+'/Jenga')
 for name in ['Jenga.exe','jenga-cli.exe']+(['jenga-connect.exe'] if version=='0.8.0' else []):shutil.copy2(binaries/name,payload/name)
 shutil.copy2(ROOT/'LICENSE',payload/'LICENSE')
 args=[str(ROOT/'tools/inno/ISCC.exe'),'/Q','/DTestIdentity',f'/DTestIdentityName={identity}',f'/DAppVersion={version}',f'/DPayloadDir={payload}',f'/DOutputDir={QA}',str(ROOT/'packaging/windows/jenga.iss')]
 r=subprocess.run(args,capture_output=True,text=True);assert r.returncode==0,(r.stdout,r.stderr)
bridge_command=None
for action,version in [('fresh','0.7.1'),('upgrade','0.8.0'),('repair','0.8.0'),('downgrade','0.7.1')]:
 log=QA/(action+'.log');exe=QA/f'Jenga-{version}-windows-x64-Setup.exe'
 r=subprocess.run([str(exe),'/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/NOCLOSEAPPLICATIONS',f'/DIR={installed}',f'/LOG={log}','/ALLOWDOWNGRADE=1'],env=env,creationflags=subprocess.CREATE_NO_WINDOW,timeout=180)
 assert r.returncode==0,(action,r.returncode)
 text=log.read_text(encoding='utf-8-sig');assert 'Installation process succeeded' in text
 r=subprocess.run([str(installed/'jenga-cli.exe'),'--version'],env=env,capture_output=True,text=True);assert r.returncode==0 and version in r.stdout,(action,r.stdout,r.stderr)
 cfg=tomllib.loads(config.read_text(encoding='utf8'));entry=cfg['mcp_servers']['jenga'];assert entry['args']==['mcp','--workspace','C:/preserved','--connect','auto'];assert cfg['mcp_servers']['unrelated']['command']=='leave-me';assert 'C:/old' not in entry['command']
 if action=='upgrade':bridge_command=entry['command'];assert bridge_command.endswith('jenga-connect.exe');assert Path(bridge_command).is_file()
 if action in ['repair','downgrade']:assert entry['command']==bridge_command,(action,entry)
 assert (QA/'data/personal-library-marker').read_text()=='preserve'
 report[action]={'version':version,'settings_preserved':True,'command':entry['command']};print('INSTALLER',action,'passed',flush=True)
(QA/'report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
uninstaller=installed/'unins000.exe'
assert uninstaller.resolve().is_relative_to(QA.resolve())
r=subprocess.run([str(uninstaller),'/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',f'/LOG={QA / "uninstall.log"}'],env=env,creationflags=subprocess.CREATE_NO_WINDOW,timeout=180)
assert r.returncode==0,('QA uninstall',r.returncode)
assert (QA/'data/personal-library-marker').read_text()=='preserve'
print('INSTALLER_QA',QA,flush=True)
