"""Fresh install, upgrade, repair and downgrade with a separate Inno identity."""
from pathlib import Path
import subprocess, os, json, time, shutil
ROOT=Path(__file__).resolve().parent.parent
QA=ROOT/'.cache/installer-0.7.1'/str(int(time.time()));QA.mkdir(parents=True)
env=os.environ.copy();env.update(USERPROFILE=str(QA/'home'),JENGA_DATA_DIR=str(QA/'data'),JENGA_CONFIG_DIR=str(QA/'settings'),JENGA_SESSION_DIR=str(QA/'sessions'))
home=QA/'home';home.mkdir();(home/'.codex').mkdir();(home/'.codex/config.toml').write_text('[mcp_servers.jenga]\ncommand="C:/old/jenga-cli.exe"\nargs=["mcp","--workspace","C:/preserved"]\n\n[mcp_servers.unrelated]\ncommand="leave-me"\n',encoding='utf8')
installed=QA/'installed';versions=['0.6.2','0.7.1'];report={}
for version in versions:
 payload=QA/('payload-'+version);payload.mkdir()
 binaries=ROOT/('target/release' if version=='0.7.1' else 'dist/'+version+'/Jenga')
 for name in ['Jenga.exe','jenga-cli.exe']:shutil.copy2(binaries/name,payload/name)
 shutil.copy2(ROOT/'LICENSE',payload/'LICENSE')
 args=[str(ROOT/'tools/inno/ISCC.exe'),'/Q','/DTestIdentity',f'/DTestIdentityName=Jenga-QA-070-{QA.name}',f'/DAppVersion={version}',f'/DPayloadDir={payload}',f'/DOutputDir={QA}',str(ROOT/'packaging/windows/jenga.iss')]
 r=subprocess.run(args,capture_output=True,text=True);assert r.returncode==0,(r.stdout,r.stderr)
for index,(action,version) in enumerate([('fresh','0.6.2'),('upgrade','0.7.1'),('repair','0.7.1'),('downgrade','0.6.2')]):
 log=QA/(action+'.log');exe=QA/f'Jenga-{version}-windows-x64-Setup.exe'
 r=subprocess.run([str(exe),'/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/NOCLOSEAPPLICATIONS',f'/DIR={installed}',f'/LOG={log}','/ALLOWDOWNGRADE=1'],env=env,creationflags=subprocess.CREATE_NO_WINDOW,timeout=120)
 assert r.returncode==0,(action,r.returncode)
 text=log.read_text(encoding='utf-8-sig');assert 'Installation process succeeded' in text
 assert 'Process exit code: 0' in text,(action,text[-2000:])
 r=subprocess.run([str(installed/'jenga-cli.exe'),'--version'],env=env,capture_output=True,text=True);assert r.returncode==0 and version in r.stdout,(action,r.stdout,r.stderr)
 config=(home/'.codex/config.toml').read_text(encoding='utf8');assert 'leave-me' in config and 'C:/preserved' in config;assert 'C:/old/jenga-cli.exe' not in config,(action,config)
 report[action]=True;print('installer',action,'passed',flush=True)
(QA/'report.json').write_text(json.dumps(report,indent=2),encoding='utf8');print('INSTALLER_QA',QA,flush=True)
