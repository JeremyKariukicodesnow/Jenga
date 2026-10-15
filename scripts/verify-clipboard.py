"""Isolated public-tool checks; never opens or edits personal projects."""
import subprocess,json,os,time
from pathlib import Path
root=Path(__file__).resolve().parent.parent;qa=root/'.cache/qa-0.5.1';qa.mkdir(parents=True,exist_ok=True)
env=os.environ.copy();env['JENGA_SESSION_DIR']=str(qa/'clipboard-sessions');env['JENGA_DATA_DIR']=str(qa/'clipboard-data')
p=subprocess.Popen([str(root/'target/release/jenga-cli.exe'),'json','--headless','--workspace',str(qa)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding='utf8',env=env);i=0
report={}
def call(m,params={}):
 global i;i+=1;p.stdin.write(json.dumps({'id':i,'method':m,'params':params})+'\n');p.stdin.flush();r=json.loads(p.stdout.readline());assert 'error' not in r,r;return r['result']
try:
 call('create_project',{'name':'ClipboardQA'+str(time.time_ns()),'folder':str(qa/'clipboard')})
 d=call('edit',{'actions':[{'op':'add_symbol','library_id':'Device:R','at':[101.6,71.12],'rotation':90},{'op':'add_symbol','library_id':'Device:C','at':[125.73,71.12],'rotation':90}]})
 d=call('edit',{'expected_revision':d['revision'],'actions':[{'op':'connect_pins','from':{'reference':'R1','number':'2'},'to':{'reference':'C1','number':'1'}}]})
 route=d['results'][0]['route_mm'];assert len(route)==2,route;report['straight_route_no_midpoints']=True
 assert not any('Off connection grid' in f['message'] for f in call('erc'))
 before=call('inspect')['revision'];copied=call('copy_objects');assert copied['text'].startswith('(lib_symbols')
 data=call('paste_objects',{'text':copied['text'],'at':[152.4,101.6],'expected_revision':before,'actor':'ai:regression'});assert len(data['uuids'])==3,data
 d=call('inspect');refs=[o['reference'] for o in d['objects'] if o['kind']=='symbol'];assert sorted(refs)==['C1','C2','R1','R2'],refs;report['clipboard_fresh_references']=True
 call('undo');assert call('inspect')['revision']==before;call('redo');assert len([o for o in call('inspect')['objects'] if o['kind']=='symbol'])==4;call('undo');report['clipboard_undo_redo']=True
 skill=call('view_skill');assert 'copy_objects' in skill['text'];skillpath=qa/'skill'/str(time.time_ns())/'SKILL.md';call('export_skill',{'path':str(skillpath)});assert skillpath.read_bytes().decode('utf8')==skill['text'];report['skill_view_export']=True
 call('save');file=call('inspect')['file']
 # Paste a real serialized block into a second empty native document, without installed-library lookup.
 call('create_schematic',{'path':str(qa/f'clipboard-import-{time.time_ns()}.kicad_sch')})
 call('paste_objects',{'text':copied['text'],'at':[50.8,50.8]});call('save');file=call('inspect')['file'];assert not call('inspect')['missing_symbols'];report['cross_document_embedded_definitions']=True
 cli=Path(r'C:\Program Files\KiCad\10.0\bin\kicad-cli.exe');r=subprocess.run([str(cli),'sch','export','netlist',file,'-o',str(qa/'clipboard.net')],capture_output=True,text=True);assert r.returncode==0,(r.stdout,r.stderr);report['KiCad_reads_clipboard_saved_native']=True
 (qa/'clipboard-report.json').write_text(json.dumps(report,indent=2),encoding='utf8');print(json.dumps(report,indent=2))
finally:p.stdin.close();p.wait()
