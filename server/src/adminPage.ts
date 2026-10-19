/** The admin page: one HTML file, no libraries. The token is typed in and kept only in this tab (sessionStorage); everything from the server goes in through textContent.
 *  (No backticks and no dollar-brace inside: this is a template literal. Texts are put together with +.) */
export const ADMIN_PAGE = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex"><meta name="color-scheme" content="dark"><title>Aside licences</title>
<link rel="icon" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'%3E%3Crect width='64' height='64' rx='14' fill='%23a3191f'/%3E%3Cpath d='M32 12 14 52h9l3.4-8.6h11.2L41 52h9zM29 36l3-8 3 8z' fill='%23fff'/%3E%3C/svg%3E">
<style>
:root{
  --bg:#0f0607;--bg2:#150a0b;--panel:#1c0d0f;--panel2:#241114;--line:#3b1c20;--line2:#52272c;
  --fg:#f4e9e9;--mute:#a98d90;--faint:#7a6063;
  --accent:#b3202a;--accent2:#d12d38;--accentbg:#3a1216;--glow:rgba(209,45,56,.28);
  --ok:#5fc08a;--okbg:#12281c;--warn:#e3a94b;--warnbg:#2f2410;--bad:#ff7b72;--badbg:#3b1516;
  --radius:12px;--shadow:0 10px 40px rgba(0,0,0,.55)}
