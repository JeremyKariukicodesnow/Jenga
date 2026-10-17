const repository='JeremyKariukicodesnow/Jenga';
const root=`https://github.com/${repository}`;
const section=document.createElement('section');
section.className='project-activity';
section.setAttribute('aria-label','Jenga on GitHub');
section.innerHTML=`<div><p class="eyebrow">BUILDING IN THE OPEN</p><h2>Follow Jenga’s progress.</h2></div><div class="activity-grid"><a href="${root}/stargazers"><strong data-count="stars">—</strong><span>GitHub stars ↗</span></a><a href="${root}/releases"><strong data-count="downloads">—</strong><span>Release downloads ↗</span></a><a href="${root}/forks"><strong data-count="forks">—</strong><span>Community forks ↗</span></a><a href="${root}/releases"><strong data-count="version">—</strong><span>Latest published release ↗</span></a></div><p class="activity-note" role="status">Loading public GitHub activity…</p><p class="details">Downloads count Windows installer and portable ZIP assets across published releases, including repeat downloads. They are not unique users or installations.</p>`;
const anchor=document.querySelector('.capabilities')||document.querySelector('section.download');
anchor?.after(section);
const count=new Intl.NumberFormat('en');
function display(data,cached=false){
 for(const name of ['stars','downloads','forks'])section.querySelector(`[data-count="${name}"]`).textContent=count.format(data[name]);
 section.querySelector('[data-count="version"]').textContent=data.version;
 section.querySelector('.activity-note').textContent=`${cached?'Recent snapshot':'From GitHub'} · Updated ${new Date(data.at).toLocaleString(undefined,{dateStyle:'medium',timeStyle:'short'})}`;
}
const key='jenga-public-activity-v1';
let cached;
try{cached=JSON.parse(localStorage.getItem(key));if(cached&&['stars','downloads','forks','at'].every(k=>Number.isFinite(cached[k]))&&typeof cached.version==='string')display(cached,true);else cached=null;}catch{/* Storage is optional. */}
async function json(url,signal){const response=await fetch(url,{signal,credentials:'omit',headers:{Accept:'application/vnd.github+json'}});if(!response.ok)throw Error('GitHub unavailable');return{data:await response.json(),next:/rel="next"/.test(response.headers.get('link')||'')};}
if(!cached||Date.now()-cached.at>15*60*1000){
 const controller=new AbortController(),timer=setTimeout(()=>controller.abort(),12000);
 try{
  const repo=await json(`https://api.github.com/repos/${repository}`,controller.signal);
  const releases=[];
  for(let page=1;page<=10;page++){
   const result=await json(`https://api.github.com/repos/${repository}/releases?per_page=100&page=${page}`,controller.signal);
   if(!Array.isArray(result.data))throw Error('Invalid release response');
   releases.push(...result.data.filter(r=>!r.draft&&r.published_at));
   if(!result.next)break;
   if(page===10)throw Error('Incomplete release history');
  }
  // Each asset counter records actual requests, including canonical and versioned links.
  let downloads=0;
  for(const release of releases)for(const suffix of ['windows-x64-Setup.exe','windows-x64-portable.zip']){
   for(const asset of release.assets||[])if([`Jenga-${suffix}`,`Jenga-${release.tag_name.replace(/^v/,'')}-${suffix}`].includes(asset.name)&&asset.state==='uploaded'&&Number.isSafeInteger(asset.download_count))downloads+=asset.download_count;
  }
  const {select}=await import('./download/releases.mjs');
  const selected=select(releases);
  const data={stars:repo.data.stargazers_count,forks:repo.data.forks_count,downloads,version:selected.release?.tag_name||'See releases',at:Date.now()};
  if(![data.stars,data.forks,data.downloads].every(Number.isSafeInteger))throw Error('Invalid GitHub counts');
  display(data);
  try{localStorage.setItem(key,JSON.stringify(data));}catch{/* Private browsing remains supported. */}
 }catch{section.querySelector('.activity-note').textContent=cached?'Showing a saved snapshot. Open GitHub for current activity.':'GitHub activity is temporarily unavailable. The links above still work.';}
 finally{clearTimeout(timer);}
}
