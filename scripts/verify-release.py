"""Check archive integrity and packaged libraries/backend without installed KiCad."""
from pathlib import Path
import hashlib, json, os, subprocess, time, zipfile

root = Path(__file__).resolve().parent.parent
release = root/'dist/0.7.1'
payload = release/'Jenga'
qa = root/'.cache'/('release-0.7.1-'+str(int(time.time())))
qa.mkdir(parents=True)
def digest(path):
    with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()
for line in (release/'SHA256SUMS').read_text(encoding='utf-8-sig').splitlines():
    expected,name = line.split('  ',1)
    assert digest(release/name)==expected,name
print('release checksums passed',flush=True)
manifest = json.loads((payload/'libraries/manifest.json').read_text(encoding='utf-8-sig'))
assert manifest['models_included']
with zipfile.ZipFile(release/'Jenga-0.7.1-windows-x64-portable.zip') as archive:
    assert archive.testzip() is None
    names = archive.namelist()
    for source in manifest['sources']:
        count = len([n for n in names if n.startswith('Jenga/libraries/'+source['kind']+'/') and not n.endswith('/')])
        assert count == source['count'],(source,count)
    for name in ['Jenga.exe','jenga-cli.exe']:
        assert digest(root/'target/release'/name)==hashlib.sha256(archive.read('Jenga/'+name)).hexdigest()
with zipfile.ZipFile(release/'Jenga-0.7.1-source.zip') as archive:
    assert archive.testzip() is None
print('complete archive contents and CRCs passed',flush=True)
env = os.environ.copy()
env.update(JENGA_DATA_DIR=str(qa/'data'),JENGA_CONFIG_DIR=str(qa/'settings'),
    JENGA_SESSION_DIR=str(qa/'sessions'),APPDATA=str(qa/'appdata'),
    USERPROFILE=str(qa/'home'),ProgramFiles=str(qa/'no-installed-kicad'))
for version in ['10','9','8','7','6']:env['KICAD'+version+'_3DMODEL_DIR']=str(qa/'obsolete-model-path')
client = subprocess.Popen([str(payload/'jenga-cli.exe'),'json','--workspace',str(qa),'--headless'],
    stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf8',env=env)
def call(method,params):
    client.stdin.write(json.dumps({'method':method,'params':params})+'\n');client.stdin.flush()
    value = json.loads(client.stdout.readline());assert 'error' not in value,(method,value)
    return value['result']
try:
    symbol = call('get_symbol',{'id':'Device:R'})
    footprint = call('get_footprint',{'id':'Resistor_SMD:R_0805_2012Metric'})
    mesh = call('footprint_3d',{'id':'Resistor_SMD:R_0805_2012Metric','project_folder':str(qa)})
    assert mesh['mesh']['triangle_count']>100,mesh
    assert 'Could not add 3D model' not in mesh['backend']['log'],mesh
    assert Path(mesh['backend']['backend']).resolve().is_relative_to(payload.resolve())
    assert mesh['backend']['models'],mesh
    for finding in mesh['backend']['models']:
        assert finding['status']=='available',finding
        assert Path(finding['resolved_path']).resolve().is_relative_to((payload/'libraries/3dmodels').resolve()),finding
    report = {'checksums':True,'archive_crc':True,'complete_libraries':True,
        'matching_binaries':True,'packaged_symbols':True,'packaged_footprints':True,
        'packaged_models_and_backend_without_kicad':True,'models':manifest['sources'][-1]['count']}
    (qa/'report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
    print('RELEASE_QA',qa,report,flush=True)
finally:
    client.stdin.close();client.wait(30)
