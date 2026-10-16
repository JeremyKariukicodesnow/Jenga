const reduced = matchMedia('(prefers-reduced-motion: reduce)');
function motion() {
  document.documentElement.dataset.motion = reduced.matches ? 'off' : 'on';
  for (const model of document.querySelectorAll('model-viewer')) model.toggleAttribute('auto-rotate', !reduced.matches);
}
reduced.addEventListener('change', motion);
motion();
const backdrop = document.createElement('div'); backdrop.className = 'orange-rain'; backdrop.setAttribute('aria-hidden','true');
for (let i=0;i<28;i++) {const column=document.createElement('i');column.style.setProperty('--column',i);column.style.setProperty('--delay',`${-(i*1.73)%17}s`);column.style.setProperty('--speed',`${13+i%7}s`);backdrop.append(column);}
document.body.prepend(backdrop);
