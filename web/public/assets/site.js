/* Aside website: the parts every page shares.
   - AsideAPI: the calls to the website's server (/api/...), with Turnstile for forms; when there is no server
     (a page opened from disk, a preview), `AsideAPI.live()` resolves to false and the pages fall back to e-mail
   - the menu (☰): every page of the site in four groups, the same everywhere
   - the bottom bar on phones: Home, Features, Beta, Docs and the menu
   - the reading line under the header and the "back to top" button
   - the header line on scroll, the light that follows the pointer on .spot cards,
     and counters (.counter[data-to]) that count up when seen (unless the page has its own: body[data-own-counters]) */
/* the server of the website (web/worker): one config call decides whether the pages talk to it */
window.AsideAPI = (function () {
  var cfg = null;
  function call(path, body) {
    return fetch("/api/" + path, body === undefined ? { headers: { accept: "application/json" } } : { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) })
      .then(function (r) { return r.json().catch(function () { return { ok: false, error: "bad_answer" }; }).then(function (j) { j.status = r.status; return j; }); });
  }
  function config() {
    if (!cfg) cfg = (location.protocol === "file:" ? Promise.reject(new Error("no server")) : call("config"))
      .then(function (j) { if (!j.ok) throw new Error("no server"); return j; })
      .catch(function () { return null; });
    return cfg;
  }
  var tsLoading = null;
  /* Renders the Turnstile check into `box` when the server has a site key; resolves to a function giving the current token. */
  function human(box) {
    return config().then(function (c) {
      if (!c || !c.turnstile || !box) return function () { return ""; };
      if (!tsLoading) tsLoading = new Promise(function (ok, no) {
        var s = document.createElement("script"); s.src = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit"; s.async = true;
        s.onload = function () { ok(); }; s.onerror = no; document.head.appendChild(s);
      });
      return tsLoading.then(function () {
        var token = "", id = window.turnstile.render(box, { sitekey: c.turnstile, theme: "dark", callback: function (t) { token = t; }, "expired-callback": function () { token = ""; } });
        return function () { return token; };
      }).catch(function () { return function () { return ""; }; });
    });
  }
  /* A friendly sentence for an error answer of the server. */
  function why(j) {
    var e = j && j.error;
    return e === "too_many" ? "Too many tries from this address. Please wait a while and try again." :
      e === "not_human" ? "The anti-bot check did not pass. Please tick it and send again." :
      e === "already" ? "There is already an application from this e-mail. If you have not confirmed it yet, look for the confirmation e-mail (also in spam); it can be sent again after 48 hours." :
      e === "busy" ? "Many applications arrived today. Please try again tomorrow." :
      e === "paused" ? "Personal pages are paused for a while. Please try again later." :
      e === "closed" ? "Applications are closed right now." :
      e === "missing" ? "Something required is missing" + (j.field ? " (" + j.field + ")" : "") + "." :
      e === "image_too_big" ? "The screenshot is too big. Try a smaller one." :
      "Sending failed. Please try again, or write to aside@filiposakkq.dev.";
  }
  /* Text written in the admin (blog, FAQ, status) to safe HTML: everything is escaped first, then a few marks are turned into tags:
     an empty line between paragraphs, "### " headings, "- " lists, ``` code blocks, `code`, **bold**, [text](https://…).
     `inline` gives one paragraph without <p> (FAQ answers). */
  function md(src, inline) {
    var esc = function (t) { return String(t).replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; }); };
    var span = function (t) {
      return esc(t).replace(/`([^`]+)`/g, "<code>$1</code>").replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
        .replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+|[a-z0-9-]+\.html(?:#[\w-]+)?)\)/gi, function (m, t, u) { return '<a href="' + u + '"' + (/^https?:/.test(u) ? ' target="_blank" rel="noopener"' : "") + ">" + t + "</a>"; });
    };
    if (inline) return span(src).replace(/\n/g, "<br>");
    var out = [];
    String(src || "").replace(/\r/g, "").split(/```/).forEach(function (part, i) {
      if (i % 2) { out.push("<pre>" + esc(part.replace(/^\w*\n/, "").replace(/\n$/, "")) + "</pre>"); return; }
      part.split(/\n\s*\n/).forEach(function (b) {
        b = b.trim(); if (!b) return;
        if (/^### /.test(b)) out.push("<h3>" + span(b.slice(4)) + "</h3>");
        else if (/^- /.test(b)) out.push("<ul>" + b.split(/\n/).map(function (l) { return "<li>" + span(l.replace(/^- /, "")) + "</li>"; }).join("") + "</ul>");
        else out.push("<p>" + span(b).replace(/\n/g, "<br>") + "</p>");
      });
    });
    return out.join("");
  }
  /* The "Now" strip (written in the admin, Status): the current build, the next one, a note and live numbers. Stays hidden when off. */
  function now(el) {
    if (!el) return;
    config().then(function (c) { return c ? call("status") : null; }).then(function (j) {
      if (!j || !j.on) return;
      var n = j.numbers, e = function (t) { return md(t, true); };
      el.innerHTML = '<span class="now-dot" aria-hidden="true"></span><div class="now-main"><b>Now</b>' + (j.build ? " · testing <code>" + e(j.build) + "</code>" : "") + (j.next ? " · next build " + e(j.next) : "") + (j.note ? '<span class="now-note">' + e(j.note) + "</span>" : "") +
        '</div><div class="now-nums"><span><b>' + n.testers + "</b> testers</span><span><b>" + n.fixed + "</b> bugs fixed</span><span><b>" + n.open + "</b> open</span></div>";
      el.hidden = false;
    });
  }
  return { call: call, config: config, live: function () { return config().then(function (c) { return !!c; }); }, human: human, why: why, md: md, now: now };
})();

