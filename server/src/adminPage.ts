/** The admin page: one HTML file, no libraries. The token is typed in and kept only in this tab (sessionStorage); everything from the server goes in through textContent. */
export const ADMIN_PAGE = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex"><title>Aside licences</title>
<link rel="icon" href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'%3E%3Cdefs%3E%3ClinearGradient id='g' x1='0' y1='1' x2='1' y2='0'%3E%3Cstop stop-color='%23ff315c'/%3E%3Cstop offset='1' stop-color='%23ff7043'/%3E%3C/linearGradient%3E%3C/defs%3E%3Crect width='64' height='64' rx='14' fill='url(%23g)'/%3E%3Cpath d='M32 14 48 50h-9l-3-8H28l-3 8h-9z' fill='%23fff'/%3E%3C/svg%3E">
<style>
:root{color-scheme:light dark;--bg:#f5f5f7;--panel:#fff;--fg:#1c1c1e;--mute:#6b6b70;--line:#e3e3e8;--hover:#f0f0f4;--accent:#ff315c;--ok:#1a7f37;--warn:#9a6700;--bad:#cf222e;--okbg:#e6f4ea;--warnbg:#fff4d6;--badbg:#fde8ea;--shadow:0 1px 2px rgba(0,0,0,.06),0 8px 24px rgba(0,0,0,.06)}
@media (prefers-color-scheme:dark){:root{--bg:#101012;--panel:#1a1a1d;--fg:#ececf0;--mute:#9a9aa2;--line:#2c2c31;--hover:#222226;--ok:#3fb950;--warn:#d29922;--bad:#ff7b72;--okbg:#12301b;--warnbg:#33290e;--badbg:#3a1618;--shadow:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.4)}}
*{box-sizing:border-box}body{margin:0;font:14px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif;background:var(--bg);color:var(--fg)}
header{display:flex;align-items:center;gap:12px;padding:12px 24px;background:var(--panel);border-bottom:1px solid var(--line);position:sticky;top:0;z-index:5}
header svg{width:28px;height:28px}header h1{font-size:16px;margin:0;font-weight:600}header .sp{flex:1}
main{max-width:1100px;margin:0 auto;padding:20px 24px 60px}
input,select,textarea,button{font:inherit;color:inherit;background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:7px 12px}
input:focus,button:focus-visible{outline:2px solid var(--accent);outline-offset:1px}
button{cursor:pointer}button:hover{background:var(--hover)}button.primary{background:var(--accent);border-color:var(--accent);color:#fff;font-weight:500}button.primary:hover{filter:brightness(1.08)}
button.danger{color:var(--bad)}button.ghost{border-color:transparent;background:transparent}
.login{max-width:380px;margin:12vh auto;background:var(--panel);border:1px solid var(--line);border-radius:14px;padding:24px;box-shadow:var(--shadow)}
.login h2{margin:0 0 4px;font-size:18px}.login p{margin:0 0 16px;color:var(--mute)}.login .row{display:flex;gap:8px}.login input{flex:1}
.stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:12px;margin-bottom:20px}
.stat{background:var(--panel);border:1px solid var(--line);border-radius:12px;padding:14px 16px}.stat span{display:block;color:var(--mute);font-size:12px;text-transform:uppercase;letter-spacing:.04em}
.stat b{font-size:28px;font-weight:600}.stat.warn b{color:var(--warn)}.stat.bad b{color:var(--bad)}
.bar{display:flex;gap:8px;flex-wrap:wrap;align-items:center;margin-bottom:12px}.bar input[type=search]{flex:1;min-width:200px}
.chips{display:flex;gap:6px}.chip{padding:5px 12px;border-radius:999px;font-size:13px}.chip.on{background:var(--fg);color:var(--bg);border-color:var(--fg)}
.card{background:var(--panel);border:1px solid var(--line);border-radius:12px;overflow:hidden}
table{width:100%;border-collapse:collapse}th{text-align:left;font-weight:500;color:var(--mute);font-size:12px;text-transform:uppercase;letter-spacing:.04em;padding:10px 14px;border-bottom:1px solid var(--line);white-space:nowrap}
td{padding:11px 14px;border-bottom:1px solid var(--line);vertical-align:middle}tr:last-child td{border-bottom:0}tbody tr{cursor:pointer}tbody tr:hover{background:var(--hover)}
.mono{font-family:ui-monospace,Consolas,monospace;font-size:13px}.mute{color:var(--mute)}.empty{padding:32px;text-align:center;color:var(--mute)}
.pill{display:inline-block;padding:2px 10px;border-radius:999px;font-size:12px;font-weight:500}.pill.ok{background:var(--okbg);color:var(--ok)}.pill.warn{background:var(--warnbg);color:var(--warn)}.pill.bad{background:var(--badbg);color:var(--bad)}
.dots{display:inline-flex;gap:3px;vertical-align:middle;margin-right:6px}.dots i{width:9px;height:9px;border-radius:50%;border:1.5px solid var(--mute)}.dots i.on{background:var(--accent);border-color:var(--accent)}
h2.sec{font-size:15px;margin:28px 0 10px;font-weight:600}
.events li{display:flex;gap:10px;align-items:baseline;padding:9px 14px;border-bottom:1px solid var(--line)}.events li:last-child{border-bottom:0}.events{list-style:none;margin:0;padding:0}
.events .dot{width:8px;height:8px;border-radius:50%;background:var(--mute);flex:none;transform:translateY(-1px)}.events .dot.ok{background:var(--ok)}.events .dot.bad{background:var(--bad)}.events .dot.warn{background:var(--warn)}.events .dot.acc{background:var(--accent)}
.events .t{margin-left:auto;color:var(--mute);white-space:nowrap;font-size:12px}.events a{color:var(--accent);cursor:pointer;text-decoration:none}
.scrim{position:fixed;inset:0;background:rgba(0,0,0,.35);z-index:9}.drawer{position:fixed;top:0;right:0;bottom:0;width:min(440px,100%);background:var(--panel);border-left:1px solid var(--line);box-shadow:var(--shadow);z-index:10;overflow:auto;padding:20px}
.drawer h3{margin:0;font-size:17px}.kv{display:grid;grid-template-columns:120px 1fr;gap:6px 12px;margin:14px 0}.kv dt{color:var(--mute)}.kv dd{margin:0;word-break:break-word}
.drawer .acts{display:flex;gap:8px;flex-wrap:wrap;margin:14px 0}.dev{display:flex;align-items:center;gap:8px;padding:8px 0;border-bottom:1px solid var(--line)}.dev span:first-child{flex:1}
.modal{position:fixed;inset:0;display:flex;align-items:flex-start;justify-content:center;padding-top:10vh;z-index:11;background:rgba(0,0,0,.35)}.modal .box{background:var(--panel);border-radius:14px;padding:22px;width:min(480px,94%);box-shadow:var(--shadow);border:1px solid var(--line)}
.modal h3{margin:0 0 14px}.field{margin-bottom:12px}.field label{display:block;color:var(--mute);font-size:12px;margin-bottom:3px}.field input{width:100%}.two{display:grid;grid-template-columns:1fr 1fr;gap:10px}
.keybox{background:var(--okbg);border:1px solid var(--ok);border-radius:10px;padding:12px;margin-top:12px}.keybox b{display:block;font-family:ui-monospace,Consolas,monospace;font-size:16px;margin:4px 0 8px;user-select:all;word-break:break-all}
.toast{position:fixed;bottom:20px;left:50%;transform:translateX(-50%);background:var(--fg);color:var(--bg);padding:9px 16px;border-radius:10px;z-index:20}.toast.bad{background:var(--bad);color:#fff}
[hidden]{display:none!important}
@media (max-width:640px){main{padding:14px}th:nth-child(3),td:nth-child(3),th:nth-child(5),td:nth-child(5){display:none}}
</style></head><body>
<header><svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="g" x1="0" y1="1" x2="1" y2="0"><stop stop-color="#ff315c"/><stop offset="1" stop-color="#ff7043"/></linearGradient></defs><rect width="64" height="64" rx="14" fill="url(#g)"/><path d="M32 14 48 50h-9l-3-8H28l-3 8h-9z" fill="#fff"/></svg><h1>Aside – licences</h1><span class="sp"></span><span class="mute" id="who" hidden>signed in</span><button class="ghost" id="logout" hidden>Sign out</button></header>
<div class="login" id="login"><h2>Sign in</h2><p>Enter the admin token of this server.</p><div class="row"><input id="token" type="password" placeholder="Admin token" autocomplete="off"><button class="primary" id="go">Sign in</button></div><p class="mute" id="loginmsg" style="margin:10px 0 0"></p></div>
<main id="app" hidden>
<div class="stats" id="stats"></div>
<div class="bar"><input type="search" id="q" placeholder="Search by licence id or e-mail"><div class="chips" id="chips"></div><button class="primary" id="newbtn">Issue a licence</button><button id="bulkbtn">Bulk issue</button><button id="exportbtn">Export CSV</button><button id="reload" title="Reload">Reload</button></div>
<div class="card"><table><thead><tr><th>Licence</th><th>E-mail</th><th>Version</th><th>Devices</th><th>Last check</th><th>State</th></tr></thead><tbody id="rows"></tbody></table><div class="empty" id="none" hidden>No licences match.</div></div>
<div class="bar" style="margin-top:28px"><h2 class="sec" style="margin:0;flex:1">Recent activity</h2><div class="chips" id="evchips"></div></div><div class="card"><ul class="events" id="events"></ul></div>
</main>
<div id="layer"></div>
<script>
const $=(s)=>document.querySelector(s);let token=sessionStorage.getItem('aside_admin')||'';
let data={licences:[],events:[],stats:{}},filter='all',query='',timer=0,evFilter='';
const el=(t,txt,cls)=>{const e=document.createElement(t);if(txt!==undefined&&txt!==null)e.textContent=txt;if(cls)e.className=cls;return e};
const when=(s)=>s?new Date(s*1000).toLocaleString():'never';
function rel(s){if(!s)return 'never';const d=Math.floor(Date.now()/1000)-s;if(d<45)return 'just now';if(d<3600)return Math.round(d/60)+' min ago';if(d<86400)return Math.round(d/3600)+' h ago';return Math.round(d/86400)+' d ago'}
function toast(m,k){const t=el('div',m,'toast '+(k||''));document.body.append(t);setTimeout(()=>t.remove(),3200)}
async function api(path,opt){const r=await fetch('/admin/api/'+path,{...opt,headers:{authorization:'Bearer '+token,'content-type':'application/json'}});const j=await r.json().catch(()=>({}));if(!r.ok)throw new Error(j.error||('error '+r.status));return j}
const post=(p,b)=>api(p,{method:'POST',body:JSON.stringify(b||{})});
async function copy(text,what){try{await navigator.clipboard.writeText(text);toast((what||'Text')+' copied')}catch(e){toast('Copy failed – select and copy by hand','bad')}}
function status(l){if(l.state!=='active')return ['revoked','bad'];if(l.last_seen&&Date.now()/1000-l.last_seen>l.check_days*86400)return ['check overdue','warn'];if(!l.last_seen)return ['not activated','warn'];return ['active','ok']}
function dots(l){const w=el('span','','dots');for(let i=0;i<l.max_devices;i++)w.append(el('i','',i<l.devices?'on':''));const s=el('span',l.devices+' / '+l.max_devices);const b=el('span');b.append(w,s);return b}
const KINDS={issued:['Licence issued','acc'],activate:['Activated on a device','ok'],'activate-again':['Activated again','ok'],refresh:['Regular check','ok'],deactivate:['Device removed by the user',''],refused:['Refused','bad'],revoked:['Licence revoked','bad'],restored:['Licence restored','ok'],released:['Device freed by the owner',''],'check-days':['Check interval changed',''],rekeyed:['New key made','acc'],'key-mailed':['Key mailed','ok'],'key-not-mailed':['Key not mailed (e-mail is not set up)','warn']};
function eventList(ul,events,link){ul.replaceChildren();if(!events.length){ul.append(el('li','Nothing yet.','mute'));return}
events.forEach(e=>{const k=KINDS[e.kind]||[e.kind,''];const li=el('li');li.append(el('span','','dot '+k[1]),el('span',k[0]));if(e.detail)li.append(el('span',e.detail,'mute mono'));
if(link&&e.licence_id){const a=el('a',e.licence_id,'mono');a.onclick=()=>openLicence(e.licence_id);li.append(a)}
const t=el('span',rel(e.at),'t');t.title=when(e.at);li.append(t);ul.append(li)})}
function paint(){const s=data.stats;const st=$('#stats');st.replaceChildren();
[['Active licences',s.active,''],['Devices in use',s.devices,''],['Check overdue',s.overdue,s.overdue?'warn':''],['Revoked',s.revoked,s.revoked?'bad':'']].forEach(([k,v,c])=>{const d=el('div','','stat '+c);d.append(el('span',k),el('b',String(v||0)));st.append(d)});
const chips=$('#chips');chips.replaceChildren();[['all','All'],['active','Active'],['overdue','Overdue'],['revoked','Revoked']].forEach(([k,l])=>{const b=el('button',l,'chip'+(filter===k?' on':''));b.onclick=()=>{filter=k;paint()};chips.append(b)});
const rows=data.licences.filter(l=>{const [name]=status(l);return filter==='all'||(filter==='active'&&l.state==='active')||(filter==='revoked'&&l.state!=='active')||(filter==='overdue'&&name==='check overdue')});
const tb=$('#rows');tb.replaceChildren();rows.forEach(l=>{const tr=el('tr');tr.append(el('td',l.id,'mono'),el('td',l.email||'–','mute'),el('td',l.major+'.x'));const d=el('td');d.append(dots(l));tr.append(d);const c=el('td',rel(l.last_seen),'mute');c.title=when(l.last_seen);tr.append(c);const [name,cls]=status(l);const p=el('td');p.append(el('span',name,'pill '+cls));tr.append(p);tr.onclick=()=>openLicence(l.id);tb.append(tr)});
$('#none').hidden=rows.length>0;const ec=$('#evchips');ec.replaceChildren();[['','All'],['refused','Refused only']].forEach(([k,l])=>{const b=el('button',l,'chip'+(evFilter===k?' on':''));b.onclick=()=>{evFilter=k;load()};ec.append(b)});eventList($('#events'),data.events,true)}
async function load(){try{const j=await api('overview?q='+encodeURIComponent(query)+(evFilter?'&events='+evFilter:''));data=j;$('#login').hidden=true;$('#app').hidden=false;$('#who').hidden=false;$('#logout').hidden=false;paint()}catch(e){$('#loginmsg').textContent=e.message==='unauthorised'?'That token is not right.':(e.message==='admin_not_configured'?'The server has no ADMIN_TOKEN (at least 16 characters).':e.message);if(e.message==='unauthorised'){sessionStorage.removeItem('aside_admin');$('#app').hidden=true;$('#login').hidden=false}}}
function close(){$('#layer').replaceChildren()}
async function openLicence(id){let j;try{j=await api('licence/'+id)}catch(e){toast(e.message,'bad');return}
const l=j.licence;const L=$('#layer');L.replaceChildren();const scrim=el('div','','scrim');scrim.onclick=close;const d=el('aside','','drawer');
const head=el('div','','dev');head.append(el('h3',l.id,'mono'),el('span','',''));const x=el('button','Close','ghost');x.onclick=close;head.append(x);d.append(head);
const [name,cls]=status(Object.assign({},l,data.licences.find(x=>x.id===l.id)||{}));const pill=el('span',name,'pill '+cls);d.append(pill);
const kv=el('dl','','kv');[['E-mail',l.email||'–'],['Features',l.features],['Version',l.major+'.x'],['Check every',l.check_days+' days'],['Devices',l.max_devices+' allowed'],['Created',when(l.created)],['Order',l.order_ref||'by hand'],['Note',l.note||'–']].forEach(([k,v])=>{kv.append(el('dt',k),el('dd',v))});d.append(kv);
const acts=el('div','','acts');const act=(label,fn,c)=>{const b=el('button',label,c||'');b.onclick=async()=>{try{await fn();await load();openLicence(l.id)}catch(e){toast(e.message,'bad')}};acts.append(b)};
const cp=el('button','Copy id');cp.onclick=()=>copy(l.id,'Licence id');acts.append(cp);
if(l.state==='active')act('Revoke',async()=>{if(!confirm('Revoke '+l.id+'? Its devices will stop being renewed.'))throw new Error('cancelled');await post('licence/'+l.id+'/revoke',{})},'danger');else act('Restore',()=>post('licence/'+l.id+'/restore'));
act('Devices allowed',async()=>{const v=prompt('How many computers may use this licence? (1-20)',l.max_devices);if(!v)throw new Error('cancelled');await post('licence/'+l.id+'/max_devices',{n:Number(v)})});
act('Edit contact',async()=>{const e=prompt('E-mail of the buyer (empty = none):',l.email||'');if(e===null)throw new Error('cancelled');const n=prompt('Note:',l.note||'');if(n===null)throw new Error('cancelled');await post('licence/'+l.id+'/update',{email:e,note:n})});
act('Check interval',async()=>{const v=prompt('Check every how many days? (1-365)',l.check_days);if(!v)throw new Error('cancelled');await post('licence/'+l.id+'/check_days',{days:Number(v)})});
act('Erase personal data',async()=>{if(!confirm('Remove the e-mail and the note of '+l.id+' (also from its history)? The licence itself stays.'))throw new Error('cancelled');await post('licence/'+l.id+'/erase',{})},'danger');
act('New key',async()=>{if(!confirm('The old key stops working at once. Continue?'))throw new Error('cancelled');const r=await post('licence/'+l.id+'/rekey',{send:l.email?confirm('Mail the new key to '+l.email+'?'):false});showKey(r.key,l.id,r.mailed,l.email)});d.append(acts);
d.append(el('h2','Devices','sec'));if(!j.activations.length)d.append(el('p','No device has activated this licence yet.','mute'));
j.activations.forEach(a=>{const r=el('div','','dev');const t=el('span');t.append(el('span',a.device.slice(0,12)+'…','mono'),el('br'),el('small','since '+when(a.created)+' · last seen '+rel(a.last_seen),'mute'));r.append(t);
if(a.released)r.append(el('span','freed','mute'));else{const b=el('button','Free');b.onclick=async()=>{try{await post('licence/'+l.id+'/release',{device:a.device});await load();openLicence(l.id)}catch(e){toast(e.message,'bad')}};r.append(b)}d.append(r)});
d.append(el('h2','History','sec'));const ul=el('ul','','events');eventList(ul,j.events,false);d.append(ul);L.append(scrim,d)}
function showKey(key,id,mailed,email){const L=$('#layer');const m=el('div','','modal');const box=el('div','','box');box.append(el('h3','New licence key'));
const kb=el('div','','keybox');kb.append(el('span','Shown only now – copy it before you close this window.','mute'),el('b',key));const c=el('button','Copy key','primary');c.onclick=()=>copy(key,'Key');kb.append(c);
box.append(el('p','Licence '+id+(mailed?' · mailed to '+email:' · not mailed'),'mute'),kb);const done=el('button','Done');done.style.marginTop='14px';done.onclick=close;box.append(done);m.append(box);L.append(m)}
function issueForm(){const L=$('#layer');L.replaceChildren();const m=el('div','','modal');const box=el('div','','box');box.append(el('h3','Issue a licence'));
const f=(id,label,ph,type)=>{const w=el('div','','field');const lb=el('label',label);lb.htmlFor=id;const i=el('input');i.id=id;i.type=type||'text';i.placeholder=ph||'';w.append(lb,i);return w};
box.append(f('i_email','E-mail of the buyer','name@example.com','email'),f('i_features','Features','all (or replace.regex,mt.batch)'));
const two=el('div','','two');two.append(f('i_days','Check every (days)','14','number'),f('i_devices','Devices','2','number'));box.append(two,f('i_note','Note','optional'));
const send=el('label','','mute');const cb=el('input');cb.type='checkbox';cb.id='i_send';cb.style.marginRight='6px';send.append(cb,'Mail the key to the buyer');box.append(send);
const row=el('div','','dev');row.style.borderBottom='0';const cancel=el('button','Cancel');cancel.onclick=close;const go=el('button','Create','primary');
go.onclick=async()=>{try{const r=await post('issue',{email:$('#i_email').value.trim(),features:$('#i_features').value.trim(),check_days:Number($('#i_days').value)||undefined,max_devices:Number($('#i_devices').value)||undefined,note:$('#i_note').value,send:$('#i_send').checked});await load();showKey(r.key,r.licence.id,r.mailed,r.licence.email)}catch(e){toast(e.message,'bad')}};
row.append(el('span'),cancel,go);box.append(row);m.append(box);m.onclick=(e)=>{if(e.target===m)close()};L.append(m);$('#i_email').focus()}
function bulkForm(){const L=$('#layer');L.replaceChildren();const m=el('div','','modal');const box=el('div','','box');box.append(el('h3','Issue many licences'));
const f=(id,label,ph,type)=>{const w=el('div','','field');const lb=el('label',label);lb.htmlFor=id;const i=el('input');i.id=id;i.type=type||'text';i.placeholder=ph||'';w.append(lb,i);return w};
box.append(f('b_count','How many (1-100)','10','number'),f('b_note','Note for all of them','testers, giveaway…'),f('b_features','Features','all'));
const two=el('div','','two');two.append(f('b_days','Check every (days)','14','number'),f('b_devices','Devices each','2','number'));box.append(two);
const row=el('div','','dev');row.style.borderBottom='0';const cancel=el('button','Cancel');cancel.onclick=close;const go=el('button','Create','primary');
go.onclick=async()=>{try{const r=await post('issue_bulk',{count:Number($('#b_count').value),note:$('#b_note').value,features:$('#b_features').value.trim(),check_days:Number($('#b_days').value)||undefined,max_devices:Number($('#b_devices').value)||undefined});await load();showKeys(r.licences)}catch(e){toast(e.message,'bad')}};
row.append(el('span'),cancel,go);box.append(row);m.append(box);m.onclick=(e)=>{if(e.target===m)close()};L.append(m);$('#b_count').focus()}
function showKeys(list){const L=$('#layer');L.replaceChildren();const m=el('div','','modal');const box=el('div','','box');box.append(el('h3',list.length+' keys made'));
const text=list.map(x=>x.id+'  '+x.key).join('\\n');const kb=el('div','','keybox');kb.append(el('span','Shown only now – copy or download them before you close this window.','mute'));const pre=el('textarea');pre.readOnly=true;pre.rows=Math.min(12,list.length+1);pre.style.width='100%';pre.style.fontFamily='ui-monospace,Consolas,monospace';pre.value=text;kb.append(pre);
const bar=el('div','','dev');bar.style.borderBottom='0';const c=el('button','Copy all','primary');c.onclick=()=>copy(text,'Keys');const d=el('button','Download .txt');d.onclick=()=>{const a=el('a');a.href=URL.createObjectURL(new Blob([text+'\\n'],{type:'text/plain'}));a.download='aside-keys.txt';a.click();setTimeout(()=>URL.revokeObjectURL(a.href),2000)};bar.append(c,d);kb.append(bar);
box.append(kb);const done=el('button','Done');done.style.marginTop='14px';done.onclick=close;box.append(done);m.append(box);L.append(m)}
async function exportCsv(){try{const r=await fetch('/admin/api/export',{headers:{authorization:'Bearer '+token}});if(!r.ok)throw new Error('error '+r.status);const a=el('a');a.href=URL.createObjectURL(await r.blob());a.download='aside-licences.csv';a.click();setTimeout(()=>URL.revokeObjectURL(a.href),2000)}catch(e){toast(e.message,'bad')}}
function signIn(){token=$('#token').value.trim();sessionStorage.setItem('aside_admin',token);$('#loginmsg').textContent='';load()}
$('#go').onclick=signIn;$('#token').onkeydown=(e)=>{if(e.key==='Enter')signIn()};
$('#logout').onclick=()=>{sessionStorage.removeItem('aside_admin');token='';location.reload()};
$('#newbtn').onclick=issueForm;$('#bulkbtn').onclick=bulkForm;$('#exportbtn').onclick=exportCsv;$('#reload').onclick=load;
$('#q').oninput=()=>{clearTimeout(timer);timer=setTimeout(()=>{query=$('#q').value;load()},250)};
document.addEventListener('keydown',(e)=>{if(e.key==='Escape')close()});
if(token)load();else $('#token').focus();
</script></body></html>`;
