import { safeEqual } from "./crypto.ts";
import type { Env } from "./env.ts";
import { fail, json, issue, body, str } from "./handlers.ts";
import { keyHash, newKey } from "./licence.ts";
import { mailKey } from "./mail.ts";
import * as store from "./store.ts";

/** What the admin sees of a licence: everything except the hash of the key. */
const view = ({ key_hash: _hash, ...rest }: store.ListedLicence | NonNullable<Awaited<ReturnType<typeof store.byId>>>) => rest;

function authorised(req: Request, env: Env): boolean {
  const given = (req.headers.get("authorization") ?? "").replace(/^Bearer\s+/i, "");
  return env.ADMIN_TOKEN.length >= 16 && safeEqual(given, env.ADMIN_TOKEN);
}

/** `/admin/api/...`: the API of the admin page. Every call needs `Authorization: Bearer <ADMIN_TOKEN>`. */
export async function adminApi(req: Request, env: Env, now: number, path: string): Promise<Response> {
  if (!env.ADMIN_TOKEN || env.ADMIN_TOKEN.length < 16) return fail("admin_not_configured", 503);
  if (!authorised(req, env)) return fail("unauthorised", 401);
  const url = new URL(req.url);
  const parts = path.split("/").filter(Boolean); // after /admin/api
  if (req.method === "GET" && parts[0] === "overview") {
    return json({ ok: true, stats: await store.stats(env.DB, now), licences: (await store.list(env.DB, url.searchParams.get("q") ?? "")).map(view), events: await store.recentEvents(env.DB, 30) });
  }
  if (req.method === "GET" && parts[0] === "licence" && parts[1]) {
    const l = await store.byId(env.DB, parts[1]);
    if (!l) return fail("not_found", 404);
    return json({ ok: true, licence: view(l), activations: await store.activations(env.DB, l.id), events: await store.events(env.DB, l.id) });
  }
  if (req.method !== "POST") return fail("not_found", 404);
  const b = (await body(req)) ?? {};
  if (parts[0] === "issue") {
    const email = str(b.email).trim();
    const checkDays = Number(b.check_days) || 14;
    const maxDevices = Number(b.max_devices) || 2;
    const major = Number(b.major) || undefined;
    if (checkDays < 1 || checkDays > 365 || maxDevices < 1 || maxDevices > 20) return fail("bad_request", 400);
    const features = str(b.features).trim() || "all";
    const { row, key } = await issue(env, now, { email, orderRef: null, features, major, checkDays, maxDevices, note: str(b.note) });
    const mailed = b.send === true ? await mailKey(env, email, key, row.id) : false;
    return json({ ok: true, licence: view(row), key, mailed });
  }
  if (parts[0] === "licence" && parts[1] && parts[2]) {
    const l = await store.byId(env.DB, parts[1]);
    if (!l) return fail("not_found", 404);
    switch (parts[2]) {
      case "revoke":
        await store.setState(env.DB, l.id, "revoked");
        await store.logEvent(env.DB, now, l.id, "revoked", str(b.reason) || "by the owner");
        return json({ ok: true });
      case "restore":
        await store.setState(env.DB, l.id, "active");
        await store.logEvent(env.DB, now, l.id, "restored", "by the owner");
        return json({ ok: true });
      case "release":
        await store.release(env.DB, l.id, str(b.device), now);
        await store.logEvent(env.DB, now, l.id, "released", "by the owner");
        return json({ ok: true });
      case "check_days": {
        const days = Number(b.days);
        if (!(days >= 1 && days <= 365)) return fail("bad_request", 400);
        await store.setCheckDays(env.DB, l.id, Math.round(days));
        await store.logEvent(env.DB, now, l.id, "check-days", String(Math.round(days)));
        return json({ ok: true });
      }
      case "rekey": {
        // the old key stops working at once; the new one is shown once (and mailed when asked)
        const key = newKey();
        await store.setKeyHash(env.DB, l.id, await keyHash(key));
        await store.logEvent(env.DB, now, l.id, "rekeyed", "by the owner");
        const mailed = b.send === true ? await mailKey(env, l.email, key, l.id) : false;
        return json({ ok: true, key, mailed });
      }
    }
  }
  return fail("not_found", 404);
}

