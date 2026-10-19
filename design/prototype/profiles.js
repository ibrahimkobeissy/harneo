/* Harneo prototype: profiles list, profile builder ("shopping cart"), files a profile produces. */
(function () {
  "use strict";
  const H = window.H, esc = H.esc, ic = H.icon;
  const S = () => H.S;

  // ---------------------------------------------- files a profile produces
  const slug = (n) => n.replace(/^\//, "").replace(/\s.*$/, "");
  H.filesFor = function (p) {
    const files = [];
    const push = (path, kind, body, art) => files.push({ path, kind, body, art });
    const items = p.items.map(H.art).filter(Boolean);
    const cm = items.find((a) => a.type === "claudemd");
    push("CLAUDE.md", "claudemd", cm ? ["# Project", "", "@HARNESS.md", "", "## Build", "cargo build --workspace", "cargo test --workspace", "", "## Conventions", "- No unwrap() outside tests", "- thiserror in libraries, anyhow in binaries"] : ["# Project", "", "@HARNESS.md", "", "## Build", "<your build commands>"], cm);
    push("HARNESS.md", "harness", ["# Harness", "", `Installed by Harneo from profile **${p.name} v${p.version}** on 2026-10-20.`, "", "| Artifact | Type | Source | Why it's here |", "|---|---|---|---|"].concat(items.slice(0, 8).map((a) => `| ${a.name} | ${H.T[a.type].label} | ${H.SRC[a.source].label} | ${a.desc.split(".")[0]} |`)).concat(["", "Re-apply or switch profiles from Harneo; originals are backed up in Harneo's data folder."]));
    const set = H.art(p.settings);
    push(".claude/settings.json", "settings", ["{", '  "permissions": {', '    "deny": ["Read(./.env)", "Read(~/.ssh/**)", "Bash(rm -rf:*)"],', '    "ask": ["Bash(git push:*)"]', "  },", '  "hooks": { "PreToolUse": [{ "matcher": "Bash|Read", "hooks": [{ "type": "command", "command": "python3 .claude/hooks/guard.py" }] }] },', items.some((a) => a.type === "style") ? '  "outputStyle": "terse",' : "", items.some((a) => a.type === "plugin") ? '  "enabledPlugins": { "agent-skills@addy-agent-skills": true },' : "", '  "statusLine": { "type": "command", "command": ".claude/hooks/status.sh" }', "}"].filter(Boolean), set);
    if (set) { push(".claude/hooks/guard.py", "hook", ["#!/usr/bin/env python3", "# Blocks reads of secret files (from the strict-guard template)", "import json, sys", "..."], set); push(".claude/hooks/status.sh", "hook", ["#!/bin/sh", "echo \"$(git branch --show-current) · harneo\""], set); }
    items.forEach((a) => {
      const n = slug(a.name);
      if (a.type === "skill") push(`.claude/skills/${n}/SKILL.md`, "skill", ["---", `name: ${n}`, `description: ${a.desc}`, a.invocation === "manual" ? "disable-model-invocation: true" : "", "---", "", `# ${n}`, "", a.desc].filter((l) => l !== ""), a);
      if (a.type === "subagent") push(`.claude/agents/${n}.md`, "agent", ["---", `name: ${n}`, `description: ${a.desc}`, "tools: Read, Grep, Glob, Bash", "---", "", a.desc], a);
      if (a.type === "command") push(`.claude/commands/${n}.md`, "command", ["---", `description: ${a.desc}`, "---", "", a.desc], a);
      if (a.type === "rule") push(`.claude/rules/${n}.md`, "rule", ["---", "paths: [\"**/*.rs\"]", "---", "", a.desc], a);
      if (a.type === "style") push(`.claude/output-styles/${n}.md`, "style", ["---", `name: ${n}`, "---", "", a.desc], a);
      if (a.type === "workflow") push(`.claude/workflows/${n}.js`, "workflow", ["export const meta = { name: '" + n + "' }", "// orchestrates subagents"], a);
    });
    if (p.mcp.length) push(".mcp.json", "mcp", ["{", '  "mcpServers": {'].concat(p.mcp.map((m, i) => { const env = Object.entries(m.env).map(([k, v]) => `"${k}": "${v}"`).join(", "); return `    "${m.id}": { "command": "npx", "args": ["…"], "env": { ${env} } }${i < p.mcp.length - 1 ? "," : ""}`; })).concat(["  }", "}"]));
    return files;
  };
  H.treeHtml = function (files, selPath, action, marks) {
    let lastDir = null;
    return files.map((f) => {
      const parts = f.path.split("/");
      const dir = parts.slice(0, -1).join("/");
      let out = "";
      if (dir && dir !== lastDir) { out += `<div class="tnode" style="padding-left:${12 + (parts.length - 2) * 12}px;color:var(--t2)">${ic("folder")}${esc(dir)}/</div>`; }
      lastDir = dir;
      const m = marks ? marks[f.path] : null;
      out += `<button class="tnode${selPath === f.path ? " on" : ""}" style="padding-left:${12 + (dir ? (parts.length - 1) * 12 : 0)}px" data-a="${action}" data-path="${esc(f.path)}">${ic(f.path.endsWith(".json") ? "braces" : f.path.endsWith(".js") || f.path.endsWith(".py") || f.path.endsWith(".sh") ? "file-code" : "file-text")}${esc(parts[parts.length - 1])}${m ? `<span class="${m[0]}">${m[1]}</span>` : ""}</button>`;
      return out;
    }).join("");
  };
  H.codeHtml = function (path, lines, diff) {
    return `<div class="fh">${ic("file-text")}${esc(path)}<span class="muted" style="margin-left:auto">${lines.length} lines</span></div><pre>${lines.map((l, i) => {
      const t = typeof l === "string" ? [" ", l] : l;
      return `<div class="ln${t[0] === "+" ? " add" : t[0] === "-" ? " del" : ""}"><span class="no">${i + 1}</span>${diff ? `<span class="mk">${t[0] === " " ? "" : t[0]}</span>` : ""}<span>${esc(t[1])}</span></div>`;
    }).join("")}</pre>`;
  };
  function typeCounts(p) {
    const c = {};
    p.items.map(H.art).filter(Boolean).forEach((a) => { c[a.type] = (c[a.type] || 0) + 1; });
    if (p.settings) c.settings = 1;
    if (p.mcp.length) c.mcp = p.mcp.length;
    return Object.entries(c).map(([t, n]) => `<span>${H.typeIc(t, true)}${n} ${n === 1 ? H.T[t].label.toLowerCase() : H.T[t].plural.toLowerCase()}</span>`).join("");
  }

  // ----------------------------------------------------------- profiles
  H.screens.profiles = function (r) {
    if (r.id) return builder(r.id);
    const list = S().profiles;
    return `<div class="page">
      <div class="ph"><h1>Profiles</h1><span class="muted">A profile is a named selection of library artifacts that gets installed into projects.</span><span class="grow"></span>
        <button class="btn" data-a="profile-import">${ic("file-up")}Import profile file</button><button class="btn primary" data-a="profile-new">${ic("plus")}New profile <kbd>N</kbd></button></div>
      ${list.length ? `<div class="cards3">${list.map((p) => `<div class="pcard">
        <a class="top" href="#/profiles/${p.id}" style="color:inherit;text-decoration:none"><span class="ic t-blue">${ic("layers")}</span><div style="flex:1;min-width:0"><h3>${esc(p.name)} <span class="mono muted">v${p.version}</span>${p.dirty ? ` <span class="badge warn">Unsaved</span>` : ""}</h3><div class="sec">${esc(p.desc)}</div></div></a>
        <div class="typecounts">${typeCounts(p)}</div>
        <div class="foot">${ic("folder-git-2")}<span>${p.projects.length ? `Used in ${H.plural(p.projects.length, "project")}` : "Not used yet"}</span><span class="grow"></span>
          <button class="btn ghost sm" data-a="profile-clone" data-id="${p.id}" title="Clone">${ic("copy")}Clone</button><button class="btn ghost sm" data-a="profile-export" data-id="${p.id}" title="Export">${ic("file-down")}Export</button></div></div>`).join("")}</div>`
      : `<div class="empty"><div class="ico">${ic("layers")}</div><h3>No profiles yet</h3><p>Pick artifacts from your library into a profile, then scaffold or refit projects with it.</p><button class="btn primary" data-a="profile-new">${ic("plus")}New profile</button></div>`}
    </div>`;
  };
  const newProfile = (o) => { const p = Object.assign({ id: H.uid("p"), name: "Untitled profile", version: 1, desc: "", settings: (S().artifacts.find((a) => a.type === "settings") || {}).id || null, mcp: [], items: [], projects: [], dirty: true, isNew: true }, o); S().profiles.push(p); return p; };
  H.actions["profile-new"] = () => { const p = newProfile(); location.hash = "#/profiles/" + p.id; };
  H.actions["profile-clone"] = (el) => { const src = H.profile(el.dataset.id); const p = newProfile({ name: src.name + " copy", desc: "Cloned from " + src.name + " v" + src.version, settings: src.settings, mcp: JSON.parse(JSON.stringify(src.mcp)), items: src.items.slice() }); location.hash = "#/profiles/" + p.id; H.toast(`Cloned ${esc(src.name)}. Add or remove artifacts, then save.`); };
  H.actions["profile-export"] = (el) => { const p = H.profile(el.dataset.id); H.toast(`Exported <span class="mono">${esc(p.name.toLowerCase().replace(/\s+/g, "-"))}.harneo-profile.json</span> (${p.items.length} artifacts, secrets as \${VAR} only)`, { icon: "file-down" }); return false; };
  H.actions["profile-import"] = () => { const p = newProfile({ name: "Team default", version: 1, desc: "Imported from team-default.harneo-profile.json", items: ["tdd", "code-reviewer", "test-engineer", "incremental-implementation", "review-cmd"], dirty: false, isNew: false }); H.toast(`Imported “${esc(p.name)}” v1. 5 artifacts matched your library.`, { icon: "file-up" }); };
  H.actions["profile-from-collection"] = (el) => {
    const c = S().collections.find((x) => x.id === el.dataset.id);
    const ids = [];
    H.collectionItems(c).forEach((a) => { if (a.type === "settings" || a.type === "mcp") return; [a.id].concat(a.bundle).forEach((x) => { if (!ids.includes(x)) ids.push(x); }); });
    const p = newProfile({ name: c.name, desc: "From collection " + c.name, items: ids });
    location.hash = "#/profiles/" + p.id;
    H.toast(`Pre-filled from “${esc(c.name)}”, bundles included. Review, then save.`);
  };

  // ------------------------------------------------------------ builder
  function builder(id) {
    const p = H.profile(id);
    if (!p) return `<div class="page"><div class="empty"><h3>Profile not found</h3><a class="btn" href="#/profiles">Back to profiles</a></div></div>`;
    const f = S().flow.pk || (S().flow.pk = { q: "", type: "all" });
    const pool = S().artifacts.filter((a) => a.type !== "settings" && a.type !== "mcp" && (f.type === "all" || a.type === f.type) && (!f.q || a.name.toLowerCase().includes(f.q.toLowerCase()))).sort((a, b) => b.sessions - a.sessions).slice(0, 40);
    const files = H.filesFor(p);
    const sel = S().flow.bsel && files.some((x) => x.path === S().flow.bsel) ? S().flow.bsel : null;
    const items = p.items.map(H.art).filter(Boolean);
    const counts = {};
    items.forEach((a) => { counts[a.type] = (counts[a.type] || 0) + 1; });
    const settingsOpts = S().artifacts.filter((a) => a.type === "settings");
    const mcpOpts = S().artifacts.filter((a) => a.type === "mcp" && !p.mcp.some((m) => m.id === a.id));
    const err = S().flow.envErr;
    return `<div class="builder">
      <div class="picker">
        <div class="crumb"><a href="#/profiles" class="muted">${ic("arrow-left")} Profiles</a></div>
        <div class="ph" style="margin-bottom:12px"><h1>Pick from your library</h1><span class="muted">Adding an artifact adds its whole bundle.</span></div>
        <div class="toolbar"><div class="search" style="width:260px"><label class="sr" for="pkq">Search library</label>${ic("search")}<input class="input" id="pkq" placeholder="Search library" value="${esc(f.q)}" data-in="pkq" autocomplete="off"></div>
          <div class="seg" role="group" aria-label="Type">${[["all", "All"], ["skill", "Skills"], ["subagent", "Subagents"], ["command", "Commands"], ["rule", "Rules"], ["plugin", "Plugins"], ["style", "Styles"], ["claudemd", "CLAUDE.md"]].map(([v, l]) => `<button class="${f.type === v ? "on" : ""}" data-a="pktype" data-v="${v}">${l}</button>`).join("")}</div></div>
        <div class="pgrid">${pool.map((a) => { const inP = p.items.includes(a.id); return `<div class="prow${inP ? " in" : ""}">${H.typeIc(a.type, true, H.status(a) === "unused")}<div><div class="nm" style="font-size:13px">${esc(a.name)}</div><div class="srcline">${a.bundle.length ? "bundle of " + (a.bundle.length + 1) + " · " : ""}${esc(H.SRC[a.source].label)} · trust ${a.trust}</div></div>
          ${inP ? `<button class="btn sm" data-a="pk-remove" data-id="${a.id}" aria-label="Remove ${esc(a.name)}">${ic("check")}Added</button>` : `<button class="btn sm" data-a="pk-add" data-id="${a.id}" aria-label="Add ${esc(a.name)}">${ic("plus")}Add</button>`}</div>`; }).join("") || `<p class="muted">No matches.</p>`}</div>
      </div>
      <aside class="cart" aria-label="Profile">
        <div class="field" style="margin:0"><label for="pname">Profile name</label><input class="input" id="pname" value="${esc(p.name)}" data-in="pname"></div>
        <div style="display:flex;align-items:center;gap:8px"><span class="mono muted">${p.isNew ? "new · v1" : p.dirty ? `v${p.version} → v${p.version + 1} on save` : "v" + p.version + " · saved"}</span><span class="grow" style="flex:1"></span><button class="btn primary" data-a="profile-save" data-id="${p.id}"${p.dirty ? "" : " disabled"}>${ic("save")}Save</button></div>
        <div class="panel"><h2>Settings template <span class="muted">exactly one</span></h2><div class="optlist">${settingsOpts.map((a) => `<button class="opt${p.settings === a.id ? " on" : ""}" data-a="pk-settings" data-id="${a.id}"><i class="radio"></i><div style="min-width:0"><b>${esc(a.name)}</b><div class="sec" style="font-size:12px">${esc(a.desc)}</div></div></button>`).join("")}</div></div>
        <div class="panel"><h2>MCP servers <span class="muted">secrets as \${VAR} only</span></h2>
          ${p.mcp.map((m) => { const a = H.art(m.id); return `<div style="margin-bottom:10px"><div style="display:flex;align-items:center;gap:8px">${H.typeIc("mcp", true)}<b>${esc(a ? a.name : m.id)}</b><button class="btn ghost sm" style="margin-left:auto" data-a="pk-mcp-remove" data-id="${m.id}" aria-label="Remove server">${ic("x")}</button></div>
            ${Object.entries(m.env).map(([k, v]) => `<div class="envrow"><span class="mono">${esc(k)}</span><input class="input mono" style="height:28px" value="${esc(v)}" data-ch="pk-env" data-m="${m.id}" data-k="${esc(k)}" id="env-${m.id}-${esc(k)}" aria-label="${esc(k)} value"></div>${err && err.m === m.id && err.k === k ? `<div class="callout warn" style="margin-top:6px;padding:8px 10px">${ic("shield-alert")}<p>That looked like a real secret, so it wasn't saved. Kept <span class="mono">${esc(v)}</span>; Harneo stores placeholders only.</p></div>` : ""}`).join("")}</div>`; }).join("") || `<p class="muted" style="margin-bottom:8px">No servers.</p>`}
          ${mcpOpts.length ? `<label class="sr" for="mcpadd">Add MCP server</label><select class="sel" id="mcpadd" data-ch="pk-mcp-add" style="width:100%"><option value="">Add a server…</option>${mcpOpts.map((a) => `<option value="${a.id}">${esc(a.name)}</option>`).join("")}</select>` : ""}</div>
        <div class="panel"><h2>The project it produces <span class="muted">${files.length} files</span></h2>
          <div class="tree" id="treeview" style="background:transparent;border:0;padding:0;max-height:340px">${H.treeHtml(files, sel, "pk-file")}</div>
          ${sel ? (() => { const fl = files.find((x) => x.path === sel); const a = fl.art; return `<div class="callout" style="margin-top:10px">${ic("file-text")}<div style="flex:1;min-width:0"><b class="mono" style="font-size:12px">${esc(sel)}</b><p>${a ? `From ${esc(a.name)} · ${esc(H.SRC[a.source].label)}` : "Generated by Harneo"}</p>${a && a.type === "skill" ? `<div class="switchrow" style="margin-top:6px"><div class="sec">Manual only</div><button class="toggle${a.invocation === "manual" ? " on" : ""}" data-a="toggle-inv" data-id="${a.id}" role="switch" aria-checked="${a.invocation === "manual"}" aria-label="Manual only"><i></i></button></div>` : ""}</div></div>`; })() : ""}</div>
        <div class="panel"><h2>Totals</h2><div class="totals">${Object.entries(counts).map(([t, n]) => `<span>${H.T[t].plural}</span><span>${n}</span>`).join("")}<span>Settings template</span><span>${p.settings ? 1 : 0}</span><span>MCP servers</span><span>${p.mcp.length}</span><span class="sum">Files written</span><span class="sum">${files.length}</span></div></div>
        ${p.projects.length ? `<div class="callout info">${ic("info")}<p>Used by ${p.projects.map((x) => esc((H.project(x) || { name: x }).name)).join(", ")}. Saving creates v${p.version + 1}; projects keep v${p.version} until you switch them from their page.</p></div>` : ""}
      </aside></div>`;
  }
  const cur = () => H.profile(H.route().id);
  H.inputs.pkq = (v) => { S().flow.pk.q = v; H.render(); };
  H.actions.pktype = (el) => { S().flow.pk.type = el.dataset.v; };
  H.inputs.pname = (v) => { const p = cur(); p.name = v; p.dirty = true; H.render(); };
  H.actions["pk-add"] = (el) => {
    const p = cur(), a = H.art(el.dataset.id);
    const added = [a.id].concat(a.bundle).filter((x) => !p.items.includes(x));
    p.items.push(...added); p.dirty = true;
    if (a.findings.length && a.trust < 60) H.toast(`Added ${esc(a.name)}. Trust ${a.trust}: ${a.findings.length} security findings.`, { icon: "shield-alert" });
    else H.toast(`Added ${esc(a.name)}${added.length > 1 ? ` + ${added.length - 1} bundled` : ""}`);
  };
  H.actions["pk-remove"] = (el) => {
    const p = cur(), a = H.art(el.dataset.id);
    const parents = p.items.map(H.art).filter((x) => x && x.bundle.includes(a.id));
    const doRemove = () => { p.items = p.items.filter((x) => x !== a.id); p.dirty = true; };
    if (!parents.length) return doRemove();
    H.confirm({ title: `${a.name} is part of a bundle`, ok: `Remove ${a.name} and ${parents.map((x) => x.name).join(", ")}`, danger: true,
      body: `<p class="sec">${parents.map((x) => `<b>${esc(x.name)}</b>`).join(", ")} ${parents.length === 1 ? "needs" : "need"} it. Removing only ${esc(a.name)} would ship a half-wired profile, so the whole bundle goes.</p>`,
      onOk: () => { const drop = [a.id].concat(parents.map((x) => x.id)); p.items = p.items.filter((x) => !drop.includes(x)); p.dirty = true; } });
  };
  H.actions["pk-settings"] = (el) => { const p = cur(); p.settings = el.dataset.id; p.dirty = true; };
  H.actions["pk-mcp-remove"] = (el) => { const p = cur(); p.mcp = p.mcp.filter((m) => m.id !== el.dataset.id); p.dirty = true; };
  H.inputs["pk-mcp-add"] = (v) => {
    if (!v) return;
    const p = cur(), a = H.art(v);
    const env = {};
    (a.reach.env !== "none" ? a.reach.env.split(/,\s*/) : ["API_TOKEN"]).forEach((k) => { env[k] = "${" + k + "}"; });
    p.mcp.push({ id: v, env }); p.dirty = true;
  };
  H.inputs["pk-env"] = (v, el) => {
    const p = cur(), m = p.mcp.find((x) => x.id === el.dataset.m), k = el.dataset.k;
    const ok = /^\$\{[A-Z0-9_]+\}$/.test(v.trim());
    S().flow.envErr = ok ? null : { m: m.id, k };
    if (ok) { m.env[k] = v.trim(); p.dirty = true; }
  };
  H.actions["pk-file"] = (el) => { S().flow.bsel = S().flow.bsel === el.dataset.path ? null : el.dataset.path; };
  H.actions["profile-save"] = (el) => {
    const p = H.profile(el.dataset.id);
    S().flow.envErr = null;
    if (!p.items.length) return H.toast("Add at least one artifact before saving.", { icon: "circle-alert" });
    if (p.isNew) { p.isNew = false; } else p.version += 1;
    p.dirty = false;
    H.toast(`Saved ${esc(p.name)} v${p.version}`);
  };
})();
