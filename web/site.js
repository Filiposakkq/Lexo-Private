/* Aside website: the parts every sub-page shares (index.html has its own copy of the menu).
   The mobile menu is built from the header links, the header gets its line on scroll, and cards marked .spot
   follow the pointer with a soft light. Counters (.counter[data-to]) count up when they come into view. */
(function () {
  "use strict";
  document.documentElement.classList.add("js");
  var top = document.getElementById("top");
  if (top) window.addEventListener("scroll", function () { top.classList.toggle("scrolled", (window.scrollY || 0) > 8); }, { passive: true });

  if (top && !document.getElementById("sheet")) {
    var links = Array.prototype.slice.call(top.querySelectorAll(".nav a"));
    var btn = document.createElement("button");
    btn.className = "menu-btn"; btn.setAttribute("aria-label", "Open menu"); btn.setAttribute("aria-expanded", "false");
    btn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M4 7h16M4 12h16M4 17h10" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>';
    top.querySelector(".top-cta").appendChild(btn);
    var sheet = document.createElement("div");
    sheet.className = "sheet"; sheet.id = "sheet"; sheet.hidden = true; sheet.setAttribute("role", "dialog"); sheet.setAttribute("aria-modal", "true"); sheet.setAttribute("aria-label", "Menu");
    sheet.innerHTML = '<div class="sheet-head"><span class="brand">' + top.querySelector(".brand svg").outerHTML + 'aside</span><button class="menu-btn" style="display:inline-flex" aria-label="Close menu"><svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6 6 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg></button></div>' +
      "<nav aria-label=\"Menu\">" + links.map(function (a) { return '<a href="' + a.getAttribute("href") + '">' + a.textContent + ' <svg viewBox="0 0 24 24"><path d="M5 12h14m-6-6 6 6-6 6" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg></a>'; }).join("") + "</nav>" +
      '<div class="btns"><a class="btn btn-hot" href="./#beta">Join the beta</a><a class="btn" href="https://ko-fi.com/filiposakkq" target="_blank" rel="noopener">Support on Ko-fi</a></div>';
    document.body.appendChild(sheet);
    var set = function (open) { sheet.hidden = !open; btn.setAttribute("aria-expanded", String(open)); document.body.style.overflow = open ? "hidden" : ""; };
    btn.addEventListener("click", function () { set(true); });
    sheet.querySelector(".sheet-head .menu-btn").addEventListener("click", function () { set(false); });
    sheet.querySelectorAll("nav a, .btns a").forEach(function (a) { a.addEventListener("click", function () { set(false); }); });
    document.addEventListener("keydown", function (e) { if (e.key === "Escape") set(false); });
  }

  document.addEventListener("pointermove", function (e) {
    var c = e.target.closest && e.target.closest(".spot"); if (!c) return;
    var r = c.getBoundingClientRect(); c.style.setProperty("--mx", (e.clientX - r.left) + "px"); c.style.setProperty("--my", (e.clientY - r.top) + "px");
  });

  var reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  var fmtN = function (n) { return String(Math.round(n)).replace(/\B(?=(\d{3})+(?!\d))/g, " "); };
  function fmt(n, f) { if (f === "M") return (n / 1e6).toFixed(1) + " M"; if (f === "k") return Math.round(n / 1000) + " k"; return fmtN(n); }
  if ("IntersectionObserver" in window && !reduce) {
    var io = new IntersectionObserver(function (ents) {
      ents.forEach(function (en) {
        if (!en.isIntersecting) return;
        var el = en.target, to = +el.getAttribute("data-to"), f = el.getAttribute("data-fmt"), t0 = null;
        (function step(t) { if (!t0) t0 = t || 0; var p = Math.min(1, ((t || 0) - t0) / 1400); el.textContent = fmt(to * (1 - Math.pow(1 - p, 4)), f); if (p < 1) requestAnimationFrame(step); })();
        io.unobserve(el);
      });
    }, { threshold: .4 });
    document.querySelectorAll(".counter[data-to]").forEach(function (el) { if (!el.closest("[data-own-counters]")) io.observe(el); });
  }
})();
