/* Harneo prototype: state, routing, frame, overlays, keyboard. Screens register in H.screens. */
(function () {
  "use strict";
  const D = window.HARNEO_DATA;
  const H = (window.H = { screens: {}, actions: {}, inputs: {} });

  // ---------------------------------------------------------------- state
  const clone = (v) => JSON.parse(JSON.stringify(v));
  H.fresh = function (firstRun) {
    const S = {
      firstRun: !!firstRun,
      artifacts: firstRun ? [] : clone(D.ARTIFACTS),
      profiles: firstRun ? [] : clone(D.PROFILES),
      projects: firstRun ? [] : clone(D.PROJECTS),
      collections: firstRun ? [] : clone(D.COLLECTIONS),
      tags: clone(D.TAGS),
      f: { types: [], tags: [], status: [], sources: [], trustMin: 0, inv: [], q: "" },
      view: "cards",
      sort: "used",
      selected: null,
      claudeCli: true,
      ui: { modal: null, palette: null, menu: null, filtersOpen: false, enter: null },
      flow: {},
    };
    return S;
  };
  H.S = H.fresh(false);
  let themePref = "system";
  try { themePref = localStorage.getItem("harneo-theme") || "system"; } catch (e) { /* storage blocked */ }

  // -------------------------------------------------------------- helpers
  const esc = (s) => String(s == null ? "" : s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
  H.esc = esc;
  H.icon = (n, cls) => `<i data-lucide="${n}"${cls ? ` class="${cls}"` : ""}></i>`;
  H.T = D.TYPES;
  H.SRC = D.SOURCES;
  H.art = (id) => H.S.artifacts.find((a) => a.id === id);
  H.typeIc = (type, sm, gray) => `<span class="ic${sm ? " sm" : ""} ${gray ? "t-gray" : "t-" + D.TYPES[type].tint}">${H.icon(D.TYPES[type].icon)}</span>`;
  H.tagColor = (id) => (H.S.tags.find((t) => t.id === id) || { color: "slate" }).color;
  H.tagChip = (id, removable) => `<span class="tag t-${H.tagColor(id)}"><i class="d"></i>${esc(id)}${removable ? `<button data-a="tag-remove" data-tag="${esc(id)}" aria-label="Remove tag ${esc(id)}">${H.icon("x")}</button>` : ""}</span>`;
  H.usageTypes = ["skill", "subagent", "command", "mcp", "workflow", "plugin", "rule"];
  H.status = (a) => {
    if (a.findings.length && a.trust < 60) return "findings";
    if (a.latest) return "outdated";
    if (H.usageTypes.includes(a.type) && a.sessions === 0 && !a.fresh) return "unused";
    return "current";
  };
  H.statusBadge = (a) => {
    const s = H.status(a);
    if (s === "findings") return `<span class="badge bad">${H.plural(a.findings.length, "finding")}</span>`;
    if (s === "outdated") return `<span class="badge warn">Update ${esc(a.latest)}</span>`;
    if (s === "unused") return `<span class="badge">Unused</span>`;
    const inv = a.invocation ? (a.invocation === "auto" ? "Auto" : "Manual") : D.TYPES[a.type].label;
    if (a.fresh && !a.sessions) return `<span class="badge info">New</span>`;
    return `<span class="badge">${a.bundle.length ? "Bundle of " + (a.bundle.length + 1) : inv}</span>`;
  };
  H.srcLabel = (a) => {
    const s = D.SOURCES[a.source];
    return s.kind === "local" ? "Local · " + s.url : s.label + " · " + (a.latest ? s.version + " → " + a.latest : s.version);
  };
  H.meter = (t, gray) => `<span class="trust"><i class="meter${gray ? " gray" : t < 60 ? " bad" : t < 80 ? " mid" : ""}"><i style="width:${t}%"></i></i>${t}</span>`;
  H.spark = (u) => { const max = Math.max(1, ...u); return `<span class="spark" aria-hidden="true">${u.map((n) => `<i style="height:${Math.round((n / max) * 100)}%"></i>`).join("")}</span>`; };
  H.plural = (n, w, p) => n + " " + (n === 1 ? w : p || w + "s");
  H.profile = (id) => H.S.profiles.find((p) => p.id === id);
  H.project = (id) => H.S.projects.find((p) => p.id === id);
  H.uid = (p) => p + "-" + Math.random().toString(36).slice(2, 8);

  // --------------------------------------------------------------- router
  H.route = () => {
    const h = (location.hash || "#/library").slice(2).split("/");
    return { name: h[0] || "library", id: h[1] ? decodeURIComponent(h[1]) : null, sub: h[2] || null };
  };
  H.go = (hash) => { if (location.hash === hash) H.render(); else location.hash = hash; };
  window.addEventListener("hashchange", () => { H.S.selected = null; H.S.ui.menu = null; H.S.ui.filtersOpen = false; H.render(); document.getElementById("view") && (document.getElementById("view").scrollTop = 0); });

  // ---------------------------------------------------------------- shell
  const navItem = (hash, ic, label, n, on) => `<a class="nav${on ? " on" : ""}" href="${hash}" title="${esc(label)}">${H.icon(ic)}<span class="lbl">${esc(label)}</span>${n != null ? `<span class="n">${n}</span>` : ""}</a>`;
  H.logo = (size) => `<svg width="${size}" height="${size}" viewBox="0 0 24 24" aria-hidden="true"><rect width="24" height="24" rx="6" fill="var(--blue)"/><path d="M7 6v12M17 6v12" stroke="#fff" stroke-width="2.2" stroke-linecap="round"/><path d="M7 12h4.2" stroke="#fff" stroke-width="2.2" stroke-linecap="round"/><circle cx="14" cy="12" r="2.1" fill="#fff"/></svg>`;
  function sidebar(r) {
    const S = H.S;
    const cols = S.collections.map((c) => navItem("#/collection/" + c.id, c.kind === "smart" ? "list-filter" : "bookmark", c.name, H.collectionItems(c).length, r.name === "collection" && r.id === c.id).replace('class="nav', 'class="nav col').replace("<span class=\"lbl\">", `<span class="cinit" aria-hidden="true">${esc(c.name.charAt(0).toUpperCase())}</span><span class="lbl">`)).join("");
    return `<nav class="side" aria-label="Main">
      <a class="brand" href="#/library">${H.logo(24)}<span>Harneo</span></a>
      ${navItem("#/library", "layout-grid", "Library", S.artifacts.length, r.name === "library")}
      ${navItem("#/profiles", "layers", "Profiles", S.profiles.length, r.name === "profiles")}
      ${navItem("#/projects", "folder-git-2", "Projects", S.projects.length, r.name === "projects")}
      <h6><span>Collections</span><button data-a="collection-new" aria-label="New collection" title="New collection">${H.icon("plus")}</button></h6>
      ${cols || `<span class="muted" style="padding:2px 8px;font-size:12px">None yet</span>`}
      <h6><span>Actions</span></h6>
      ${navItem("#/import", "download", "Import", null, r.name === "import")}
      ${navItem("#/scaffold", "folder-plus", "Scaffold project", null, r.name === "scaffold")}
      ${navItem("#/refit", "refresh-cw", "Refit project", null, r.name === "refit")}
      ${navItem("#/cleanup", "sparkles", "Cleanup", null, r.name === "cleanup")}
      <div class="side-foot">
        <button class="kbdbar" data-a="palette-open" title="Search and commands (Ctrl K)">${H.icon("search")}<span class="lbl">Search</span><kbd>Ctrl K</kbd></button>
        ${navItem("#/settings", "settings", "Settings", null, r.name === "settings")}
        <div class="demo" title="Prototype controls"><span class="lbl">Prototype</span>
          <button data-a="theme-cycle" title="Theme: ${themePref}">${H.icon(H.isDark() ? "moon" : "sun")}</button>
          <button data-a="demo-firstrun" title="Start as a first-time user">${H.icon("user-plus")}</button>
          <button data-a="demo-reset" title="Reset demo data">${H.icon("rotate-ccw")}</button>
        </div>
      </div>
    </nav>`;
  }

  // -------------------------------------------------------------- render
  function captureFocus() {
    const el = document.activeElement;
    if (!el || !el.id) return null;
    return { id: el.id, s: el.selectionStart, e: el.selectionEnd };
  }
  function restoreFocus(f) {
    if (!f) return;
    const el = document.getElementById(f.id);
    if (!el) return;
    el.focus({ preventScroll: true });
    if (f.s != null && el.setSelectionRange) try { el.setSelectionRange(f.s, f.e); } catch (e) { /* not a text input */ }
  }
  const keepScroll = ["view", "dbody", "codeview", "treeview", "palres", "mbody"];
  H.render = function () {
    const S = H.S, r = H.route();
    const focus = captureFocus();
    const scroll = {};
    keepScroll.forEach((id) => { const el = document.getElementById(id); if (el) scroll[id] = el.scrollTop; });
    applyTheme();
    let html;
    if ((S.firstRun && r.name !== "import") || r.name === "welcome") {
      html = H.screens.welcome();
    } else {
      const screen = H.screens[r.name] || H.screens.library;
      html = `<div class="app">${sidebar(r)}<main class="view" id="view">${screen(r)}</main></div>`;
    }
    html += H.overlays();
    document.getElementById("root").innerHTML = html;
    keepScroll.forEach((id) => { const el = document.getElementById(id); if (el && scroll[id] != null) el.scrollTop = scroll[id]; });
    if (window.lucide) window.lucide.createIcons();
    restoreFocus(focus);
    if (S.ui.focus) { const el = document.getElementById(S.ui.focus); S.ui.focus = null; if (el) el.focus(); }
    if (S.ui.modalFocus) { S.ui.modalFocus = false; const b = document.querySelector(".modal [autofocus]") || document.querySelector(".modal .btn.primary, .modal .btn.danger-solid") || document.querySelector(".modal .btn"); if (b) b.focus(); }
    S.ui.enter = null;
  };
  H.enter = (key) => (H.S.ui.enter === key ? " enter" : "");

  // ------------------------------------------------------------ overlays
  H.overlays = function () {
    const S = H.S;
    let o = "";
    if (S.ui.filtersOpen) o += `<div class="scrim" data-a="filters-close"></div>`;
    if (S.selected && H.screens.drawer) o += `<div class="scrim${H.enter("drawer")}" data-a="drawer-close"></div>` + H.screens.drawer(S.selected);
    if (S.ui.modal) o += `<div class="scrim top${H.enter("modal")}" data-a="modal-close"></div>` + H.modalHtml(S.ui.modal);
    if (S.ui.palette) o += `<div class="scrim pal-s${H.enter("palette")}" data-a="palette-close"></div>` + palette();
    return o;
  };
  // Modals: { title, body, foot, wide }  (body/foot are html strings or functions)
  // Inside a click the dispatcher renders once afterwards; elsewhere (keyboard) render now.
  H.later = () => { if (!H._dispatching) H.render(); };
  H.modal = (m) => { H.S.ui.modal = m; H.S.ui.enter = "modal"; H.S.ui.modalFocus = true; H.later(); };
  H.modalHtml = (m) => {
    const body = typeof m.body === "function" ? m.body() : m.body;
    const foot = typeof m.foot === "function" ? m.foot() : m.foot;
    return `<div class="modal${m.wide ? " wide" : ""}${H.enter("modal")}" role="dialog" aria-modal="true" aria-label="${esc(m.title)}">
      <div class="mh"><div style="flex:1"><h2>${esc(m.title)}</h2>${m.sub ? `<p class="sec" style="margin-top:4px">${m.sub}</p>` : ""}</div><button class="btn ghost sm" data-a="modal-close" aria-label="Close">${H.icon("x")}</button></div>
      <div class="mb" id="mbody">${body}</div>
      ${foot ? `<div class="mf">${foot}</div>` : ""}</div>`;
  };
  H.closeModal = () => { H.S.ui.modal = null; H.render(); };
  H.confirm = (o) => H.modal({
    title: o.title, sub: o.sub,
    body: o.body || "",
    foot: `<button class="btn" data-a="modal-close">Cancel</button><button class="btn ${o.danger ? "danger-solid" : "primary"}" data-a="confirm-ok">${esc(o.ok)}</button>`,
    onOk: o.onOk,
  });
  H.actions["confirm-ok"] = () => { const m = H.S.ui.modal; H.S.ui.modal = null; m && m.onOk && m.onOk(); H.render(); };
  H.actions["modal-close"] = () => H.closeModal();

  // --------------------------------------------------------------- toasts
  H.toast = function (msg, opts) {
    opts = opts || {};
    const box = document.getElementById("toasts");
    const el = document.createElement("div");
    el.className = "toast";
    el.setAttribute("role", "status");
    el.innerHTML = H.icon(opts.icon || "check") + `<span>${msg}</span>` + (opts.undo ? `<button>Undo</button>` : "");
    box.appendChild(el);
    if (window.lucide) window.lucide.createIcons();
    if (opts.undo) el.querySelector("button").onclick = () => { opts.undo(); el.remove(); H.render(); };
    setTimeout(() => el.remove(), opts.undo ? 7000 : 4200);
  };

  // -------------------------------------------------------------- palette
  function palItems(q) {
    const S = H.S;
    const items = [
      ["Places", "layout-grid", "Library", "#/library"], ["Places", "layers", "Profiles", "#/profiles"], ["Places", "folder-git-2", "Projects", "#/projects"], ["Places", "sparkles", "Cleanup", "#/cleanup"], ["Places", "settings", "Settings", "#/settings"],
      ["Actions", "download", "Import artifacts", "#/import"], ["Actions", "folder-plus", "Scaffold a new project", "#/scaffold"], ["Actions", "refresh-cw", "Refit an existing project", "#/refit"], ["Actions", "plus", "New profile", "act:profile-new"], ["Actions", "bookmark-plus", "New collection", "act:collection-new"], ["Actions", "sun-moon", "Toggle theme", "act:theme-cycle"], ["Actions", "keyboard", "Keyboard shortcuts", "act:shortcuts"],
    ];
    S.collections.forEach((c) => items.push(["Collections", c.kind === "smart" ? "list-filter" : "bookmark", c.name, "#/collection/" + c.id]));
    S.profiles.forEach((p) => items.push(["Profiles", "layers", p.name + " v" + p.version, "#/profiles/" + p.id]));
    S.projects.forEach((p) => items.push(["Projects", "folder-git-2", p.name, "#/projects/" + p.id, p.path]));
    const ql = q.trim().toLowerCase();
    let out = items.filter((i) => !ql || i[2].toLowerCase().includes(ql));
    if (ql) S.artifacts.filter((a) => a.name.toLowerCase().includes(ql)).slice(0, 8).forEach((a) => out.push(["Artifacts", D.TYPES[a.type].icon, a.name, "art:" + a.id, D.TYPES[a.type].label]));
    return out.slice(0, 40);
  }
  function palette() {
    const p = H.S.ui.palette;
    const items = palItems(p.q);
    p.count = items.length;
    if (p.i >= items.length) p.i = Math.max(0, items.length - 1);
    let g = null;
    const res = items.map((it, i) => {
      const head = it[0] !== g ? `<div class="grp">${(g = it[0])}</div>` : "";
      return head + `<button class="it${i === p.i ? " act" : ""}" data-a="palette-go" data-to="${esc(it[3])}" data-i="${i}">${H.icon(it[1])}<span>${esc(it[2])}</span>${it[4] ? `<span class="muted">${esc(it[4])}</span>` : ""}</button>`;
    }).join("");
    return `<div class="pal${H.enter("palette")}" role="dialog" aria-label="Search and commands">
      <div class="pin-in">${H.icon("search")}<input id="palq" placeholder="Search places, actions, artifacts…" value="${esc(p.q)}" data-in="palette-q" autocomplete="off" aria-label="Search"></div>
      <div class="res" id="palres">${res || `<div class="empty" style="padding:28px"><p>No matches for “${esc(p.q)}”.</p></div>`}</div>
      <div class="foot"><span><kbd>↑</kbd> <kbd>↓</kbd> move</span><span><kbd>Enter</kbd> open</span><span><kbd>Esc</kbd> close</span></div></div>`;
  }
  H.actions["palette-open"] = () => { H.S.ui.palette = { q: "", i: 0 }; H.S.ui.enter = "palette"; H.S.ui.focus = "palq"; H.later(); };
  H.actions["palette-close"] = () => { H.S.ui.palette = null; H.render(); };
  H.inputs["palette-q"] = (v) => { H.S.ui.palette.q = v; H.S.ui.palette.i = 0; H.render(); };
  H.actions["palette-go"] = (el) => runPal(el.dataset.to);
  function runPal(to) {
    H.S.ui.palette = null;
    if (to.startsWith("#")) return H.go(to);
    if (to.startsWith("act:")) { H.actions[to.slice(4)](); return H.render(); }
    if (to.startsWith("art:")) { if (H.route().name !== "library") location.hash = "#/library"; H.openDrawer(to.slice(4)); }
  }

  // ---------------------------------------------------------------- theme
  H.isDark = () => themePref === "dark" || (themePref === "system" && window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches);
  function applyTheme() { document.documentElement.setAttribute("data-theme", H.isDark() ? "dark" : "light"); }
  H.setTheme = (t) => { themePref = t; try { localStorage.setItem("harneo-theme", t); } catch (e) { /* storage blocked */ } H.render(); };
  H.themePref = () => themePref;
  H.actions["theme-cycle"] = () => H.setTheme(H.isDark() ? "light" : "dark");
  if (window.matchMedia) window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => H.render());

  // ----------------------------------------------------------- demo bar
  H.actions["demo-reset"] = () => { H.S = H.fresh(false); H.go("#/library"); H.toast("Demo data reset"); };
  H.actions["demo-firstrun"] = () => { H.S = H.fresh(true); H.go("#/welcome"); };

  H.actions["shortcuts"] = () => H.modal({
    title: "Keyboard shortcuts",
    body: `<div class="keys">
      <div><span>Search and commands</span><kbd>Ctrl K</kbd></div><div><span>Focus library search</span><kbd>/</kbd></div>
      <div><span>Move between cards</span><span><kbd>←</kbd> <kbd>→</kbd> <kbd>↑</kbd> <kbd>↓</kbd></span></div><div><span>Open card</span><kbd>Enter</kbd></div>
      <div><span>Close panel or dialog</span><kbd>Esc</kbd></div><div><span>Import</span><kbd>I</kbd></div>
      <div><span>New profile</span><kbd>N</kbd></div><div><span>Toggle cards / list</span><kbd>V</kbd></div>
      <div><span>This list</span><kbd>?</kbd></div><div><span>Toggle theme</span><kbd>T</kbd></div></div>`,
  });

  // ------------------------------------------------------------ events
  document.addEventListener("click", (ev) => {
    const el = ev.target.closest("[data-a]");
    if (!el) { if (H.S.ui.menu && !ev.target.closest(".menu")) { H.S.ui.menu = null; H.render(); } return; }
    const fn = H.actions[el.dataset.a];
    if (!fn) return;
    ev.preventDefault();
    if (H.S.ui.menu && !el.closest(".menu") && el.dataset.a !== "menu") H.S.ui.menu = null;
    H._dispatching = true;
    let r;
    try { r = fn(el, ev); } finally { H._dispatching = false; }
    if (r !== false && !el.dataset.norender) H.render();
  });
  document.addEventListener("input", (ev) => {
    const el = ev.target.closest("[data-in]");
    if (el && H.inputs[el.dataset.in]) H.inputs[el.dataset.in](el.type === "checkbox" ? el.checked : el.value, el, ev);
  });
  document.addEventListener("change", (ev) => {
    const el = ev.target.closest("[data-ch]");
    if (el && H.inputs[el.dataset.ch]) { H.inputs[el.dataset.ch](el.value, el, ev); H.render(); }
  });
  H.actions.menu = (el) => { H.S.ui.menu = H.S.ui.menu === el.dataset.menu ? null : el.dataset.menu; };

  document.addEventListener("keydown", (ev) => {
    const S = H.S;
    const typing = /INPUT|TEXTAREA|SELECT/.test(ev.target.tagName);
    if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "k") { ev.preventDefault(); return S.ui.palette ? H.actions["palette-close"]() : H.actions["palette-open"](); }
    if (S.ui.palette) {
      const p = S.ui.palette;
      if (ev.key === "ArrowDown") { ev.preventDefault(); p.i = Math.min(p.count - 1, p.i + 1); H.render(); }
      else if (ev.key === "ArrowUp") { ev.preventDefault(); p.i = Math.max(0, p.i - 1); H.render(); }
      else if (ev.key === "Enter") { ev.preventDefault(); const b = document.querySelector('.pal .it[data-i="' + p.i + '"]'); if (b) runPal(b.dataset.to); }
      else if (ev.key === "Escape") H.actions["palette-close"]();
      return;
    }
    if (ev.key === "Escape") {
      if (S.ui.menu) { S.ui.menu = null; return H.render(); }
      if (S.ui.modal) return H.closeModal();
      if (S.selected) { S.selected = null; return H.render(); }
      if (S.ui.filtersOpen) { S.ui.filtersOpen = false; return H.render(); }
      if (typing) ev.target.blur();
      return;
    }
    if (typing || ev.ctrlKey || ev.metaKey || ev.altKey || S.ui.modal) return;
    if (H.keys && H.keys(ev)) return;
    if (ev.key === "?") { ev.preventDefault(); H.actions.shortcuts(); H.render(); }
    else if (ev.key.toLowerCase() === "i" && !S.firstRun) { ev.preventDefault(); H.go("#/import"); }
    else if (ev.key.toLowerCase() === "n" && !S.firstRun) { ev.preventDefault(); H.actions["profile-new"](); H.render(); }
    else if (ev.key.toLowerCase() === "t") { ev.preventDefault(); H.actions["theme-cycle"](); }
  });

  window.addEventListener("DOMContentLoaded", () => H.render());
})();