*{box-sizing:border-box}
html{background:var(--bg)}
body{margin:0;font:14px/1.5 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;color:var(--fg);background:radial-gradient(1200px 500px at 85% -10%,rgba(179,32,42,.22),transparent 60%),var(--bg);min-height:100vh}
::selection{background:var(--accent);color:#fff}
::-webkit-scrollbar{width:10px;height:10px}::-webkit-scrollbar-thumb{background:var(--line2);border-radius:8px}::-webkit-scrollbar-track{background:transparent}
a{color:var(--accent2);text-decoration:none}a:hover{text-decoration:underline}
.mono{font-family:ui-monospace,"Cascadia Mono",Consolas,monospace;font-size:13px}.mute{color:var(--mute)}.faint{color:var(--faint)}.small{font-size:12px}
[hidden]{display:none!important}

/* controls */
input,select,textarea,button{font:inherit;color:inherit}
input,select,textarea{background:var(--bg2);border:1px solid var(--line);border-radius:9px;padding:8px 12px;width:100%}
input::placeholder,textarea::placeholder{color:var(--faint)}
input:focus,select:focus,textarea:focus{outline:none;border-color:var(--accent2);box-shadow:0 0 0 3px var(--glow)}
button{background:var(--panel2);border:1px solid var(--line2);border-radius:9px;padding:7px 14px;cursor:pointer;transition:background .12s,border-color .12s,transform .05s;white-space:nowrap}
button:hover{background:#2e1519;border-color:#6b3238}button:active{transform:translateY(1px)}
button:focus-visible{outline:2px solid var(--accent2);outline-offset:2px}
button.primary{background:var(--accent);border-color:var(--accent2);color:#fff;font-weight:600}button.primary:hover{background:var(--accent2)}
button.danger{color:var(--bad);border-color:#5a2226}button.danger:hover{background:var(--badbg);border-color:var(--bad)}
button.ghost{background:transparent;border-color:transparent;color:var(--mute)}button.ghost:hover{background:var(--panel2);color:var(--fg)}
button:disabled{opacity:.45;cursor:default}

/* top bar */
.top{position:sticky;top:0;z-index:5;display:flex;align-items:center;gap:10px;padding:12px 28px;background:rgba(15,6,7,.82);backdrop-filter:blur(10px);border-bottom:1px solid var(--line)}
.brand{display:flex;align-items:center;gap:11px;margin-right:auto}
.brand svg{width:32px;height:32px;filter:drop-shadow(0 2px 8px var(--glow))}
.brand b{font-size:16px;letter-spacing:.01em}.brand span{color:var(--mute);margin-left:6px;font-weight:400}

/* login */
.login{max-width:400px;margin:14vh auto 0;padding:28px;background:linear-gradient(180deg,var(--panel2),var(--panel));border:1px solid var(--line2);border-radius:16px;box-shadow:var(--shadow)}
.login h2{margin:0 0 4px;font-size:20px}.login p{margin:0 0 18px;color:var(--mute)}.login .row{display:flex;gap:8px}
#loginmsg{color:var(--bad);margin-top:10px;min-height:20px}

/* page */
main{max-width:1320px;margin:0 auto;padding:24px 28px 80px}
.stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px;margin-bottom:20px}
.stat{position:relative;padding:14px 16px 13px;background:linear-gradient(180deg,var(--panel2),var(--panel));border:1px solid var(--line);border-radius:var(--radius);overflow:hidden}
.stat:before{content:"";position:absolute;left:0;top:0;bottom:0;width:3px;background:var(--line2)}
.stat.good:before{background:var(--ok)}.stat.warn:before{background:var(--warn)}.stat.bad:before{background:var(--accent2)}.stat.acc:before{background:var(--accent2)}
.stat span{display:block;color:var(--mute);font-size:12px;letter-spacing:.05em;text-transform:uppercase}
.stat b{display:block;font-size:30px;font-weight:650;line-height:1.25;font-variant-numeric:tabular-nums}
.stat.warn b{color:var(--warn)}.stat.bad b{color:var(--bad)}
.stat small{color:var(--faint)}
.grid{display:grid;grid-template-columns:minmax(0,1fr) 360px;gap:16px;align-items:start}
@media (max-width:1050px){.grid{grid-template-columns:1fr}}
.panel{background:linear-gradient(180deg,var(--panel2),var(--panel));border:1px solid var(--line);border-radius:var(--radius);overflow:hidden}
.panel-head{display:flex;align-items:center;gap:10px;flex-wrap:wrap;padding:12px 14px;border-bottom:1px solid var(--line)}
.panel-head h2{margin:0;font-size:15px;font-weight:600;margin-right:auto}
.panel-head input[type=search]{width:260px;max-width:100%}
.chips{display:flex;gap:4px;padding:3px;background:var(--bg2);border:1px solid var(--line);border-radius:999px}
.chip{padding:3px 12px;border-radius:999px;border:0;background:transparent;color:var(--mute);font-size:13px}
.chip:hover{background:var(--panel2);color:var(--fg)}.chip.on{background:var(--accent);color:#fff}
table{width:100%;border-collapse:collapse}
th{text-align:left;font-weight:500;color:var(--mute);font-size:11.5px;text-transform:uppercase;letter-spacing:.06em;padding:9px 14px;border-bottom:1px solid var(--line);white-space:nowrap}
td{padding:11px 14px;border-bottom:1px solid var(--line);vertical-align:middle}
tbody tr{cursor:pointer;transition:background .1s}tbody tr:hover{background:var(--accentbg)}tbody tr:last-child td{border-bottom:0}
tr.cancelled td{opacity:.62}
.empty{padding:36px;text-align:center;color:var(--mute)}
.pill{display:inline-block;white-space:nowrap;padding:2px 10px;border-radius:999px;font-size:12px;font-weight:500;border:1px solid var(--line2);color:var(--mute)}
.pill.ok{background:var(--okbg);color:var(--ok);border-color:#1e4a31}.pill.warn{background:var(--warnbg);color:var(--warn);border-color:#58421a}.pill.bad{background:var(--badbg);color:var(--bad);border-color:#6a2428}.pill.acc{background:var(--accentbg);color:#ff9aa0;border-color:#6a2428}
.badges{display:flex;gap:5px;margin-top:4px;flex-wrap:wrap}.badge{white-space:nowrap;font-size:11px;padding:1px 7px;border-radius:6px;background:var(--accentbg);color:#ff9aa0;border:1px solid #5a2226}
.dots{display:inline-flex;gap:3px;vertical-align:middle;margin-right:7px}.dots i{width:9px;height:9px;border-radius:50%;border:1.5px solid var(--faint)}.dots i.on{background:var(--accent2);border-color:var(--accent2);box-shadow:0 0 6px var(--glow)}

/* activity */
.events{list-style:none;margin:0;padding:4px 0;max-height:640px;overflow:auto}
.events li{display:grid;grid-template-columns:10px 1fr auto;gap:2px 10px;align-items:baseline;padding:8px 14px;border-bottom:1px solid rgba(59,28,32,.55)}
.events li:last-child{border-bottom:0}
.events .dot{width:8px;height:8px;border-radius:50%;background:var(--faint);transform:translateY(-1px)}
.events .dot.ok{background:var(--ok)}.events .dot.bad{background:var(--bad)}.events .dot.warn{background:var(--warn)}.events .dot.acc{background:var(--accent2)}
.events .what{min-width:0}.events .t{color:var(--faint);font-size:12px;white-space:nowrap}
.events .sub{grid-column:2/4;color:var(--mute);font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.events a{cursor:pointer;white-space:nowrap}

/* drawer and dialogs */
.scrim{position:fixed;inset:0;background:rgba(5,1,2,.66);backdrop-filter:blur(2px);z-index:9;animation:fade .14s}
.drawer{position:fixed;top:0;right:0;bottom:0;width:min(500px,100%);overflow:auto;background:var(--panel);border-left:1px solid var(--line2);box-shadow:var(--shadow);z-index:10;padding:22px 24px 40px;animation:slide .18s ease-out}
.drawer-head{display:flex;align-items:center;gap:12px;margin-bottom:6px}.drawer-head h3{margin:0;font-size:18px;flex:1;word-break:break-all}
.sec{margin:26px 0 10px;font-size:12px;font-weight:600;letter-spacing:.07em;text-transform:uppercase;color:var(--mute)}
.kv{display:grid;grid-template-columns:130px 1fr;gap:7px 14px;margin:14px 0 0}.kv dt{color:var(--mute)}.kv dd{margin:0;word-break:break-word}
.tiles{display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin:16px 0}
.tile{padding:10px 12px;border:1px solid var(--line);border-radius:10px;background:var(--bg2)}.tile span{display:block;color:var(--mute);font-size:11.5px;text-transform:uppercase;letter-spacing:.05em}.tile b{font-size:18px}
.tile.warn b{color:var(--warn)}.tile.bad b{color:var(--bad)}
.acts{display:flex;gap:8px;flex-wrap:wrap;margin:14px 0}
.dev{display:flex;align-items:center;gap:10px;padding:10px 0;border-bottom:1px solid var(--line)}.dev:last-child{border-bottom:0}.dev .grow{flex:1;min-width:0}
.notice{padding:10px 13px;border-radius:10px;font-size:13px;margin:12px 0;border:1px solid}.notice.warn{background:var(--warnbg);border-color:#58421a}.notice.bad{background:var(--badbg);border-color:#6a2428}.notice b{color:var(--fg)}
.modal{position:fixed;inset:0;display:flex;align-items:flex-start;justify-content:center;padding:9vh 16px 16px;z-index:11;background:rgba(5,1,2,.66);backdrop-filter:blur(2px);animation:fade .14s;overflow:auto}
.box{width:min(480px,100%);padding:24px;background:linear-gradient(180deg,var(--panel2),var(--panel));border:1px solid var(--line2);border-radius:16px;box-shadow:var(--shadow);animation:pop .16s ease-out}
.box h3{margin:0 0 6px;font-size:18px}.box>p{margin:0 0 14px;color:var(--mute)}
.field{margin-bottom:12px}.field label{display:block;color:var(--mute);font-size:12px;margin-bottom:4px}
.foot{display:flex;gap:8px;justify-content:flex-end;margin-top:18px}
.check{display:flex;gap:8px;align-items:center;color:var(--mute);margin:4px 0}.check input{width:auto}
.keybox{margin-top:12px;padding:14px;border:1px solid #1e4a31;background:var(--okbg);border-radius:10px}
.keybox b{display:block;margin:6px 0 10px;font:600 17px ui-monospace,Consolas,monospace;word-break:break-all;user-select:all}
.toast{position:fixed;bottom:22px;left:50%;transform:translateX(-50%);z-index:20;padding:10px 18px;border-radius:10px;background:var(--panel2);border:1px solid var(--line2);box-shadow:var(--shadow);animation:pop .16s}
.toast.bad{background:var(--badbg);border-color:var(--bad);color:#fff}.toast.ok{border-color:var(--ok)}
@keyframes fade{from{opacity:0}}@keyframes slide{from{transform:translateX(30px);opacity:0}}@keyframes pop{from{transform:translateY(8px) scale(.98);opacity:0}}
@media (max-width:760px){main{padding:14px}.top{padding:10px 14px;flex-wrap:wrap}th:nth-child(3),td:nth-child(3),th:nth-child(5),td:nth-child(5){display:none}.panel-head input[type=search]{width:100%}.tiles{grid-template-columns:1fr 1fr}}
</style></head><body>
<header class="top">
  <div class="brand"><svg viewBox="0 0 64 64" aria-hidden="true"><rect width="64" height="64" rx="14" fill="#a3191f"/><path d="M32 12 14 52h9l3.4-8.6h11.2L41 52h9zM29 36l3-8 3 8z" fill="#fff"/></svg><b>Aside<span>Licences</span></b></div>
  <button id="reload" class="ghost" title="Reload" hidden>Reload</button><button id="exportbtn" class="ghost" hidden>Export CSV</button><button id="bulkbtn" hidden>Bulk issue</button><button id="newbtn" class="primary" hidden>Issue a licence</button><button id="logout" class="ghost" hidden>Sign out</button>
</header>
<div class="login" id="login"><h2>Sign in</h2><p>Enter the admin token of this server.</p><div class="row"><input id="token" type="password" placeholder="Admin token" autocomplete="off"><button class="primary" id="go">Sign in</button></div><div id="loginmsg"></div></div>
<main id="app" hidden>
  <div class="stats" id="stats"></div>
  <div class="grid">
    <section class="panel">
      <div class="panel-head"><h2>Licences</h2><input type="search" id="q" placeholder="Search by licence id or e-mail"><div class="chips" id="chips"></div></div>
      <div style="overflow:auto"><table><thead><tr><th>Licence</th><th>E-mail</th><th>Version</th><th>Computers</th><th>Last check</th><th>State</th></tr></thead><tbody id="rows"></tbody></table></div>
      <div class="empty" id="none" hidden>No licences found.</div>
    </section>
    <aside class="panel">
      <div class="panel-head"><h2>Recent activity</h2><div class="chips" id="evchips"></div></div>
      <ul class="events" id="events"></ul>
    </aside>
  </div>
</main>
<div id="layer"></div>
<script>
const $=(s)=>document.querySelector(s);let token=sessionStorage.getItem('aside_admin')||'';
let data={licences:[],events:[],stats:{}},filter='all',query='',timer=0,evFilter='';
const NL=String.fromCharCode(10),DAY=86400;
const el=(t,txt,cls)=>{const e=document.createElement(t);if(txt!==undefined&&txt!==null)e.textContent=txt;if(cls)e.className=cls;return e};
const when=(s)=>s?new Date(s*1000).toLocaleString():'never';
const dshort=(s)=>new Date(s*1000).toLocaleDateString();
function rel(s){if(!s)return 'never';const d=Math.floor(Date.now()/1000)-s;if(d<45)return 'just now';if(d<3600)return Math.round(d/60)+' min ago';if(d<86400)return Math.round(d/3600)+' h ago';return Math.round(d/86400)+' d ago'}
function toast(m,k){const t=el('div',m,'toast '+(k||''));document.body.append(t);setTimeout(()=>t.remove(),3400)}
async function api(path,opt){const r=await fetch('/admin/api/'+path,Object.assign({},opt,{headers:{authorization:'Bearer '+token,'content-type':'application/json'}}));const j=await r.json().catch(()=>({}));if(!r.ok)throw new Error(j.error||('error '+r.status));return j}
const post=(p,b)=>api(p,{method:'POST',body:JSON.stringify(b||{})});
async function copy(text,what){try{await navigator.clipboard.writeText(text);toast((what||'Text')+' copied','ok')}catch(e){toast('Copy failed – select and copy by hand','bad')}}

/* what a licence is now */
function status(l){if(l.state!=='active')return ['revoked','bad'];if(!l.last_seen)return ['not activated','warn'];if(Date.now()/1000-l.last_seen>l.check_days*DAY)return ['check overdue','warn'];return ['active','ok']}
function dots(l){const w=el('span','','dots');for(let i=0;i<l.max_devices;i++)w.append(el('i','',i<l.devices?'on':''));const b=el('span');b.append(w,el('span',l.devices+' / '+l.max_devices));return b}
function endsAt(l,lastSeen){return lastSeen?lastSeen+2*l.check_days*DAY:null}
const KINDS={issued:['Licence issued','acc'],activate:['Activated on a computer','ok'],'activate-again':['Activated again','ok'],refresh:['Regular check','ok'],deactivate:['Computer removed by the user',''],'release-user':['Computer freed by the user',''],refused:['Refused','bad'],revoked:['Licence revoked','bad'],restored:['Licence restored','ok'],released:['Computer freed by the owner',''],'check-days':['Check interval changed',''],'max-devices':['Computers allowed changed',''],updated:['Contact changed',''],erased:['Personal data erased',''],rekeyed:['New key made by the owner','acc'],'key-mailed':['Key mailed','ok'],'key-not-mailed':['Key not mailed (e-mail is not set up)','warn'],'recover-request':['Lost key: code requested','acc'],'recover-refused':['Lost key: already used once','warn'],'key-recovered':['Lost key: replaced by the user','acc']};
function eventList(ul,events,link){ul.replaceChildren();if(!events.length){const li=el('li');li.style.display='block';li.append(el('span','Nothing yet.','mute'));ul.append(li);return}
events.forEach(e=>{const k=KINDS[e.kind]||[e.kind,''];const li=el('li');const w=el('span','','what');w.append(el('span',k[0]));
if(link&&e.licence_id){const a=el('a',e.licence_id,'mono');a.style.marginLeft='8px';a.onclick=()=>openLicence(e.licence_id);w.append(a)}
const t=el('span',rel(e.at),'t');t.title=when(e.at);li.append(el('span','','dot '+k[1]),w,t);if(e.detail)li.append(el('span',e.detail,'sub mono'));ul.append(li)})}

/* the page */
function stat(label,value,cls,note){const d=el('div','','stat '+(cls||''));d.append(el('span',label),el('b',String(value)));if(note)d.append(el('small',note));return d}
function paint(){const s=data.stats;const st=$('#stats');st.replaceChildren(
  stat('Active licences',s.active,'good'),stat('Computers in use',s.devices,''),stat('Check overdue',s.overdue,s.overdue?'warn':'','no contact for a whole interval'),
  stat('Revoked',s.revoked,s.revoked?'bad':''),stat('Lost keys replaced',s.recovered||0,'acc','by the buyers, once each'),stat('Refused, 24 h',s.refused_day||0,s.refused_day?'warn':'','wrong keys, limits'));
const chips=$('#chips');chips.replaceChildren();[['all','All'],['active','Active'],['overdue','Overdue'],['revoked','Revoked']].forEach(([k,l])=>{const b=el('button',l,'chip'+(filter===k?' on':''));b.onclick=()=>{filter=k;paint()};chips.append(b)});
const rows=data.licences.filter(l=>{const name=status(l)[0];return filter==='all'||(filter==='active'&&l.state==='active')||(filter==='revoked'&&l.state!=='active')||(filter==='overdue'&&name==='check overdue')});
const tb=$('#rows');tb.replaceChildren();rows.forEach(l=>{const tr=el('tr','',l.state!=='active'?'cancelled':'');tr.onclick=()=>openLicence(l.id);
const id=el('td');id.append(el('span',l.id,'mono'));const bd=el('span','','badges');if(l.recovered)bd.append(el('span','key replaced','badge'));if(l.user_releases>=2)bd.append(el('span','no releases left','badge'));if(bd.childNodes.length)id.append(bd);
tr.append(id,el('td',l.email||'–','mute'),el('td',l.major+'.x'));const d=el('td');d.append(dots(l));tr.append(d);
tr.append(el('td',l.last_seen?rel(l.last_seen):'never','mute'));const c=el('td');const [n,k]=status(l);c.append(el('span',n,'pill '+k));tr.append(c);tb.append(tr)});
$('#none').hidden=rows.length>0;
const ec=$('#evchips');ec.replaceChildren();[['','All'],['refused','Refused'],['key-recovered','Lost keys']].forEach(([k,l])=>{const b=el('button',l,'chip'+(evFilter===k?' on':''));b.onclick=()=>{evFilter=k;load()};ec.append(b)});
eventList($('#events'),data.events,true)}
async function load(){try{const j=await api('overview?q='+encodeURIComponent(query)+(evFilter?'&events='+evFilter:''));data=j;$('#login').hidden=true;$('#app').hidden=false;['reload','exportbtn','bulkbtn','newbtn','logout'].forEach(i=>{$('#'+i).hidden=false});paint()}
catch(e){$('#loginmsg').textContent=e.message==='unauthorised'?'That token is not right.':(e.message==='admin_not_configured'?'The server has no ADMIN_TOKEN (at least 16 characters).':e.message);if(e.message==='unauthorised'){sessionStorage.removeItem('aside_admin');$('#app').hidden=true;$('#login').hidden=false}}}

/* the dialogs of the page (not the ones of the browser) */
function close(){$('#layer').replaceChildren()}
function dialog(title,text,fields,opts){opts=opts||{};return new Promise((resolve)=>{
  const m=el('div','','modal');const box=el('div','','box');box.append(el('h3',title));if(text)box.append(el('p',text));
  const inputs={};(fields||[]).forEach(f=>{const w=el('div','','field');const i=el('input');i.type=f.type||'text';
    if(f.type==='checkbox'){i.checked=!!f.value;const row=el('label','','check');row.append(i,f.label);w.append(row)}
    else{i.value=f.value===undefined?'':f.value;if(f.placeholder)i.placeholder=f.placeholder;w.append(el('label',f.label),i)}
    inputs[f.id]=i;box.append(w)});
  const done=(v)=>{m.remove();resolve(v)};
  const foot=el('div','','foot');const no=el('button',opts.cancel||'Cancel');no.onclick=()=>done(null);const yes=el('button',opts.ok||'OK',opts.danger?'danger':'primary');
  yes.onclick=()=>{const out={};Object.keys(inputs).forEach(k=>{out[k]=inputs[k].type==='checkbox'?inputs[k].checked:inputs[k].value});done(out)};foot.append(no,yes);box.append(foot);m.append(box);
  m.onclick=(e)=>{if(e.target===m)done(null)};box.onkeydown=(e)=>{if(e.key==='Enter'&&e.target.tagName==='INPUT')yes.click()};
  $('#layer').append(m);const first=box.querySelector('input');if(first)first.focus();else yes.focus()})}
const ask=(title,text,ok,danger)=>dialog(title,text,[],{ok:ok,danger:danger}).then(v=>v!==null);

/* a licence */
async function openLicence(id){let j;try{j=await api('licence/'+id)}catch(e){toast(e.message,'bad');return}
  const l=j.licence;const listed=data.licences.find(x=>x.id===l.id)||{};const L=$('#layer');L.replaceChildren();
  const scrim=el('div','','scrim');scrim.onclick=close;const d=el('aside','','drawer');
  const head=el('div','','drawer-head');head.append(el('h3',l.id,'mono'));const [name,cls]=status(Object.assign({},l,listed));head.append(el('span',name,'pill '+cls));const x=el('button','Close','ghost');x.onclick=close;head.append(x);d.append(head);
  const live=j.activations.filter(a=>!a.released);
  const left=Math.max(0,j.release_limit-j.user_releases);
  const tiles=el('div','','tiles');
  [['Computers',live.length+' / '+l.max_devices,''],['Freed by the user',j.user_releases+' / '+j.release_limit,left===0?'warn':''],['Lost key',j.recovered?'replaced':'not used',j.recovered?'warn':'']].forEach(([k,v,c])=>{const t=el('div','','tile '+c);t.append(el('span',k),el('b',v));tiles.append(t)});d.append(tiles);
  if(l.state!=='active'){const ends=live.map(a=>endsAt(l,a.last_seen)).filter(Boolean);const wb=el('div','','notice warn');wb.append(el('b','Cancelled. '));
    if(!live.length)wb.append('No computer holds this licence.');else{const last=Math.max.apply(null,ends);wb.append('A computer that is online stops within hours (Aside asks the server when it starts and every few hours). A computer that stays offline stops at the latest on '+dshort(last)+(last*1000<Date.now()?' (that day has passed, so all of them have stopped).':'.'))}d.append(wb)}
  const kv=el('dl','','kv');[['E-mail',l.email||'–'],['Features',l.features],['Version',l.major+'.x'],['Check every',l.check_days+' days'],['Created',when(l.created)],['Order',l.order_ref||'by hand'],['Note',l.note||'–']].forEach(([k,v])=>{kv.append(el('dt',k),el('dd',v))});d.append(kv);
  const reload=async()=>{await load();openLicence(l.id)};
  const act=(label,fn,c)=>{const b=el('button',label,c||'');b.onclick=async()=>{try{if((await fn())===false)return;await reload()}catch(e){toast(e.message,'bad')}};return b};
  d.append(el('h4','Actions','sec'));const acts=el('div','','acts');
  const cp=el('button','Copy id');cp.onclick=()=>copy(l.id,'Licence id');acts.append(cp);
  acts.append(act('Computers allowed',async()=>{const r=await dialog('Computers allowed','How many computers may use this licence? (1–20)',[{id:'n',label:'Number',type:'number',value:l.max_devices}],{ok:'Save'});if(!r)return false;await post('licence/'+l.id+'/max_devices',{n:Number(r.n)})}));
  acts.append(act('Check interval',async()=>{const r=await dialog('Check interval','How often Aside asks the server (days, 1–365). The file stays valid for the same time again without a connection.',[{id:'days',label:'Days',type:'number',value:l.check_days}],{ok:'Save'});if(!r)return false;await post('licence/'+l.id+'/check_days',{days:Number(r.days)})}));
  acts.append(act('Edit contact',async()=>{const r=await dialog('Contact','The e-mail is where the key and the codes for a lost key go.',[{id:'email',label:'E-mail of the buyer',type:'email',value:l.email||''},{id:'note',label:'Note',value:l.note||''}],{ok:'Save'});if(!r)return false;await post('licence/'+l.id+'/update',{email:r.email,note:r.note})}));
  acts.append(act('New key',async()=>{const r=await dialog('New key','The old key stops working for new computers at once. Computers that are already activated keep working, unless you free them too.',(l.email?[{id:'send',label:'Mail the new key to '+l.email,type:'checkbox',value:true}]:[]).concat([{id:'freeall',label:'Also free all computers (if the key leaked): every computer has to enter the new key',type:'checkbox',value:false}]),{ok:'Make a new key',danger:true});if(!r)return false;const out=await post('licence/'+l.id+'/rekey',{send:!!r.send,free_all:!!r.freeall});if(out.freed)toast(out.freed+' computer(s) freed','ok');showKey(out.key,l.id,out.mailed,l.email);return false}));
  d.append(acts);
  d.append(el('h4','Danger zone','sec'));const dz=el('div','','acts');
  if(l.state==='active')dz.append(act('Revoke licence',async()=>{const ends=live.map(a=>endsAt(l,a.last_seen)).filter(Boolean);const when2=ends.length?dshort(Math.max.apply(null,ends)):'–';const r=await dialog('Revoke '+l.id+'?','Computers that are online stop within hours. Computers that stay offline stop at the latest on '+when2+'.',[{id:'reason',label:'Reason (for the history)',value:''}],{ok:'Revoke',danger:true});if(!r)return false;await post('licence/'+l.id+'/revoke',{reason:r.reason})},'danger'));
  else dz.append(act('Restore licence',()=>post('licence/'+l.id+'/restore')));
  dz.append(act('Erase personal data',async()=>{if(!await ask('Erase personal data?','The e-mail and the note of '+l.id+' go, also from its history. The licence itself stays.','Erase',true))return false;await post('licence/'+l.id+'/erase',{})},'danger'));
  d.append(dz);
  d.append(el('h4','Computers','sec'));if(!j.activations.length)d.append(el('p','No computer has activated this licence yet.','mute'));
  j.activations.forEach(a=>{const r=el('div','','dev');const t=el('div','','grow');t.append(el('div',a.name||'Unnamed computer'),el('div',a.device.slice(0,12)+'…'+(a.app_version?'  ·  v'+a.app_version:''),'mono faint'),el('div','since '+when(a.created)+'  ·  last seen '+rel(a.last_seen)+(l.state!=='active'&&!a.released?'  ·  stops by '+dshort(endsAt(l,a.last_seen)):''),'small mute'));r.append(t);
    if(a.released)r.append(el('span','freed','pill'));else{const b=el('button','Free');b.onclick=async()=>{if(!await ask('Free this place?','The computer stops working with this licence. This does not count against the limit of the user.','Free'))return;try{await post('licence/'+l.id+'/release',{device:a.device});await reload()}catch(e){toast(e.message,'bad')}};r.append(b)}d.append(r)});
  d.append(el('h4','History','sec'));const ul=el('ul','','events');ul.style.maxHeight='none';eventList(ul,j.events,false);d.append(ul);L.append(scrim,d)}

/* new keys, issuing */
function showKey(key,id,mailed,email){const L=$('#layer');const m=el('div','','modal');const box=el('div','','box');box.append(el('h3','Licence key'),el('p','Licence '+id+(mailed?' · mailed to '+email:' · not mailed')));
  const kb=el('div','','keybox');kb.append(el('span','Shown only now – copy it before you close this window.','mute'),el('b',key));const c=el('button','Copy key','primary');c.onclick=()=>copy(key,'Key');kb.append(c);box.append(kb);
  const foot=el('div','','foot');const done=el('button','Done');done.onclick=close;foot.append(done);box.append(foot);m.append(box);L.append(m)}
async function issueForm(){const r=await dialog('Issue a licence','A key is made and shown once. Mail it to the buyer or copy it.',[
  {id:'email',label:'E-mail of the buyer',type:'email',placeholder:'name@example.com'},{id:'features',label:'Features',placeholder:'all (or replace.regex,mt.batch)'},{id:'days',label:'Check every (days)',type:'number',placeholder:'14'},{id:'devices',label:'Computers',type:'number',placeholder:'2'},{id:'note',label:'Note',placeholder:'optional'},{id:'send',label:'Mail the key to the buyer',type:'checkbox'}],{ok:'Create'});
  if(!r)return;try{const o=await post('issue',{email:r.email.trim(),features:r.features.trim(),check_days:Number(r.days)||undefined,max_devices:Number(r.devices)||undefined,note:r.note,send:r.send});await load();showKey(o.key,o.licence.id,o.mailed,o.licence.email)}catch(e){toast(e.message,'bad')}}
async function bulkForm(){const r=await dialog('Issue many licences','For testers or a giveaway. The licences have no e-mail; the keys are shown once.',[
  {id:'count',label:'How many (1–100)',type:'number',placeholder:'10'},{id:'note',label:'Note for all of them',placeholder:'testers, giveaway…'},{id:'features',label:'Features',placeholder:'all'},{id:'days',label:'Check every (days)',type:'number',placeholder:'14'},{id:'devices',label:'Computers each',type:'number',placeholder:'2'}],{ok:'Create'});
  if(!r)return;try{const o=await post('issue_bulk',{count:Number(r.count),note:r.note,features:r.features.trim(),check_days:Number(r.days)||undefined,max_devices:Number(r.devices)||undefined});await load();showKeys(o.licences)}catch(e){toast(e.message,'bad')}}
function showKeys(list){const L=$('#layer');L.replaceChildren();const m=el('div','','modal');const box=el('div','','box');box.append(el('h3',list.length+' keys made'));
  const text=list.map(x=>x.id+'  '+x.key).join(NL);const kb=el('div','','keybox');kb.append(el('span','Shown only now – copy or download them before you close this window.','mute'));
  const pre=el('textarea');pre.readOnly=true;pre.rows=Math.min(12,list.length+1);pre.style.fontFamily='ui-monospace,Consolas,monospace';pre.style.marginTop='8px';pre.value=text;kb.append(pre);
  const bar=el('div','','foot');bar.style.justifyContent='flex-start';const c=el('button','Copy all','primary');c.onclick=()=>copy(text,'Keys');const dl=el('button','Download .txt');
  dl.onclick=()=>{const a=el('a');a.href=URL.createObjectURL(new Blob([text+NL],{type:'text/plain'}));a.download='aside-keys.txt';a.click();setTimeout(()=>URL.revokeObjectURL(a.href),2000)};bar.append(c,dl);kb.append(bar);
  box.append(kb);const foot=el('div','','foot');const done=el('button','Done');done.onclick=close;foot.append(done);box.append(foot);m.append(box);L.append(m)}
async function exportCsv(){try{const r=await fetch('/admin/api/export',{headers:{authorization:'Bearer '+token}});if(!r.ok)throw new Error('error '+r.status);const a=el('a');a.href=URL.createObjectURL(await r.blob());a.download='aside-licences.csv';a.click();setTimeout(()=>URL.revokeObjectURL(a.href),2000)}catch(e){toast(e.message,'bad')}}

function signIn(){token=$('#token').value.trim();sessionStorage.setItem('aside_admin',token);$('#loginmsg').textContent='';load()}
$('#go').onclick=signIn;$('#token').onkeydown=(e)=>{if(e.key==='Enter')signIn()};
$('#logout').onclick=()=>{sessionStorage.removeItem('aside_admin');token='';location.reload()};
$('#newbtn').onclick=issueForm;$('#bulkbtn').onclick=bulkForm;$('#exportbtn').onclick=exportCsv;$('#reload').onclick=load;
$('#q').oninput=()=>{clearTimeout(timer);timer=setTimeout(()=>{query=$('#q').value;load()},250)};
document.addEventListener('keydown',(e)=>{if(e.key==='Escape')close()});
if(token)load();else $('#token').focus();
</script></body></html>`;
