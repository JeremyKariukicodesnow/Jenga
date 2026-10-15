"""Exercise the packaged libraries/backend with installed KiCad discovery disabled."""
from pathlib import Path
import json, os, subprocess, time

ROOT = Path(__file__).resolve().parent.parent
PAYLOAD = ROOT / 'dist/0.8.0/Jenga'
QA = ROOT / '.cache' / ('offline-080-' + str(int(time.time())))
QA.mkdir(parents=True)
isolated_keys = {'APPDATA', 'LOCALAPPDATA', 'USERPROFILE', 'PROGRAMFILES',
                 'PROGRAMFILES(X86)', 'PROGRAMW6432', 'PATH', 'JENGA_KICAD_CLI'}
env = {k: v for k, v in os.environ.items()
       if not k.upper().startswith('KICAD') and k.upper() not in isolated_keys}
env.update(APPDATA=str(QA / 'roaming'), LOCALAPPDATA=str(QA / 'local'),
           USERPROFILE=str(QA / 'home'), ProgramFiles=str(QA / 'no-installed-programs'),
           JENGA_DATA_DIR=str(QA / 'data'), JENGA_CONFIG_DIR=str(QA / 'settings'),
           JENGA_SESSION_DIR=str(QA / 'sessions'),
           PATH=str(Path(os.environ['SystemRoot']) / 'System32'))
requests = [
    ('list_libraries', {}),
    ('get_symbol', {'id': 'Device:R'}),
    ('get_footprint', {'id': 'Resistor_SMD:R_0805_2012Metric'}),
    ('model_inspect', {'path': str(PAYLOAD / 'libraries/3dmodels/Resistor_SMD.3dshapes/R_0805_2012Metric.step')}),
]
result = subprocess.run(
    [str(PAYLOAD / 'jenga-cli.exe'), 'json', '--headless', '--workspace', str(QA)],
    input=''.join(json.dumps({'method': m, 'params': p}) + '\n' for m, p in requests),
    text=True, encoding='utf8', capture_output=True, env=env, timeout=120,
    creationflags=subprocess.CREATE_NO_WINDOW)
assert result.returncode == 0, result.stderr
responses = [json.loads(line) for line in result.stdout.splitlines()]
assert len(responses) == len(requests), result.stdout
for (method, _), response in zip(requests, responses):
    assert 'error' not in response, (method, response)
sources = responses[0]['result']['sources']
assert {s['kind'] for s in sources} == {'symbols', 'footprints', '3dmodels'}, sources
assert all(s['name'] == 'Jenga bundled' and
           Path(s['path']).is_relative_to(PAYLOAD) for s in sources), sources
assert responses[1]['result']['physical_pin_count'] == 2
assert responses[2]['result']['models_3d']
report = {'installed_kicad_discovery_disabled': True, 'sources': sources,
          'symbol': 'Device:R', 'footprint': 'Resistor_SMD:R_0805_2012Metric',
          'model': responses[3]['result']}
(QA / 'report.json').write_text(json.dumps(report, indent=2), encoding='utf8')
print('OFFLINE_QA passed:', QA)