(function () {
  "use strict";
  document.documentElement.classList.add("js");
  var home = document.body.hasAttribute("data-home");
  var here = (location.pathname.split("/").pop() || "index.html").toLowerCase();
  var local = function (href) { return home && href.indexOf("./#") === 0 ? href.slice(2) : href; };
  var arrow = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14m-6-6 6 6-6 6" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>';
  var MENU = [
    ["Product", [["./#tour", "Features"], ["./#try", "Try it"], ["./#pricing", "Pricing"], ["compare.html", "Compare"]]],
    ["Project", [["roadmap.html", "Roadmap"], ["releases.html", "Releases"], ["showcase.html", "Showcase"], ["blog.html", "Blog"], ["news.html", "News by e-mail"]]],
    ["Get involved", [["beta.html", "Beta programs"], ["developers.html", "For game developers"]]],
    ["Help", [["docs.html", "Documentation"], ["./#faq", "FAQ"], ["contact.html", "Contact"]]]
  ];

  var top = document.getElementById("top");
  if (top) window.addEventListener("scroll", function () { top.classList.toggle("scrolled", (window.scrollY || 0) > 8); }, { passive: true });

  /* the menu */
  var sheet = document.createElement("div");
  sheet.className = "sheet"; sheet.id = "sheet"; sheet.hidden = true;
  sheet.setAttribute("role", "dialog"); sheet.setAttribute("aria-modal", "true"); sheet.setAttribute("aria-label", "Menu");
  var logo = top ? top.querySelector(".brand svg").outerHTML : "";
  sheet.innerHTML = '<div class="sheet-head"><a class="brand" href="' + (home ? "#main" : "./") + '">' + logo + 'aside</a><button class="menu-btn" type="button" style="display:inline-flex" aria-label="Close menu"><svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6 6 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg></button></div>' +
    '<div class="sheet-groups">' + MENU.map(function (g) {
      return '<nav class="sheet-group" aria-label="' + g[0] + '"><h2>' + g[0] + "</h2>" + g[1].map(function (l) {
        var cur = l[0] === here || (here === "index.html" && false);
        return '<a href="' + local(l[0]) + '"' + (cur ? ' aria-current="page"' : "") + ">" + l[1] + arrow + "</a>";
      }).join("") + "</nav>";
    }).join("") + "</div>" +
    '<div class="sheet-foot"><a class="btn btn-hot" href="beta.html">Join the beta</a><a class="btn" href="https://ko-fi.com/filiposakkq" target="_blank" rel="noopener">Support on Ko-fi</a>' +
    '<p><a href="mailto:aside@filiposakkq.dev">aside@filiposakkq.dev</a> · <a href="https://filiposakkq.dev" target="_blank" rel="noopener">filiposakkq.dev</a></p></div>';
  document.body.appendChild(sheet);
  var opener = null;
  function setSheet(open) {
    sheet.hidden = !open; document.body.style.overflow = open ? "hidden" : "";
    document.querySelectorAll("[data-menu]").forEach(function (b) { b.setAttribute("aria-expanded", String(open)); });
    if (open) sheet.querySelector(".sheet-head .menu-btn").focus(); else if (opener) opener.focus();
  }
  if (top && !top.querySelector("[data-menu]")) {
    var btn = document.createElement("button");
    btn.className = "menu-btn"; btn.type = "button"; btn.setAttribute("data-menu", ""); btn.setAttribute("aria-label", "Open menu"); btn.setAttribute("aria-expanded", "false");
    btn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h10" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>';
    top.querySelector(".top-cta").appendChild(btn);
  }
  document.addEventListener("click", function (e) {
    var m = e.target.closest && e.target.closest("[data-menu]");
    if (m) { opener = m; setSheet(true); return; }
    if (e.target.closest && e.target.closest("#sheet a")) setSheet(false);
  });
  sheet.querySelector(".sheet-head .menu-btn").addEventListener("click", function () { setSheet(false); });
  document.addEventListener("keydown", function (e) { if (e.key === "Escape" && !sheet.hidden) setSheet(false); });

  /* the bottom bar on phones */
  var I = {
    home: '<path d="M4 11 12 4l8 7v9h-5v-6H9v6H4z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>',
    feat: '<path d="M4 5h7v7H4zM13 5h7v7h-7zM4 14h7v6H4zM13 14h7v6h-7z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>',
    beta: '<path d="M12 3l2.4 5.3 5.6.6-4.2 3.8 1.2 5.6L12 15.5 7 18.3l1.2-5.6L4 8.9l5.6-.6z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>',
    docs: '<path d="M6 3h9l4 4v14H6zM14 3v5h5M9 13h7M9 17h5" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>',
    menu: '<path d="M4 7h16M4 12h16M4 17h10" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>'
  };
  var tab = function (href, label, icon, cur) { return '<a href="' + href + '"' + (cur ? ' aria-current="page"' : "") + '><svg viewBox="0 0 24 24" aria-hidden="true">' + I[icon] + "</svg><span>" + label + "</span></a>"; };
  var bar = document.createElement("nav");
  bar.className = "tabbar"; bar.setAttribute("aria-label", "Quick links");
  bar.innerHTML = tab(home ? "#main" : "./", "Home", "home", home) + tab(local("./#tour"), "Features", "feat", false) + tab("beta.html", "Beta", "beta", here === "beta.html") + tab("docs.html", "Docs", "docs", here === "docs.html") +
    '<button type="button" data-menu aria-expanded="false" aria-label="Open menu"><svg viewBox="0 0 24 24" aria-hidden="true">' + I.menu + "</svg><span>Menu</span></button>";
  document.body.appendChild(bar);
  document.body.classList.add("has-tabbar");

  /* reading line and back to top */
  var line = document.createElement("div"); line.className = "readline"; line.setAttribute("aria-hidden", "true");
  if (top) top.appendChild(line);
  var up = document.createElement("button");
  up.className = "to-top"; up.type = "button"; up.setAttribute("aria-label", "Back to top"); up.hidden = true;
  up.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 19V5m-6 6 6-6 6 6" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>';
  up.addEventListener("click", function () { window.scrollTo({ top: 0, behavior: "smooth" }); });
  document.body.appendChild(up);
  var ticking = false;
  function onScroll() {
    ticking = false;
    var y = window.scrollY || 0, max = document.documentElement.scrollHeight - window.innerHeight;
    line.style.transform = "scaleX(" + (max > 0 ? Math.min(1, y / max) : 0) + ")";
    up.hidden = y < 700;
  }
  window.addEventListener("scroll", function () { if (!ticking) { ticking = true; requestAnimationFrame(onScroll); } }, { passive: true });
  window.addEventListener("resize", onScroll);
  onScroll();

  /* light under the pointer */
  document.addEventListener("pointermove", function (e) {
    var c = e.target.closest && e.target.closest(".spot"); if (!c) return;
    var r = c.getBoundingClientRect(); c.style.setProperty("--mx", (e.clientX - r.left) + "px"); c.style.setProperty("--my", (e.clientY - r.top) + "px");
  });

  /* counters */
  var reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (document.body.hasAttribute("data-own-counters") || reduce || !("IntersectionObserver" in window)) return;
  var fmtN = function (n) { return String(Math.round(n)).replace(/\B(?=(\d{3})+(?!\d))/g, " "); };
  function fmt(n, f) { if (f === "M") return (n / 1e6).toFixed(1) + " M"; if (f === "k") return Math.round(n / 1000) + " k"; return fmtN(n); }
  var io = new IntersectionObserver(function (ents) {
    ents.forEach(function (en) {
      if (!en.isIntersecting) return;
      var el = en.target, to = +el.getAttribute("data-to"), f = el.getAttribute("data-fmt"), t0 = null;
      (function step(t) { if (!t0) t0 = t || 0; var p = Math.min(1, ((t || 0) - t0) / 1400); el.textContent = fmt(to * (1 - Math.pow(1 - p, 4)), f); if (p < 1) requestAnimationFrame(step); })();
      io.unobserve(el);
    });
  }, { threshold: .4 });
  document.querySelectorAll(".counter[data-to]").forEach(function (el) { io.observe(el); });
})();