/** The admin page: one HTML file, no libraries; the token is typed in and kept only in this tab (sessionStorage). */
export const ADMIN_PAGE = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex"><title>Aside licences</title>
<style>
:root{color-scheme:light dark;--bg:#fff;--fg:#1a1a1a;--mute:#666;--line:#ddd;--card:#f6f6f6;--ok:#1a7f37;--warn:#9a6700;--bad:#cf222e}
@media (prefers-color-scheme:dark){:root{--bg:#161616;--fg:#eee;--mute:#999;--line:#333;--card:#202020;--ok:#3fb950;--warn:#d29922;--bad:#ff7b72}}
body{font:14px/1.5 system-ui,sans-serif;background:var(--bg);color:var(--fg);margin:0;padding:16px;max-width:1000px;margin-inline:auto}
h1{font-size:18px;margin:0 0 12px}input,select,button,textarea{font:inherit;color:inherit;background:var(--card);border:1px solid var(--line);border-radius:6px;padding:6px 10px}
button{cursor:pointer}button.bad{color:var(--bad)}table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:6px 8px;border-top:1px solid var(--line);vertical-align:top}
th{color:var(--mute);font-weight:500;border-top:0}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:8px;margin:12px 0}
.card{background:var(--card);border-radius:8px;padding:10px}.card b{display:block;font-size:22px;font-weight:500}.mute{color:var(--mute)}
.row{display:flex;gap:8px;flex-wrap:wrap;margin:8px 0}.pill{padding:1px 8px;border-radius:10px;font-size:12px;border:1px solid currentColor}.ok{color:var(--ok)}.warn{color:var(--warn)}.bad{color:var(--bad)}
tr.click{cursor:pointer}pre{background:var(--card);padding:8px;border-radius:6px;overflow:auto}.panel{background:var(--card);border-radius:8px;padding:12px;margin:12px 0}
</style></head><body>
<h1>Aside – licences</h1>
<div class="row"><input id="token" type="password" placeholder="Admin token" size="34"><button id="login">Sign in</button><span id="msg" class="mute"></span></div>
<div id="app" hidden>
<div class="cards" id="stats"></div>
<div class="row"><input id="q" placeholder="Search by id or e-mail" size="30"><button id="search">Search</button><button id="newbtn">Issue a licence</button></div>
<div class="panel" id="issue" hidden>
<div class="row"><input id="i_email" placeholder="E-mail of the buyer" size="28"><input id="i_features" placeholder="features (all)" size="16"><input id="i_days" type="number" placeholder="check days (14)" size="6"><input id="i_devices" type="number" placeholder="devices (2)" size="6"></div>
<div class="row"><input id="i_note" placeholder="Note" size="40"><label><input type="checkbox" id="i_send"> mail the key</label><button id="i_go">Create</button></div>
<pre id="i_out" hidden></pre></div>
<table id="list"><thead><tr><th>Id</th><th>E-mail</th><th>Version</th><th>Devices</th><th>Last check</th><th>State</th></tr></thead><tbody></tbody></table>
<div class="panel" id="detail" hidden></div>
<h1 style="margin-top:20px">Recent events</h1><table id="events"><tbody></tbody></table>
</div>
<script>
const $=(s)=>document.querySelector(s);let token=sessionStorage.getItem('aside_admin')||'';
const el=(t,txt,cls)=>{const e=document.createElement(t);if(txt!==undefined)e.textContent=txt;if(cls)e.className=cls;return e};
const when=(s)=>s?new Date(s*1000).toLocaleString():'-';
async function api(path,opt){const r=await fetch('/admin/api/'+path,{...opt,headers:{authorization:'Bearer '+token,'content-type':'application/json'}});const j=await r.json().catch(()=>({}));if(!r.ok)throw new Error(j.error||r.status);return j}
const post=(p,b)=>api(p,{method:'POST',body:JSON.stringify(b||{})});
function state(l){const s=el('span',l.state==='active'?'active':'revoked','pill '+(l.state==='active'?'ok':'bad'));return s}
async function load(){try{const j=await api('overview?q='+encodeURIComponent($('#q').value));$('#app').hidden=false;$('#msg').textContent='';
const st=$('#stats');st.replaceChildren();[['Active licences',j.stats.active],['Devices',j.stats.devices],['Overdue check',j.stats.overdue],['Revoked',j.stats.revoked]].forEach(([k,v])=>{const c=el('div','','card');c.append(el('span',k,'mute'),el('b',String(v)));st.append(c)});
const tb=$('#list tbody');tb.replaceChildren();j.licences.forEach(l=>{const tr=el('tr','','click');[l.id,l.email,l.major+'.x',l.devices+' / '+l.max_devices,when(l.last_seen)].forEach(t=>tr.append(el('td',t)));const td=el('td');td.append(state(l));tr.append(td);tr.onclick=()=>open(l.id);tb.append(tr)});
const ev=$('#events tbody');ev.replaceChildren();j.events.forEach(e=>{const tr=el('tr');[when(e.at),e.licence_id||'-',e.kind,e.detail].forEach(t=>tr.append(el('td',t)));ev.append(tr)})}catch(e){$('#msg').textContent='Error: '+e.message;if(e.message==='unauthorised'){sessionStorage.removeItem('aside_admin');$('#app').hidden=true}}}
async function open(id){const j=await api('licence/'+id);const d=$('#detail');d.hidden=false;d.replaceChildren();const l=j.licence;
d.append(el('b',l.id+' · '+(l.email||'no e-mail')),el('div','features: '+l.features+' · version '+l.major+'.x · check every '+l.check_days+' days · created '+when(l.created)+(l.note?' · '+l.note:''),'mute'));
const t=el('table');j.activations.forEach(a=>{const tr=el('tr');tr.append(el('td',a.device.slice(0,12)+'…'),el('td','since '+when(a.created)),el('td','last seen '+when(a.last_seen)));const td=el('td');if(a.released)td.append(el('span','released','mute'));else{const b=el('button','Free this device');b.onclick=async()=>{await post('licence/'+l.id+'/release',{device:a.device});open(l.id);load()};td.append(b)}tr.append(td);t.append(tr)});d.append(t);
const row=el('div','','row');const act=(label,fn,cls)=>{const b=el('button',label,cls);b.onclick=async()=>{await fn();open(l.id);load()};row.append(b)};
if(l.state==='active')act('Revoke',()=>post('licence/'+l.id+'/revoke',{}),'bad');else act('Restore',()=>post('licence/'+l.id+'/restore'));
act('Check interval…',async()=>{const v=prompt('Check every how many days?',l.check_days);if(v)await post('licence/'+l.id+'/check_days',{days:Number(v)})});
act('New key…',async()=>{if(!confirm('The old key stops working. Continue?'))return;const r=await post('licence/'+l.id+'/rekey',{send:confirm('Mail the new key to '+(l.email||'nobody')+'?')});alert('New key (shown once):\\n'+r.key+(r.mailed?'\\n\\nMailed.':''))});
d.append(row);const ev=el('table');j.events.forEach(e=>{const tr=el('tr');[when(e.at),e.kind,e.detail].forEach(x=>tr.append(el('td',x)));ev.append(tr)});d.append(ev)}
$('#login').onclick=()=>{token=$('#token').value.trim();sessionStorage.setItem('aside_admin',token);load()};
$('#search').onclick=load;$('#newbtn').onclick=()=>{$('#issue').hidden=!$('#issue').hidden};
$('#i_go').onclick=async()=>{try{const r=await post('issue',{email:$('#i_email').value,features:$('#i_features').value,check_days:Number($('#i_days').value)||undefined,max_devices:Number($('#i_devices').value)||undefined,note:$('#i_note').value,send:$('#i_send').checked});const o=$('#i_out');o.hidden=false;o.textContent='Licence '+r.licence.id+'\\nKey (shown once): '+r.key+(r.mailed?'\\nMailed to '+r.licence.email:'');load()}catch(e){alert(e.message)}};
if(token){$('#token').value=token;load()}
</script></body></html>`;
