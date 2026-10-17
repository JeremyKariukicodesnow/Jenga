import {inject} from './assets/vendor/vercel-analytics.mjs';

// GitHub Pages serves the same static site. Send those visits to the existing
// Jenga Vercel project rather than requesting nonexistent GitHub endpoints.
const github=location.hostname==='jeremykariukicodesnow.github.io';
const local=['localhost','127.0.0.1',''].includes(location.hostname);
if(!local)inject({
 mode:'production',
 ...(github?{scriptSrc:'https://jenga.sparkfield.co.ke/_vercel/insights/script.js',endpoint:'https://jenga.sparkfield.co.ke/_vercel/insights'}:{}),
 beforeSend(event){
  // Keep page paths useful without collecting arbitrary query parameters.
  const url=new URL(event.url);url.search='';url.hash='';
  return {...event,url:url.href};
 }
});
