const reduced = matchMedia('(prefers-reduced-motion: reduce)');
function motion() {
  document.documentElement.dataset.motion = reduced.matches ? 'off' : 'on';
  if(reduced.matches)for(const model of document.querySelectorAll('model-viewer'))model.removeAttribute('auto-rotate');
}
reduced.addEventListener('change', motion);
motion();
const backdrop = document.createElement('div'); backdrop.className = 'orange-rain'; backdrop.setAttribute('aria-hidden','true');
for (let i=0;i<28;i++) {const column=document.createElement('i');column.style.setProperty('--column',i);column.style.setProperty('--delay',`${-(i*1.73)%17}s`);column.style.setProperty('--speed',`${13+i%7}s`);backdrop.append(column);}
document.body.prepend(backdrop);
const header=document.querySelector('header');
if(header){let queued=false;const syncHeader=()=>{header.classList.toggle('is-scrolled',window.scrollY>24);queued=false;};syncHeader();window.addEventListener('scroll',()=>{if(!queued){queued=true;requestAnimationFrame(syncHeader);}},{passive:true});}
if(!reduced.matches&&'IntersectionObserver'in window){
 const entrance=new IntersectionObserver(entries=>{for(const entry of entries)if(entry.isIntersecting){entry.target.classList.add('reveal-in');entrance.unobserve(entry.target);}},{threshold:0.06});
 const watch=()=>document.querySelectorAll('.story,.feature-grid article,.project-activity').forEach(node=>entrance.observe(node));
 watch();new MutationObserver(watch).observe(document.querySelector('main'),{childList:true,subtree:true});
}
