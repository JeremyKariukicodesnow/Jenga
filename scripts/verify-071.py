"""0.7.1 regression: saved projects, native net preservation and generated models."""
from pathlib import Path
import json,os,shutil,subprocess,time
root=Path(__file__).resolve().parent.parent
qa=root/'.cache'/('reliability-071-'+str(int(time.time())))
qa.mkdir(parents=True)
env=os.environ.copy()
env.update(JENGA_DATA_DIR=str(qa/'data'),JENGA_CONFIG_DIR=str(qa/'config'),JENGA_SESSION_DIR=str(qa/'sessions'))
cli=root/'target/release/jenga-cli.exe'
child=subprocess.Popen([str(cli),'json','--workspace',str(qa),'--headless'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf8',env=env)
def call(method,params=None):
    child.stdin.write(json.dumps({'method':method,'params':params or {}})+'\n');child.stdin.flush()
    v=json.loads(child.stdout.readline());assert 'error' not in v,(method,v);return v['result']
def padmap(v):return {(o['reference'],p['number'],p['uuid']):p['net_name'] for o in v['objects'] for p in o.get('pads',[])}
report={}
try:
    source=Path.home()/'Documents/555 Pulse Generator'
    dest=qa/'555 Pulse Generator'
    shutil.copytree(source,dest,ignore=shutil.ignore_patterns('*.zip','assembly.*','manufacturing','*.png','*.pdf'))
    for p in (dest/'.jenga').rglob('*.json'):
        text=p.read_text(encoding='utf8');text=text.replace(str(source).replace('\\','\\\\'),str(dest).replace('\\','\\\\'));p.write_text(text,encoding='utf8')
    sch=dest/'555 Pulse Generator.kicad_sch';board=sch.with_suffix('.kicad_pcb')
    call('open',{'path':str(sch)})
    assert call('pcb_preflight')['passed']
    call('pcb_open',{'path':str(board)})
    before=call('pcb_inspect')
    call('save',{'target':'schematic'})
    assert call('verify',{'target':'schematic'})['passed']
    call('pcb_fill_zones')
    after=call('pcb_inspect');assert padmap(before)==padmap(after)
    native=call('pcb_verify',{'external':True});assert native['passed'],native
    assert not native['kicad_drc']['violations'],native['kicad_drc']
    assert all(m['status']=='available' or m['status']=='not_required' for m in native['models'])
    report['555_native_drc_and_net_preservation']=True
    call('pcb_save')
    update=call('pcb_update_from_schematic')
    assert update['can_commit'],update
    old_tracks=[o for o in after['objects'] if o['kind']=='segment']
    call('pcb_commit',{'preview_id':update['preview_id']})
    assert old_tracks==[o for o in call('pcb_inspect')['objects'] if o['kind']=='segment']
    report['guarded_update_preserves_tracks']=True
    call('pcb_save')
    model=call('model_create',{'name':'MeasuredBody','project_folder':str(dest),'shapes':[{'kind':'box','size_mm':[6,10,3],'at_mm':[0,0,1.5]}]})
    mesh=call('model_inspect',{'path':model['path']})
    lo,hi=mesh['bounds_mm'];sizes=[hi[i]-lo[i] for i in range(3)]
    assert all(abs(a-b)<0.03 for a,b in zip(sizes,[6,10,3])),mesh
    report['generated_vrml_real_mm_dimensions']=sizes
    from PIL import Image
    views=['isometric','top','bottom','front','back','left','right']
    exports=qa/'view-exports'
    exports.mkdir()
    pdf=call('pcb_export_3d_view',{'path':str(exports/'assembly-views.pdf'),'format':'pdf','views':views,'width':800,'height':600})
    assert pdf['page_count']==7
    png=call('pcb_export_3d_view',{'path':str(exports/'assembly-views'),'format':'png','views':views,'width':800,'height':600})
    assert len(png['files'])==7
    for file in png['files']:
        image=Image.open(file);image.load();assert image.size==(800,600) and len(image.getcolors(800*600))>8
    call('pcb_export_3d_view',{'path':str(exports/'current.png'),'format':'png','view':'current','yaw':1.2,'pitch':0.8,'zoom':1.1,'width':800,'height':600})
    call('pcb_create_page',{'name':'Accessory','width_mm':30,'height_mm':25})
    assert len(call('project_info')['boards'])==2
    pages=call('pcb_export_pages',{'path':str(exports/'board-pages.pdf'),'format':'pdf','width':800})
    assert pages['page_count']==2
    for format in ['png','svg','native']:
        output=call('pcb_export_pages',{'path':str(exports/('board-pages-'+format)),'format':format,'width':800})
        assert all(Path(f).exists() for f in output['files'])
    report['seven_3d_views_png_pdf_and_two_pcb_pages']=True
    call('pcb_open',{'path':str(board)})
    copy=qa/'save-as'/'Copied.kicad_pcb'
    call('pcb_save',{'path':str(copy)})
    saved=call('pcb_verify')
    assert all(m['status'] in ['available','not_required'] for m in saved['models'])
    report['save_as_preserves_models']=True
    call('pcb_close')
    maize=Path.home()/'Documents/Maize Factory/Maize Factory.kicad_sch'
    call('open',{'path':str(maize)})
    audit=call('pcb_preflight');assert audit['findings']
    assert all(f['reference'] and f['message'] for f in audit['findings'])
    report['maize_footprint_findings']=audit['finding_count']
    report['maize_sample']=audit['findings'][:3]
    only=call('pcb_create_project',{'name':'Independent071','folder':str(qa)})
    assert call('project_info')['pages']==[]
    report['pcb_only_project']=True
    (qa/'report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
    print('QA_PATH',qa,flush=True);print(json.dumps(report,indent=2),flush=True)
finally:
    child.stdin.close();child.wait(30)
