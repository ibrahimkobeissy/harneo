/* Harneo prototype: Security page (library-wide audit) and Settings template contents. */
(function () {
  "use strict";
  const H = window.H, esc = H.esc, ic = H.icon;
  const S = () => H.S;
  const SEV = { high: 3, medium: 2, low: 1 };
  const reaches = (v) => v && v !== "none";

  // ============================================================ SECURITY
  H.screens.security = function () {
    const arts = S().artifacts;
    const findings = [];
    arts.forEach((a) => a.findings.forEach((f) => findings.push({ a, f })));
    const bySev = { high: 0, medium: 0, low: 0 };
    findings.forEach((x) => { bySev[x.f.level]++; });
    const buckets = [[0, 59, "Untrusted", "var(--red-fill)"], [60, 79, "Review", "var(--amber-fill)"], [80, 100, "Trusted", "var(--green-fill)"]].map(([lo, hi, l, c]) => ({ l, c, lo, hi, n: arts.filter((a) => a.trust >= lo && a.trust <= hi).length }));
    const hist = Array.from({ length: 10 }, (_, i) => arts.filter((a) => Math.min(9, Math.floor(a.trust / 10)) === i).length);
    const hmax = Math.max(1, ...hist);
    const reach = { network: arts.filter((a) => reaches(a.reach.network)), shell: arts.filter((a) => reaches(a.reach.shell)), env: arts.filter((a) => reaches(a.reach.env)), any: arts.filter((a) => /any/.test(a.reach.files)) };
    const rf = S().flow.reachF || "all";
    const reachList = (rf === "all" ? arts.filter((a) => reaches(a.reach.network) || reaches(a.reach.shell) || reaches(a.reach.env) || /any|read-only/.test(a.reach.files)) : reach[rf]);

    const fSort = H.getSort("sec");
    if (!fSort.key) { fSort.key = "severity"; fSort.dir = "desc"; }
    const fRows = H.sortBy(findings, fSort, (x, k) => (k === "severity" ? SEV[x.f.level] * 1000 - x.a.trust : k === "artifact" ? x.a.name : k === "type" ? H.T[x.a.type].label : k === "finding" ? x.f.title : k === "trust" ? x.a.trust : x.f.detail));
    const rSort = H.getSort("reach");
    if (!rSort.key) { rSort.key = "trust"; rSort.dir = "asc"; }
    const rRows = H.sortBy(reachList, rSort, (a, k) => (k === "artifact" ? a.name : k === "trust" ? a.trust : a.reach[k]));
    const sevBadge = (l) => `<span class="badge ${l === "high" ? "bad" : l === "medium" ? "warn" : ""}">${l === "high" ? "High" : l === "medium" ? "Medium" : "Low"}</span>`;
    const statBtn = (k, label, list, icon) => `<button class="rstat${rf === k ? " on" : ""}" data-a="reach-f" data-v="${k}">${ic(icon)}<span><b>${list.length}</b> ${label}</span></button>`;

    return `<div class="page">
      <div class="ph"><h1>Security</h1><span class="muted">Every artifact is audited when it's imported or updated: trust score, findings, and exactly what it can reach.</span><span class="grow"></span>
        <button class="btn" data-a="toast" data-msg="Re-audited ${arts.length} artifacts: no new findings">${ic("refresh-cw")}Re-run audit</button></div>
      <div class="ins ins3">
        <section class="panel"><h2>Trust scores <span class="muted">${arts.length} artifacts</span></h2>
          <div class="bars" style="height:48px" role="img" aria-label="Trust score distribution">${hist.map((n, i) => `<i style="height:${Math.max(4, Math.round((n / hmax) * 100))}%;background:${i < 6 ? "var(--red-fill)" : i < 8 ? "var(--amber-fill)" : "var(--green-fill)"};opacity:${n ? 1 : 0.25}" title="${i * 10}–${i * 10 + 9}: ${n}"></i>`).join("")}</div>
          <div class="axis"><span>0</span><span>50</span><span>100</span></div>
          <div class="leg">${buckets.map((b) => `<span style="display:flex;gap:6px;align-items:center"><i class="dot" style="background:${b.c}"></i><b>${b.n}</b> ${b.l.toLowerCase()} <span class="muted mono">${b.lo}–${b.hi}</span></span>`).join("")}</div></section>
        <section class="panel"><h2>Findings <span class="muted">${findings.length} in ${new Set(findings.map((x) => x.a.id)).size} artifacts</span></h2>
          <div class="sevrow">${["high", "medium", "low"].map((l) => `<div><b class="${l}">${bySev[l]}</b><span>${l}</span></div>`).join("")}</div>
          <p class="sec" style="margin-top:10px">High findings drop the trust score below 60 and flag the artifact in the library.</p></section>
        <section class="panel"><h2>What artifacts can reach <span class="muted">permission transparency</span></h2>
          <div class="rstats">${statBtn("network", "network hosts", reach.network, "globe")}${statBtn("shell", "run shell commands", reach.shell, "terminal")}${statBtn("env", "read env vars", reach.env, "key-round")}${statBtn("any", "touch any file", reach.any, "files")}</div></section>
      </div>

      <h2 class="h">Findings</h2>
      ${findings.length ? `<table class="cmp sortable"><thead><tr>${H.th("sec", "severity", "Severity")}${H.th("sec", "artifact", "Artifact")}${H.th("sec", "type", "Type")}${H.th("sec", "finding", "Finding")}${H.th("sec", "detail", "Evidence")}${H.th("sec", "trust", "Trust")}<th></th></tr></thead><tbody>
        ${fRows.map(({ a, f }) => `<tr class="${f.level === "high" ? "flag" : ""}"><td>${sevBadge(f.level)}</td><td><button class="nm linkish" data-a="open" data-id="${a.id}">${esc(a.name)}</button></td><td>${H.T[a.type].label}</td><td>${esc(f.title)}</td><td class="mono">${esc(f.detail)}</td><td>${H.meter(a.trust)}</td><td style="text-align:right">${f.level === "high" ? `<button class="btn sm danger" data-a="remove-artifact" data-id="${a.id}">Remove</button>` : `<button class="btn sm" data-a="open" data-id="${a.id}">View</button>`}</td></tr>`).join("")}
      </tbody></table>` : `<div class="empty"><div class="ico">${ic("shield-check")}</div><h3>No findings</h3><p>Nothing in the library triggered an audit rule.</p></div>`}

      <h2 class="h">Reach ${rf !== "all" ? `<button class="btn ghost sm" data-a="reach-f" data-v="all">${ic("x")}Show all</button>` : ""}<span class="muted" style="font-weight:400">${rf === "all" ? "artifacts that reach beyond the project's own files" : "filtered: " + rf}</span></h2>
      <table class="cmp sortable"><thead><tr>${H.th("reach", "artifact", "Artifact")}${H.th("reach", "files", "Files")}${H.th("reach", "shell", "Shell")}${H.th("reach", "network", "Network")}${H.th("reach", "env", "Env vars")}${H.th("reach", "trust", "Trust")}</tr></thead><tbody>
        ${rRows.map((a) => `<tr><td><span style="display:flex;align-items:center;gap:8px">${H.typeIc(a.type, true)}<button class="nm linkish" data-a="open" data-id="${a.id}">${esc(a.name)}</button></span></td>${["files", "shell", "network", "env"].map((k) => `<td class="mono${reaches(a.reach[k]) && /any|all/.test(a.reach[k]) ? " hot" : ""}">${reaches(a.reach[k]) ? esc(a.reach[k]) : `<span class="muted">none</span>`}</td>`).join("")}<td>${H.meter(a.trust)}</td></tr>`).join("") || `<tr><td colspan="6" class="muted">None.</td></tr>`}
      </tbody></table>
    </div>`;
  };
  H.actions["reach-f"] = (el) => { S().flow.reachF = el.dataset.v === S().flow.reachF ? "all" : el.dataset.v; };

  // =================================================== SETTINGS TEMPLATES
  const list = (items, empty) => (items && items.length ? `<div class="codechips">${items.map((x) => `<span class="mono">${esc(x)}</span>`).join("")}</div>` : `<span class="muted">${empty || "none"}</span>`);
  H.settingsSummary = (a) => {
    const c = a.config;
    if (!c) return "";
    const bits = [`deny ${c.deny.length}`, `ask ${c.ask.length}`, `allow ${c.allow.length}`, H.plural(c.hooks.length, "hook")];
    if (c.statusLine) bits.push("status line");
    if (c.outputStyle) bits.push("style: " + c.outputStyle);
    if (c.plugins.length) bits.push(H.plural(c.plugins.length, "plugin"));
    return bits.join(" · ");
  };
  H.settingsDetail = (a) => {
    const c = a.config;
    const used = S().profiles.filter((p) => p.settings === a.id);
    return `<div class="dsec"><h3>Template contents <span class="muted" style="font-weight:400">one whole .claude/settings.json</span></h3>
      <div class="kv" style="grid-template-columns:96px 1fr;row-gap:10px">
        <span>Deny</span><span>${list(c.deny)}</span><span>Ask first</span><span>${list(c.ask)}</span><span>Allow</span><span>${list(c.allow)}</span>
        <span>Hooks</span><span>${c.hooks.length ? c.hooks.map((h) => `<div style="margin-bottom:4px"><span class="mono">${esc(h.event)}${h.matcher ? " · " + esc(h.matcher) : ""}</span> → <span class="mono">${esc(h.script)}</span><div class="muted">${esc(h.what)}; the script travels with the template</div></div>`).join("") : `<span class="muted">none</span>`}</span>
        <span>Status line</span><span>${c.statusLine ? `<span class="mono">${esc(c.statusLine)}</span>` : `<span class="muted">none</span>`}</span>
        <span>Output style</span><span>${c.outputStyle ? `<span class="mono">${esc(c.outputStyle)}</span>` : `<span class="muted">default</span>`}</span>
        <span>Plugins</span><span>${list(c.plugins)}</span><span>Env</span><span>${list(c.env)}</span>
        ${c.model ? `<span>Model</span><span class="mono">${esc(c.model)}</span>` : ""}
        <span>Imported from</span><span class="mono">${esc(c.source)}</span>
      </div>
      <div style="display:flex;gap:6px;margin-top:12px;flex-wrap:wrap"><button class="btn sm" data-a="settings-view" data-id="${a.id}">${ic("braces")}View settings.json</button><button class="btn sm" data-a="settings-dup" data-id="${a.id}">${ic("copy")}Duplicate</button></div>
      <p class="sec" style="margin-top:10px">${used.length ? `Used by ${used.map((p) => `<a href="#/profiles/${p.id}">${esc(p.name)}</a>`).join(", ")}.` : "Not used by any profile."} Edit the library copy in your own editor; projects get changes when they re-apply their profile.</p></div>`;
  };
  H.settingsJson = (a) => {
    const c = a.config;
    const arr = (xs) => "[" + xs.map((x) => JSON.stringify(x)).join(", ") + "]";
    return ["{", '  "permissions": {', `    "deny": ${arr(c.deny)},`, `    "ask": ${arr(c.ask)},`, `    "allow": ${arr(c.allow)}`, "  },"]
      .concat(c.hooks.length ? ['  "hooks": {'].concat(c.hooks.map((h, i) => `    "${h.event}": [{ "matcher": "${h.matcher}", "hooks": [{ "type": "command", "command": "${h.script}" }] }]${i < c.hooks.length - 1 ? "," : ""}`)).concat(["  },"]) : [])
      .concat(c.statusLine ? [`  "statusLine": { "type": "command", "command": "${c.statusLine}" },`] : [])
      .concat(c.outputStyle ? [`  "outputStyle": "${c.outputStyle}",`] : [])
      .concat(c.plugins.length ? [`  "enabledPlugins": { ${c.plugins.map((p) => `"${p}": true`).join(", ")} },`] : [])
      .concat(c.env.length ? [`  "env": { ${c.env.map((e) => { const [k, v] = e.split("="); return `"${k}": "${v}"`; }).join(", ")} },`] : [])
      .concat(c.model ? [`  "model": "${c.model}",`] : [])
      .concat([`  "$schema": "https://json.schemastore.org/claude-code-settings.json"`, "}"]);
  };
  H.actions["settings-view"] = (el) => {
    const a = H.art(el.dataset.id);
    const lines = H.settingsJson(a);
    H.modal({ title: `${a.name}: settings.json`, sub: `${H.settingsSummary(a)}. ${a.config.hooks.length ? "Hook scripts are copied with it: " + a.config.hooks.map((h) => h.script).join(", ") + "." : ""}`, wide: true,
      body: `<div class="code" style="border:1px solid var(--line);border-radius:8px">${H.codeHtml(".claude/settings.json", lines)}</div>`,
      foot: `<button class="btn" data-a="settings-dup" data-id="${a.id}">${ic("copy")}Duplicate</button><button class="btn primary" data-a="toast" data-msg="Opened the library copy of ${esc(a.name)} in $EDITOR">${ic("square-pen")}Open in editor</button>` });
  };
  H.actions["settings-dup"] = (el) => {
    const a = H.art(el.dataset.id);
    let name = a.name + "-copy", i = 2;
    while (S().artifacts.some((x) => x.name === name)) name = a.name + "-copy-" + i++;
    const copy = JSON.parse(JSON.stringify(a));
    Object.assign(copy, { id: H.uid("s"), name, source: "local", desc: "Copy of " + a.name + ". Edit it in your editor.", installedIn: [], latest: null, localEdits: false });
    copy.config.source = "~/.local/share/io.harneo/library/settings/" + name + "/settings.json";
    S().artifacts.unshift(copy);
    S().ui.modal = null;
    H.toast(`Created template ${esc(name)}. Opened it in your editor.`, { icon: "copy" });
    if (S().selected) S().selected = copy.id;
  };
})();
