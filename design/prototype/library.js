/* Harneo prototype: Library, collections, artifact detail drawer, tags. */
(function () {
  "use strict";
  const H = window.H, esc = H.esc, ic = H.icon;
  const S = () => H.S;

  // ------------------------------------------------------------ filtering
  const STATUS_LABEL = { findings: "Has findings", outdated: "Outdated", unused: "Unused 8 weeks", current: "Current" };
  function matches(a, f) {
    if (f.types.length && !f.types.includes(a.type)) return false;
    if (f.tags.length && !f.tags.some((t) => a.tags.includes(t))) return false;
    if (f.status.length && !f.status.includes(H.status(a))) return false;
    if (f.sources.length && !f.sources.includes(a.source)) return false;
    if (f.inv.length && !f.inv.includes(a.invocation)) return false;
    if (a.trust < f.trustMin) return false;
    if (f.q) {
      const q = f.q.toLowerCase();
      const hay = (a.name + " " + a.desc + " " + H.SRC[a.source].label + " " + a.tags.join(" ")).toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  }
  H.collectionItems = (c) => {
    if (c.kind === "manual") return c.items.map(H.art).filter(Boolean);
    const f = Object.assign({ types: [], tags: [], status: [], sources: [], inv: [], trustMin: 0, q: "" }, c.rule);
    return S().artifacts.filter((a) => matches(a, f));
  };
  const filtersActive = (f) => f.types.length || f.tags.length || f.status.length || f.sources.length || f.inv.length || f.trustMin > 0 || f.q;
  function sorted(list) {
    const s = S().sort;
    const by = { used: (a, b) => b.sessions - a.sessions, name: (a, b) => a.name.localeCompare(b.name), trust: (a, b) => b.trust - a.trust, risk: (a, b) => a.trust - b.trust };
    return list.slice().sort(by[s] || by.used);
  }

  // --------------------------------------------------------- filter column
  function fopt(group, value, label, n, extra) {
    const on = S().f[group].includes(value);
    return `<button class="fopt${on ? " on" : ""}" data-a="filter" data-g="${group}" data-v="${esc(value)}" aria-pressed="${on}"><i class="box">${on ? ic("check") : ""}</i>${extra || ""}<span class="label">${esc(label)}</span><span class="n">${n}</span></button>`;
  }
  function filterColumn(base) {
    const f = S().f;
    const count = (fn) => base.filter(fn).length;
    const types = Object.keys(H.T).map((t) => [t, count((a) => a.type === t)]).filter((x) => x[1]).sort((a, b) => b[1] - a[1]);
    const tags = S().tags.map((t) => [t.id, count((a) => a.tags.includes(t.id))]).filter((x) => x[1]);
    const srcs = Object.keys(H.SRC).map((s) => [s, count((a) => a.source === s)]).filter((x) => x[1]);
    const clear = (g) => (f[g].length ? `<button data-a="filter-clear" data-g="${g}">Clear</button>` : "");
    return `<aside class="filters${S().ui.filtersOpen ? " open" : ""}" aria-label="Filters">
      <h4>Type ${clear("types")}</h4>
      ${types.map(([t, n]) => fopt("types", t, H.T[t].plural, n, `<span class="ic sm t-${H.T[t].tint}" style="width:18px;height:18px">${ic(H.T[t].icon)}</span>`)).join("")}
      <h4>Tags ${clear("tags")}</h4>
      ${tags.map(([t, n]) => fopt("tags", t, t, n, `<i class="dot" style="border-radius:50%;background:var(--tint-${H.tagColor(t)})"></i>`)).join("") || `<span class="muted">No tags yet</span>`}
      <h4>Status ${clear("status")}</h4>
      ${["findings", "outdated", "unused"].map((s) => fopt("status", s, STATUS_LABEL[s], count((a) => H.status(a) === s))).join("")}
      <h4>Source ${clear("sources")}</h4>
      ${srcs.map(([s, n]) => fopt("sources", s, H.SRC[s].label, n)).join("")}
      <h4>Trust score <span class="muted" style="font-weight:400">≥ ${f.trustMin}</span></h4>
      <div class="trustrow"><input type="range" min="0" max="100" step="5" value="${f.trustMin}" data-in="trust" id="trustrange" aria-label="Minimum trust score"></div>
      <h4>Invocation ${clear("inv")}</h4>
      ${fopt("inv", "auto", "Auto", count((a) => a.invocation === "auto"))}
      ${fopt("inv", "manual", "Manual only", count((a) => a.invocation === "manual"))}
    </aside>`;
  }
  H.actions.filter = (el) => { const g = el.dataset.g, v = el.dataset.v, arr = S().f[g]; const i = arr.indexOf(v); i < 0 ? arr.push(v) : arr.splice(i, 1); };
  H.actions["filter-clear"] = (el) => { S().f[el.dataset.g] = []; };
  H.actions["filters-all-clear"] = () => { Object.assign(S().f, { types: [], tags: [], status: [], sources: [], inv: [], trustMin: 0, q: "" }); };
  H.actions["filters-open"] = () => { S().ui.filtersOpen = true; };
  H.actions["filters-close"] = () => { S().ui.filtersOpen = false; };
  H.inputs.trust = (v) => { S().f.trustMin = +v; H.render(); };
  H.inputs.q = (v) => { S().f.q = v; H.render(); };
  H.inputs.sort = (v) => { S().sort = v; };
  H.actions.view = (el) => { S().view = el.dataset.v; };
  H.actions.status = (el) => { S().f.status = [el.dataset.v]; if (H.route().name !== "library") location.hash = "#/library"; };

  // ----------------------------------------------------------- insights
  function insights(all) {
    const n = (s) => all.filter((a) => H.status(a) === s).length;
    const cur = n("current"), un = n("unused"), out = n("outdated"), fi = n("findings");
    const issues = un + out + fi;
    const weeks = [0, 1, 2, 3, 4, 5, 6, 7].map((w) => all.reduce((s, a) => s + a.usage[w], 0));
    const max = Math.max(1, ...weeks);
    const last = weeks[7], prev = weeks[6] || 1;
    const delta = Math.round(((last - prev) / prev) * 100);
    const legend = (s, color, label, v) => `<button data-a="status" data-v="${s}"><i class="dot" style="background:${color}"></i><b>${v}</b> ${label}</button>`;
    return `<div class="ins">
      <section class="panel" aria-label="Library health"><h2>Library health ${issues ? `<a href="#/cleanup">Review ${issues} issues</a>` : ""}</h2>
        <div class="stack" role="img" aria-label="${cur} current, ${un} unused, ${out} outdated, ${fi} with findings">
          <i style="flex:${cur};background:var(--green-fill)"></i><i style="flex:${un};background:var(--line2)"></i><i style="flex:${out};background:var(--amber-fill)"></i><i style="flex:${fi};background:var(--red-fill)"></i></div>
        <div class="leg"><span style="display:flex;align-items:center;gap:6px"><i class="dot" style="background:var(--green-fill)"></i><b style="color:var(--t1)">${cur}</b> current</span>${legend("unused", "var(--line2)", "unused", un)}${legend("outdated", "var(--amber-fill)", "outdated", out)}${legend("findings", "var(--red-fill)", "findings", fi)}</div>
      </section>
      <section class="panel" aria-label="Activity"><h2>Sessions using library artifacts <span class="mono muted">${last} this week · ${delta >= 0 ? "+" : ""}${delta}%</span></h2>
        <div class="bars" role="img" aria-label="Sessions per week for 8 weeks, latest ${last}">${weeks.map((w, i) => `<i class="${i === 7 ? "now" : ""}" style="height:${Math.round((w / max) * 100)}%" title="${w} sessions"></i>`).join("")}</div>
        <div class="axis"><span>8 weeks ago</span><span>this week</span></div>
      </section></div>`;
  }

  // -------------------------------------------------------------- cards
  function tile(a) {
    const st = H.status(a), gray = st === "unused";
    const tags = a.tags.slice(0, 2).map((t) => H.tagChip(t)).join("") + (a.tags.length > 2 ? `<span class="tag">+${a.tags.length - 2}</span>` : "");
    const usage = H.usageTypes.includes(a.type);
    return `<button class="tile${S().selected === a.id ? " sel" : ""}${gray ? " unused" : ""}" data-a="open" data-id="${a.id}" data-card id="card-${a.id}">
      <div class="head">${H.typeIc(a.type, false, gray)}<div><div class="nm">${esc(a.name)}</div><div class="srcline">${esc(H.srcLabel(a))}</div></div></div>
      <div class="desc">${esc(a.desc)}</div>
      <div class="tags">${tags}</div>
      <div class="foot">${H.meter(a.trust, gray)}${usage ? (gray ? `<span class="m muted">0 sessions · ${esc(a.lastUsed)}</span>` : H.spark(a.usage) + `<span class="m muted">${a.sessions} sessions</span>`) : `<span class="m muted">${H.plural(a.installedIn.length, "project")}</span>`}${H.statusBadge(a)}</div>
    </button>`;
  }
  function listView(list) {
    return `<table class="listv"><thead><tr><th>Name</th><th>Type</th><th>Source</th><th>Tags</th><th>Trust</th><th>Sessions 8w</th><th>Status</th></tr></thead><tbody>
      ${list.map((a) => `<tr class="${S().selected === a.id ? "sel" : ""}${H.status(a) === "unused" ? " unused" : ""}" data-a="open" data-id="${a.id}" tabindex="0" data-card id="card-${a.id}">
        <td><span class="namecell">${H.typeIc(a.type, true, H.status(a) === "unused")}${esc(a.name)}</span></td><td>${H.T[a.type].label}</td><td class="mono muted">${esc(H.srcLabel(a))}</td>
        <td>${a.tags.slice(0, 2).map((t) => H.tagChip(t)).join(" ")}</td><td>${H.meter(a.trust)}</td><td class="mono">${H.usageTypes.includes(a.type) ? a.sessions : "—"}</td><td>${H.statusBadge(a)}</td></tr>`).join("")}
    </tbody></table>`;
  }
  function results(list, base) {
    const f = S().f;
    const chips = [];
    f.types.forEach((v) => chips.push(["types", v, H.T[v].plural]));
    f.tags.forEach((v) => chips.push(["tags", v, "#" + v]));
    f.status.forEach((v) => chips.push(["status", v, STATUS_LABEL[v]]));
    f.sources.forEach((v) => chips.push(["sources", v, H.SRC[v].label]));
    f.inv.forEach((v) => chips.push(["inv", v, v === "auto" ? "Auto" : "Manual only"]));
    if (f.trustMin) chips.push(["trust", "", "Trust ≥ " + f.trustMin]);
    const isCol = H.route().name === "collection";
    const toolbar = `<div class="toolbar">
      ${chips.map((c) => `<span class="fchip">${esc(c[2])}<button data-a="${c[0] === "trust" ? "trust-clear" : "filter"}" data-g="${c[0]}" data-v="${esc(c[1])}" aria-label="Remove filter ${esc(c[2])}">${ic("x")}</button></span>`).join("")}
      <span>${H.plural(list.length, "artifact")}${list.length !== base.length ? " of " + base.length : ""}</span>
      ${filtersActive(f) ? `<button class="btn ghost sm" data-a="filters-all-clear">Clear all</button>${!isCol ? `<button class="btn ghost sm" data-a="collection-from-filters">${ic("list-filter")}Save as collection</button>` : ""}` : ""}
      <span class="grow"></span>
      <label class="sr" for="sortsel">Sort</label><select class="sel" id="sortsel" data-ch="sort">
        ${[["used", "Most used"], ["name", "Name"], ["trust", "Highest trust"], ["risk", "Lowest trust"]].map((o) => `<option value="${o[0]}"${S().sort === o[0] ? " selected" : ""}>Sort: ${o[1]}</option>`).join("")}</select></div>`;
    if (!list.length) {
      return toolbar + `<div class="empty"><div class="ico">${ic("search-x")}</div><h3>No artifacts match</h3><p>Nothing in ${isCol ? "this collection" : "the library"} matches these filters${f.q ? ` and “${esc(f.q)}”` : ""}.</p><button class="btn" data-a="filters-all-clear">Clear filters</button></div>`;
    }
    return toolbar + (S().view === "cards" ? `<div class="grid" id="cardgrid">${list.map(tile).join("")}</div>` : listView(list));
  }
  function topbar(title, extra) {
    return `<div class="libtop">${title}
      <button class="btn filtersbtn" data-a="filters-open">${ic("sliders-horizontal")}Filters</button>
      <div class="search"><label class="sr" for="libq">Search artifacts</label>${ic("search")}<input class="input" id="libq" placeholder="Search artifacts, sources, tags" value="${esc(S().f.q)}" data-in="q" autocomplete="off"></div>
      <div class="seg" role="group" aria-label="View"><button class="${S().view === "cards" ? "on" : ""}" data-a="view" data-v="cards" aria-pressed="${S().view === "cards"}">${ic("layout-grid")}Cards</button><button class="${S().view === "list" ? "on" : ""}" data-a="view" data-v="list" aria-pressed="${S().view === "list"}">${ic("list")}List</button></div>
      ${extra || `<a class="btn primary" href="#/import">${ic("download")}Import</a>`}</div>`;
  }

  H.screens.library = function () {
    const all = S().artifacts;
    if (!all.length) {
      return `<div class="page"><div class="empty" style="padding-top:12vh"><div class="ico">${ic("library")}</div><h3>Your library is empty</h3>
        <p style="max-width:52ch">The library holds every artifact you trust: skills, subagents, commands, settings, MCP servers and more. Import them once, then compose profiles and install them into projects.</p>
        <a class="btn primary" href="#/import">${ic("download")}Import artifacts</a></div></div>`;
    }
    const list = sorted(all.filter((a) => matches(a, S().f)));
    return `<div class="lib">${filterColumn(all)}<div class="libmain">
      ${topbar(`<h1>Library</h1>`)}
      ${insights(all)}
      ${results(list, all)}</div></div>`;
  };

  H.screens.collection = function (r) {
    const c = S().collections.find((x) => x.id === r.id);
    if (!c) return H.screens.library();
    const base = H.collectionItems(c);
    const list = sorted(base.filter((a) => matches(a, S().f)));
    const rule = c.kind === "smart" ? "Saved filter: " + Object.entries(c.rule).map(([k, v]) => (k === "tags" ? v.map((t) => "#" + t).join(", ") : k === "status" ? v.map((s) => STATUS_LABEL[s]).join(" or ") : k === "types" ? v.map((t) => H.T[t].plural).join(", ") : k === "sources" ? v.map((s) => H.SRC[s].label).join(", ") : k === "trustMin" ? "trust ≥ " + v : k + ": " + v)).join(" · ") : "Hand-picked. Add artifacts from their detail panel.";
    return `<div class="lib">${filterColumn(base)}<div class="libmain">
      ${topbar(`<h1>${ic(c.kind === "smart" ? "list-filter" : "bookmark")} ${esc(c.name)}</h1><span class="badge">${c.kind === "smart" ? "Smart" : "Manual"}</span>`,
        `<button class="btn" data-a="profile-from-collection" data-id="${c.id}">${ic("layers")}Create profile</button><button class="btn ghost" data-a="menu" data-menu="col-${c.id}" aria-label="Collection actions">${ic("ellipsis")}</button>
         ${S().ui.menu === "col-" + c.id ? `<span class="suggest"><div class="menu" style="right:0;left:auto"><button data-a="collection-rename" data-id="${c.id}">${ic("pencil")}Rename</button><div class="sep"></div><button data-a="collection-delete" data-id="${c.id}" style="color:var(--red)">${ic("trash-2")}Delete collection</button></div></span>` : ""}`)}
      <p class="sec" style="margin:10px 0 16px">${esc(rule)} Collections only organize the library; nothing is installed until you use a profile.</p>
      ${results(list, base)}</div></div>`;
  };

  // ---------------------------------------------------------- collections
  H.actions["collection-new"] = () => H.modal({
    title: "New collection",
    sub: "Collections organize your library. They never install anything.",
    body: () => `<div class="field"><label for="colname">Name</label><input class="input" id="colname" data-in="colname" value="${esc(S().flow.colname || "")}" placeholder="e.g. Review kit" autofocus></div>
      <div class="optlist">
        <button class="opt${(S().flow.colkind || "manual") === "manual" ? " on" : ""}" data-a="colkind" data-v="manual"><i class="radio"></i><div><b>Manual</b><div class="sec">Hand-pick artifacts from their detail panel.</div></div></button>
        <button class="opt${S().flow.colkind === "smart" ? " on" : ""}" data-a="colkind" data-v="smart"${filtersActive(S().f) ? "" : " disabled"}><i class="radio"></i><div><b>Smart</b><div class="sec">${filtersActive(S().f) ? "Saves the library's current filters; updates itself." : "Set filters in the library first, then save them as a smart collection."}</div></div></button>
      </div>`,
    foot: `<button class="btn" data-a="modal-close">Cancel</button><button class="btn primary" data-a="collection-create">Create collection</button>`,
  });
  H.inputs.colname = (v) => { S().flow.colname = v; };
  H.actions.colkind = (el) => { S().flow.colkind = el.dataset.v; };
  H.actions["collection-from-filters"] = () => { S().flow.colkind = "smart"; S().flow.colname = ""; H.actions["collection-new"](); };
  H.actions["collection-create"] = () => {
    const name = (S().flow.colname || "").trim() || "Untitled collection";
    const kind = S().flow.colkind || "manual";
    const f = S().f;
    const rule = {};
    ["types", "tags", "status", "sources", "inv"].forEach((k) => { if (f[k].length) rule[k] = f[k].slice(); });
    if (f.trustMin) rule.trustMin = f.trustMin;
    if (f.q) rule.q = f.q;
    const c = kind === "smart" ? { id: H.uid("c"), name, kind, rule } : { id: H.uid("c"), name, kind, items: [] };
    S().collections.push(c);
    S().flow.colname = ""; S().flow.colkind = null; S().ui.modal = null;
    H.actions["filters-all-clear"]();
    H.go("#/collection/" + c.id);
    H.toast(`Created ${kind} collection “${esc(name)}”`);
  };
  H.actions["collection-rename"] = (el) => {
    const c = S().collections.find((x) => x.id === el.dataset.id);
    S().flow.colname = c.name;
    H.modal({ title: "Rename collection", body: `<div class="field"><label for="colname">Name</label><input class="input" id="colname" data-in="colname" value="${esc(c.name)}" autofocus></div>`, foot: `<button class="btn" data-a="modal-close">Cancel</button><button class="btn primary" data-a="collection-rename-ok" data-id="${c.id}">Rename</button>` });
  };
  H.actions["collection-rename-ok"] = (el) => { const c = S().collections.find((x) => x.id === el.dataset.id); c.name = (S().flow.colname || c.name).trim(); S().ui.modal = null; };
  H.actions["collection-delete"] = (el) => {
    const c = S().collections.find((x) => x.id === el.dataset.id);
    H.confirm({ title: `Delete “${c.name}”?`, sub: "The artifacts stay in your library; only this grouping is removed.", ok: "Delete collection", danger: true, onOk: () => {
      S().collections = S().collections.filter((x) => x !== c);
      H.go("#/library");
      H.toast(`Deleted collection “${esc(c.name)}”`, { undo: () => S().collections.push(c) });
    } });
  };

  // --------------------------------------------------------------- drawer
  H.openDrawer = (id) => { S().selected = id; S().ui.enter = "drawer"; H.later(); };
  H.actions.open = (el) => H.openDrawer(el.dataset.id);
  H.actions["drawer-close"] = () => { const id = S().selected; S().selected = null; S().ui.focus = id ? "card-" + id : null; };

  H.screens.drawer = function (id) {
    const a = H.art(id);
    if (!a) return "";
    const st = H.status(a), src = H.SRC[a.source];
    const usage = H.usageTypes.includes(a.type);
    const inProfiles = S().profiles.filter((p) => p.items.includes(a.id) || p.settings === a.id || p.mcp.some((m) => m.id === a.id));
    const max = Math.max(1, ...a.usage);
    const cols = S().collections.filter((c) => c.kind === "manual" && c.items.includes(a.id));
    const menu = S().ui.menu;
    return `<aside class="drawer${H.enter("drawer")}" role="dialog" aria-label="${esc(a.name)} details">
      <div class="dhead"><div class="row">${H.typeIc(a.type)}<div style="min-width:0"><h2>${esc(a.name)}</h2><div class="sec">${H.T[a.type].label}${a.invocation ? " · " + (a.invocation === "auto" ? "auto-invoked" : "manual only") : ""}${a.bundle.length ? " · bundle of " + (a.bundle.length + 1) : ""}</div></div>
        <button class="btn ghost sm x" data-a="drawer-close" aria-label="Close details">${ic("x")}</button></div>
        <div class="dactions">
          <button class="btn sm" data-a="toast" data-msg="Opened ${esc(a.name)} in $EDITOR">${ic("square-pen")}Open in editor</button>
          <button class="btn sm" data-a="toast" data-msg="Revealed the library copy in your file manager">${ic("folder-open")}Show in folder</button>
          <span class="suggest"><button class="btn sm" data-a="menu" data-menu="addto">${ic("plus")}Add to…${ic("chevron-down")}</button>
            ${menu === "addto" ? `<div class="menu">${S().collections.filter((c) => c.kind === "manual").map((c) => `<button data-a="add-to-collection" data-c="${c.id}">${ic("bookmark")}${esc(c.name)}${c.items.includes(a.id) ? `<span class="muted" style="margin-left:auto">added</span>` : ""}</button>`).join("")}<button data-a="collection-new">${ic("bookmark-plus")}New collection…</button><div class="sep"></div>${S().profiles.map((p) => `<button data-a="add-to-profile" data-p="${p.id}">${ic("layers")}Profile: ${esc(p.name)}${p.items.includes(a.id) ? `<span class="muted" style="margin-left:auto">in profile</span>` : ""}</button>`).join("")}</div>` : ""}</span>
          <button class="btn sm danger" data-a="remove-artifact" data-id="${a.id}">${ic("trash-2")}Remove</button>
        </div></div>
      <div class="dbody" id="dbody">
        ${st === "findings" ? `<div class="dsec"><div class="callout bad">${ic("shield-alert")}<div><b>Trust ${a.trust}: ${a.findings.length} security findings.</b><p>Review before using it in any profile.</p></div></div></div>` : ""}
        ${a.latest ? `<div class="dsec"><div class="callout warn">${ic("circle-arrow-up")}<div style="flex:1"><b>Update available: ${esc(src.version)} → ${esc(a.latest)}</b><p>${a.localEdits ? "You edited this artifact locally. You'll choose between your version and upstream." : "Upstream changed since you imported it."}</p></div><button class="btn sm" data-a="update-artifact" data-id="${a.id}">Update</button></div></div>` : ""}
        <div class="dsec"><p class="sec">${esc(a.desc)}</p></div>
        <div class="dsec"><h3>Tags</h3><div class="tagedit">${a.tags.map((t) => H.tagChip(t, true)).join("")}
          <span class="suggest"><label class="sr" for="tagin">Add tag</label><input class="input" id="tagin" placeholder="Add tag…" data-in="tagq" value="${esc(S().flow.tagq || "")}" autocomplete="off">${tagMenu(a)}</span></div></div>
        ${a.type === "skill" ? `<div class="dsec"><div class="switchrow"><div><h3 style="margin:0">Manual only</h3><p class="sec">Never auto-invoked. Sets <span class="mono">disable-model-invocation: true</span>.</p></div><button class="toggle${a.invocation === "manual" ? " on" : ""}" data-a="toggle-inv" data-id="${a.id}" role="switch" aria-checked="${a.invocation === "manual"}" aria-label="Manual only"><i></i></button></div></div>` : ""}
        ${a.findings.length ? `<div class="dsec"><h3>Security findings</h3>${a.findings.map((x) => `<div class="finding ${x.level}">${ic(x.level === "high" ? "octagon-alert" : "triangle-alert")}<div><b>${esc(x.title)}</b><span class="mono">${esc(x.detail)}</span></div></div>`).join("")}</div>` : ""}
        <div class="dsec"><h3>Trust and reach ${H.meter(a.trust)}</h3><div class="kv">
          <span>Files</span><span class="mono">${esc(a.reach.files)}</span><span>Shell</span><span class="mono">${esc(a.reach.shell)}</span>
          <span>Network</span><span class="mono">${esc(a.reach.network)}</span><span>Env vars</span><span class="mono">${esc(a.reach.env)}</span></div></div>
        ${a.bundle.length ? `<div class="dsec"><h3>Bundle <span class="muted" style="font-weight:400">installed together, always</span></h3><div class="pins">${a.bundle.map((b) => { const x = H.art(b); return x ? `<button class="pin" data-a="open" data-id="${x.id}">${H.typeIc(x.type, true)}<span>${esc(x.name)}</span><span class="muted">${H.T[x.type].label}</span></button>` : ""; }).join("")}</div></div>` : ""}
        ${usage ? `<div class="dsec"><h3>Usage <span class="muted" style="font-weight:400">${a.sessions} sessions in 8 weeks · last ${esc(a.lastUsed)}</span></h3>
          <div class="bars" style="height:48px" role="img" aria-label="Sessions per week">${a.usage.map((n, i) => `<i class="${i === 7 ? "now" : ""}" style="height:${Math.max(3, Math.round((n / max) * 100))}%"></i>`).join("")}</div><div class="axis"><span>8 weeks ago</span><span>this week</span></div></div>` : ""}
        <div class="dsec"><h3>Used in</h3>
          ${inProfiles.length ? `<div class="pins">${inProfiles.map((p) => `<a class="pin" href="#/profiles/${p.id}">${ic("layers")}<span>${esc(p.name)} v${p.version}</span><span class="muted">${H.plural(p.projects.length, "project")}</span></a>`).join("")}</div>` : `<p class="muted">Not in any profile yet.</p>`}
          ${cols.length ? `<p class="sec" style="margin-top:8px">Collections: ${cols.map((c) => `<a href="#/collection/${c.id}">${esc(c.name)}</a>`).join(", ")}</p>` : ""}</div>
        <div class="dsec"><h3>Provenance</h3><div class="kv">
          <span>Source</span><span class="mono">${src.kind === "local" ? esc(src.url) : `<a href="#" data-a="toast" data-msg="Opens ${esc(src.url)} in your browser">${esc(src.url.replace("https://", ""))}</a>`}</span>
          ${src.kind === "git" ? `<span>Branch</span><span class="mono">${esc(src.branch)}</span><span>Subfolder</span><span class="mono">${esc(src.subfolder)}</span><span>Commit</span><span class="mono">${esc(src.commit)}</span>` : ""}
          <span>Version</span><span class="mono">${esc(src.version)}${a.latest ? ` <span class="badge warn">${esc(a.latest)} available</span>` : ""}</span><span>Library copy</span><span class="mono">~/.local/share/io.harneo/library/${esc(a.type)}/${esc(a.name.replace("/", ""))}</span></div></div>
      </div></aside>`;
  };
  function tagMenu(a) {
    const q = (S().flow.tagq || "").trim().toLowerCase();
    if (!q) return "";
    const opts = S().tags.filter((t) => t.id.includes(q) && !a.tags.includes(t.id));
    const exact = S().tags.some((t) => t.id === q);
    return `<div class="menu">${opts.map((t) => `<button data-a="tag-add" data-tag="${esc(t.id)}">${H.tagChip(t.id)}</button>`).join("")}${!exact && /^[a-z0-9-]+$/.test(q) ? `<button data-a="tag-add" data-tag="${esc(q)}" data-new="1">${ic("plus")}Create tag “${esc(q)}”</button>` : ""}${!opts.length && (exact || !/^[a-z0-9-]+$/.test(q)) ? `<span class="muted" style="padding:6px 8px;display:block">${exact ? "Already tagged" : "Lowercase letters, digits and dashes"}</span>` : ""}</div>`;
  }
  const COLORS = ["blue", "green", "violet", "pink", "orange", "teal", "amber", "indigo", "cyan"];
  H.inputs.tagq = (v) => { S().flow.tagq = v; H.render(); };
  H.actions["tag-add"] = (el) => {
    const a = H.art(S().selected), t = el.dataset.tag;
    if (el.dataset.new && !S().tags.some((x) => x.id === t)) S().tags.push({ id: t, color: COLORS[S().tags.length % COLORS.length] });
    if (!a.tags.includes(t)) a.tags.push(t);
    S().flow.tagq = ""; S().ui.focus = "tagin";
  };
  H.actions["tag-remove"] = (el) => { const a = H.art(S().selected); a.tags = a.tags.filter((t) => t !== el.dataset.tag); };
  H.actions["toggle-inv"] = (el) => { const a = H.art(el.dataset.id); a.invocation = a.invocation === "manual" ? "auto" : "manual"; H.toast(a.invocation === "manual" ? `${esc(a.name)} is now manual only` : `${esc(a.name)} can be auto-invoked again`); };
  H.actions.toast = (el) => { H.toast(el.dataset.msg, { icon: "info" }); return false; };
  H.actions["add-to-collection"] = (el) => {
    const c = S().collections.find((x) => x.id === el.dataset.c), id = S().selected;
    if (!c.items.includes(id)) { c.items.push(id); H.toast(`Added to “${esc(c.name)}”`); } else H.toast(`Already in “${esc(c.name)}”`, { icon: "info" });
  };
  H.actions["add-to-profile"] = (el) => {
    const p = H.profile(el.dataset.p), a = H.art(S().selected);
    if (p.items.includes(a.id)) return H.toast(`Already in ${esc(p.name)}`, { icon: "info" });
    const added = [a.id].concat(a.bundle).filter((x) => !p.items.includes(x));
    p.items.push(...added); p.dirty = true;
    H.toast(`Added ${esc(a.name)}${added.length > 1 ? ` + ${added.length - 1} bundled` : ""} to ${esc(p.name)}. Save the profile to apply it.`);
  };
  H.actions["remove-artifact"] = (el) => {
    const a = H.art(el.dataset.id);
    const inProfiles = S().profiles.filter((p) => p.items.includes(a.id));
    H.confirm({ title: `Remove ${a.name} from the library?`, danger: true, ok: "Remove from library",
      body: `<p class="sec">${inProfiles.length ? `It's in ${H.plural(inProfiles.length, "profile")} (${inProfiles.map((p) => esc(p.name)).join(", ")}); it will be taken out of them. ` : ""}Projects that already have it keep their copy until you push a profile change. Nothing in any project is deleted now.</p>`,
      onOk: () => {
        const idx = S().artifacts.indexOf(a);
        S().artifacts.splice(idx, 1);
        inProfiles.forEach((p) => { p.items = p.items.filter((x) => x !== a.id); });
        S().selected = null;
        H.toast(`Removed ${esc(a.name)} from the library`, { undo: () => { S().artifacts.splice(idx, 0, a); inProfiles.forEach((p) => p.items.push(a.id)); } });
      } });
  };
  H.actions["update-artifact"] = (el) => {
    const a = H.art(el.dataset.id), src = H.SRC[a.source];
    if (!a.localEdits) {
      return H.confirm({ title: `Update ${a.name} to ${a.latest}?`, ok: "Update library copy",
        body: `<p class="sec">Updates the library copy from <span class="mono">${esc(src.label)}</span> (${esc(src.version)} → ${esc(a.latest)}). Projects get it when you re-apply or switch their profile.</p><div class="callout" style="margin-top:12px">${ic("git-commit-horizontal")}<div><b>Upstream changes</b><p class="mono">+ adds a dry-run flag · ~ clearer changelog step · − drops the npm-only tag step</p></div></div>`,
        onOk: () => { a.latest = null; H.toast(`Updated ${esc(a.name)}`); } });
    }
    S().flow.keep = S().flow.keep || "mine";
    H.modal({ title: `${a.name}: your edits vs upstream`, sub: `Harneo never merges and never overwrites silently. Pick one version; the other is backed up.`, wide: true,
      body: () => `<div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
        ${["mine", "upstream"].map((k) => `<div class="group" style="margin:0;${S().flow.keep === k ? "border-color:var(--blue);box-shadow:0 0 0 3px var(--blue-ring)" : ""}"><header>${k === "mine" ? "Yours (edited locally)" : "Upstream " + esc(a.latest)}</header><div class="code"><pre>${(k === "mine" ? ["export const meta = { name: 'release' }", "  step('changelog')", "+ step('notify-slack')   // your addition", "  step('tag', { sign: true })"] : ["export const meta = { name: 'release', version: '3.4.1' }", "  step('changelog')", "+ step('dry-run')", "  step('tag')"]).map((l, i) => `<div class="ln${l.startsWith("+") ? " add" : ""}"><span class="no">${i + 1}</span><span>${esc(l)}</span></div>`).join("")}</pre></div></div>`).join("")}</div>
        <div class="optlist" style="margin-top:12px"><button class="opt${S().flow.keep === "mine" ? " on" : ""}" data-a="keep" data-v="mine"><i class="radio"></i><div><b>Keep mine</b><div class="sec">Stay on your edited copy; mark ${esc(a.latest)} as skipped.</div></div></button>
        <button class="opt${S().flow.keep === "upstream" ? " on" : ""}" data-a="keep" data-v="upstream"><i class="radio"></i><div><b>Take upstream ${esc(a.latest)}</b><div class="sec">Your edited copy is backed up to <span class="mono">library/.backups/release-2026-10-19</span>.</div></div></button></div>`,
      foot: `<button class="btn" data-a="modal-close">Cancel</button><button class="btn primary" data-a="keep-ok" data-id="${a.id}">Confirm</button>` });
  };
  H.actions.keep = (el) => { S().flow.keep = el.dataset.v; };
  H.actions["keep-ok"] = (el) => { const a = H.art(el.dataset.id); const up = S().flow.keep === "upstream"; a.latest = null; if (up) a.localEdits = false; S().ui.modal = null; H.toast(up ? `Updated ${esc(a.name)}; your edits are backed up` : `Kept your version of ${esc(a.name)}`); };
  H.actions["trust-clear"] = () => { S().f.trustMin = 0; };

  // --------------------------------------------------- grid keyboard nav
  H.keys = (ev) => {
    const r = H.route().name;
    if (r !== "library" && r !== "collection") return false;
    if (ev.key === "/") { ev.preventDefault(); const q = document.getElementById("libq"); if (q) q.focus(); return true; }
    if (ev.key.toLowerCase() === "v") { S().view = S().view === "cards" ? "list" : "cards"; H.render(); return true; }
    if (S().selected) return false;
    const cards = Array.from(document.querySelectorAll("[data-card]"));
    if (!cards.length || !/^Arrow/.test(ev.key)) return false;
    ev.preventDefault();
    let i = cards.indexOf(document.activeElement);
    if (i < 0) { cards[0].focus(); return true; }
    const top = cards[0].offsetTop;
    const perRow = S().view === "cards" ? Math.max(1, cards.filter((c) => c.offsetTop === top).length) : 1;
    const step = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: perRow, ArrowUp: -perRow }[ev.key];
    i = Math.max(0, Math.min(cards.length - 1, i + step));
    cards[i].focus();
    cards[i].scrollIntoView({ block: "nearest" });
    return true;
  };
})();
