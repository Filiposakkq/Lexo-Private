/* The live parts of the styleguide: the swatches and the contrast table are built from the real values of tokens.css. (An external file: the server forbids inline scripts.) */
(function () {
  var css = getComputedStyle(document.documentElement);
  var v = function (n) { return css.getPropertyValue(n).trim(); };
  function lum(h) { h = h.replace('#', ''); if (h.length === 3) h = h.split('').map(function (c) { return c + c; }).join(''); var a = [0, 2, 4].map(function (i) { var c = parseInt(h.substr(i, 2), 16) / 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); }); return 0.2126 * a[0] + 0.7152 * a[1] + 0.0722 * a[2]; }
  function ratio(a, b) { var x = lum(a), y = lum(b); if (x < y) { var t = x; x = y; y = t; } return (x + 0.05) / (y + 0.05); }
  var groups = [['Povrchy', ['--bg-app', '--bg-rail', '--bg-bar', '--bg-card', '--bg-hover', '--bg-code']], ['Čáry', ['--line', '--line-soft', '--line-accent']], ['Text', ['--text', '--text-2', '--text-muted', '--text-faint', '--text-disabled']],
    ['Akcent', ['--accent', '--accent-hover', '--accent-hi', '--accent-soft', '--accent-text']], ['Stavy', ['--todo', '--todo-bg', '--checked', '--checked-bg', '--danger', '--danger-bg', '--track']]];
  var sw = document.getElementById('swatches');
  groups.forEach(function (g) { g[1].forEach(function (n) { var val = v(n); sw.insertAdjacentHTML('beforeend', '<div class="sw"><div class="c" style="background:' + val + '"></div><div class="t"><b>' + n + '</b><span>' + val + ' · ' + g[0] + '</span></div></div>'); }); });
  var pairs = [['--text', '--bg-app', 4.5, 'text'], ['--text-2', '--bg-app', 4.5, 'text'], ['--text-muted', '--bg-app', 4.5, 'text'], ['--text-muted', '--bg-card', 4.5, 'text na kartě'], ['--text-faint', '--bg-card', 4.5, 'popisek na kartě'], ['--text-faint', '--bg-app', 4.5, 'popisek'],
    ['--accent-text', '--bg-app', 4.5, 'odkaz'], ['--accent-text', '--accent-soft', 4.5, 'aktivní čip'], ['--on-accent', '--accent', 4.5, 'hlavní tlačítko'], ['--accent-hi', '--bg-card', 3, 'proužek, ovládací prvek'], ['--todo', '--todo-bg', 4.5, 'stav k překladu'], ['--checked', '--checked-bg', 4.5, 'stav zkontrolováno'], ['--danger', '--danger-bg', 4.5, 'chyba'], ['--text-disabled', '--bg-card', 3, 'vypnuté (jen ozdoba)']];
  var tb = document.querySelector('#contrast tbody');
  pairs.forEach(function (p) { var r = ratio(v(p[0]), v(p[1])), ok = r >= p[2]; tb.insertAdjacentHTML('beforeend', '<tr><td>' + p[0] + ' na ' + p[1] + ' <span class="faint">(' + p[3] + ')</span></td><td><span style="background:' + v(p[1]) + ';color:' + v(p[0]) + ';padding:2px 8px;border-radius:6px">Ukázka textu</span></td><td>' + (Math.round(r * 100) / 100).toFixed(2).replace('.', ',') + ' : 1</td><td class="' + (ok ? 'pass' : 'fail') + '">' + (ok ? 'vyhovuje' : 'nevyhovuje (min. ' + String(p[2]).replace('.', ',') + ')') + '</td></tr>'); });
})();
