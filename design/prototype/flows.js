/* Harneo prototype: import (and first-run import), scaffold, refit. */
(function () {
  "use strict";
  const H = window.H, esc = H.esc, ic = H.icon;
  const S = () => H.S;
  const D = window.HARNEO_DATA;

  function steps(list, at) {
    return `<ol class="steps" aria-label="Progress">${list.map((l, i) => `${i ? `<span class="stepline" aria-hidden="true"></span>` : ""}<li class="step${i === at ? " on" : i < at ? " done" : ""}"${i === at ? ' aria-current="step"' : ""}><span class="num">${i < at ? ic("check") : i + 1}</span>${esc(l)}</li>`).join("")}</ol>`;
  }
  const later = (ms, fn) => setTimeout(() => { fn(); H.render(); }, ms);

  // ============================================================== IMPORT
  const imp = () => S().flow.imp || (S().flow.imp = { step: 0, source: "claude", url: "", path: "", err: null, busy: false, items: [], bundles: [], tagSugg: {} });
  H.screens.import = function () {
    const f = imp();
    const first = S().firstRun;
    let body = "";
    if (f.step === 0) {
      const ch = (k, icon, title, sub) => `<button class="choice${f.source === k ? " on" : ""}" data-a="imp-src" data-v="${k}" aria-pressed="${f.source === k}"><span class="ic t-blue">${ic(icon)}</span><b>${title}</b><span class="sec">${sub}</span></button>`;
      body = `<div class="choices">${ch("claude", "house", "Your ~/.claude", "Skills, commands, agents and settings you already use. 6 artifacts detected.")}${ch("git", "git-branch", "A git URL", "A public pack like Addy's agent-skills. Harneo records URL, branch, subfolder and commit.")}${ch("folder", "folder-open", "A local folder", "Any repo or folder with a .claude/ directory or loose artifacts.")}</div>
        ${f.source === "git" ? `<div class="field" style="margin-top:18px"><label for="impurl">Repository URL</label><input class="input mono" id="impurl" placeholder="github.com/owner/repo" value="${esc(f.url)}" data-in="imp-url" autocomplete="off">
          ${f.err ? `<span class="err">${ic("circle-alert")}${f.err}</span>` : `<span class="hint">Optionally add a branch and subfolder: <span class="mono">github.com/owner/repo/tree/main/skills</span></span>`}
          <div style="display:flex;gap:6px;flex-wrap:wrap;margin-top:4px"><span class="muted">Try:</span>${["github.com/addyosmani/agent-skills", "github.com/mattpocock/skills", "github.com/nope/does-not-exist"].map((u) => `<button class="btn sm" data-a="imp-url-pick" data-v="${u}">${esc(u.replace("github.com/", ""))}</button>`).join("")}</div></div>` : ""}
        ${f.source === "folder" ? `<div class="field" style="margin-top:18px"><label for="imppath">Folder</label><div style="display:flex;gap:8px"><input class="input mono" id="imppath" value="${esc(f.path)}" placeholder="~/code/my-team-harness" data-in="imp-path"><button class="btn" data-a="imp-browse">${ic("folder-search")}Browse…</button></div>
          ${f.err ? `<span class="err">${ic("circle-alert")}${f.err}</span>` : `<span class="hint">Try <span class="mono">~/code/empty-dir</span> to see the “nothing found” state.</span>`}</div>` : ""}`;
    } else if (f.step === 1) {
      if (f.busy) body = `<div class="panel" style="display:flex;gap:12px;align-items:center">${'<span class="spinner"></span>'}<div><b>Scanning ${esc(srcName(f))}…</b><div class="sec">Looking for the 10 artifact types: skills, commands, subagents, settings, MCP servers, rules, output styles, workflows, plugins, CLAUDE.md.</div></div></div>`;
      else {
        const groups = {};
        f.items.forEach((it, i) => { (groups[it.type] = groups[it.type] || []).push([it, i]); });
        const n = f.items.filter((x) => x.on).length;
        body = `<p class="sec" style="margin-bottom:12px">Found ${H.plural(f.items.length, "artifact")} in <span class="mono">${esc(srcName(f))}</span>. Tick what to import.</p>
          ${Object.entries(groups).map(([t, list]) => `<div class="group"><header>${H.typeIc(t, true)}<span>${H.T[t].plural}</span><span class="n">${list.length}</span><span class="grow"></span><button class="btn ghost sm" data-a="imp-all" data-t="${t}">Select all</button></header>
            ${list.map(([it, i]) => `<button class="row" style="width:100%;text-align:left" data-a="imp-tick" data-i="${i}" aria-pressed="${it.on}"><i class="cb${it.on ? " on" : ""}">${it.on ? ic("check") : ""}</i><div class="grow"><b>${esc(it.name)}</b><div class="srcline">${esc(it.note)}</div></div>${it.exists ? `<span class="badge">Already in library</span>` : ""}</button>`).join("")}</div>`).join("")}
          <p class="muted">${H.plural(n, "artifact")} selected.</p>`;
      }
    } else if (f.step === 2) {
      if (f.busy) body = `<div class="panel"><div style="display:flex;gap:12px;align-items:center;margin-bottom:12px"><span class="spinner"></span><div><b>Claude is reading ${H.plural(f.items.filter((x) => x.on).length, "artifact")}…</b><div class="sec">Detecting bundles and suggesting tags through your <span class="mono">claude</span> CLI. Running the security audit.</div></div></div><div class="progress"><i style="width:${f.prog || 10}%"></i></div></div>`;
      else {
        const sel = f.items.filter((x) => x.on);
        body = `${!S().claudeCli ? `<div class="callout warn" style="margin-bottom:14px">${ic("triangle-alert")}<div><b>Claude CLI not found.</b><p>Bundles and tags weren't suggested. Import now and add them by hand from each artifact, or install the CLI and re-run detection later.</p></div></div>` : ""}
          ${S().claudeCli ? `<h2 class="h" style="margin-top:0">Proposed bundles <span class="muted" style="font-weight:400">artifacts that only work together</span></h2>
          ${f.bundles.map((b, bi) => `<div class="group"><header>${ic("boxes")}<span>${esc(b.lead)}</span><span class="n">+ ${b.members.length}</span><span class="grow"></span><button class="toggle${b.on ? " on" : ""}" data-a="imp-bundle" data-i="${bi}" role="switch" aria-checked="${b.on}" aria-label="Accept bundle"><i></i></button></header>
            <div class="row"><span class="sec">${esc(b.why)}</span></div><div class="row" style="flex-wrap:wrap;gap:6px">${b.members.map((m, mi) => `<span class="tag t-slate">${esc(m)}<button data-a="imp-bundle-drop" data-i="${bi}" data-m="${mi}" aria-label="Remove ${esc(m)} from bundle">${ic("x")}</button></span>`).join("")}</div></div>`).join("") || `<p class="muted">No bundles found: these artifacts don't depend on each other.</p>`}
          <h2 class="h">Suggested tags <span class="muted" style="font-weight:400">accept, remove, or add your own later</span></h2>
          <div class="group">${sel.map((it) => `<div class="row"><b style="width:220px;flex:none;overflow:hidden;text-overflow:ellipsis">${esc(it.name)}</b><div class="grow" style="display:flex;gap:6px;flex-wrap:wrap">${(f.tagSugg[it.name] || []).map((t) => `<span class="tag t-${H.tagColor(t)}"><i class="d"></i>${esc(t)}<button data-a="imp-tag-drop" data-n="${esc(it.name)}" data-t="${esc(t)}" aria-label="Remove ${esc(t)}">${ic("x")}</button></span>`).join("") || `<span class="muted">no suggestion</span>`}</div></div>`).join("")}</div>` : ""}
          <h2 class="h">Security audit</h2>
          <div class="group">${sel.map((it) => `<div class="row${it.trust < 60 ? " flag" : ""}">${H.typeIc(it.type, true)}<div class="grow"><b>${esc(it.name)}</b>${it.trust < 60 ? `<div class="sec" style="color:var(--red)">Pipes a downloaded script into sh; unrestricted Bash.</div>` : ""}</div>${H.meter(it.trust)}${it.trust < 60 ? `<span class="badge bad">2 findings</span>` : `<span class="badge ok">No findings</span>`}</div>`).join("")}</div>`;
      }
    }
    const labels = ["Source", "Choose artifacts", "Review"];
    const canNext = f.step === 0 ? (f.source === "claude" || (f.source === "git" && f.url) || (f.source === "folder" && f.path)) : f.step === 1 ? !f.busy && f.items.some((x) => x.on) : !f.busy;
    return `<div class="page narrow">
      ${first ? "" : `<div class="crumb"><a href="#/library" class="muted">${ic("arrow-left")} Library</a></div>`}
      <div class="ph"><h1>${first ? "Import your first artifacts" : "Import"}</h1></div>
      ${first && f.step === 0 ? `<div class="callout info" style="margin-bottom:16px">${ic("info")}<p>Importing copies artifacts into Harneo's library. Nothing in your projects or <span class="mono">~/.claude</span> changes.</p></div>` : ""}
      ${steps(labels, f.step)}${body}
      <div class="flowfoot">${f.step ? `<button class="btn" data-a="imp-back"${f.busy ? " disabled" : ""}>Back</button>` : first ? `<button class="btn ghost" data-a="welcome-skip">Skip for now</button>` : ""}<span class="grow"></span>
        <button class="btn primary" data-a="imp-next"${canNext ? "" : " disabled"}>${f.step === 2 ? ic("download") + "Import " + H.plural(f.items.filter((x) => x.on).length, "artifact") : "Continue"}</button></div></div>`;
  };
  const srcName = (f) => (f.source === "claude" ? "~/.claude" : f.source === "git" ? f.url : f.path);
  // ~/.claude artifacts are already in use: their history comes from the session logs.
  // Artifacts new to this machine start as "not used yet", never as unused.
  function usageFor(f, it) {
    if (f.source === "claude" && H.usageTypes.includes(it.type)) {
      const base = 2 + (it.name.length % 4);
      const usage = [0, 1, 2, 3, 4, 5, 6, 7].map((w) => base + Math.round(w * 0.6) + ((w * it.name.length) % 3));
      return { usage, sessions: usage.reduce((s, n) => s + n, 0), lastUsed: "today", fresh: false };
    }
    return { usage: [0, 0, 0, 0, 0, 0, 0, 0], sessions: 0, lastUsed: "not used yet", fresh: true };
  }
  H.actions["imp-src"] = (el) => { const f = imp(); f.source = el.dataset.v; f.err = null; };
  H.inputs["imp-url"] = (v) => { const f = imp(); f.url = v; f.err = null; H.render(); };
  H.actions["imp-url-pick"] = (el) => { const f = imp(); f.url = el.dataset.v; f.err = null; };
  H.inputs["imp-path"] = (v) => { const f = imp(); f.path = v; f.err = null; H.render(); };
  H.actions["imp-browse"] = () => { const f = imp(); f.path = "~/code/team-harness"; f.err = null; H.toast("Picked ~/code/team-harness (system folder dialog in the real app)", { icon: "folder-open" }); };
  H.actions["imp-back"] = () => { const f = imp(); f.step -= 1; f.err = null; };
  H.actions["imp-tick"] = (el) => { const it = imp().items[+el.dataset.i]; it.on = !it.on; };
  H.actions["imp-all"] = (el) => { imp().items.filter((x) => x.type === el.dataset.t && !x.exists).forEach((x) => { x.on = true; }); };
  H.actions["imp-bundle"] = (el) => { const b = imp().bundles[+el.dataset.i]; b.on = !b.on; };
  H.actions["imp-bundle-drop"] = (el) => { const b = imp().bundles[+el.dataset.i]; b.members.splice(+el.dataset.m, 1); if (!b.members.length) imp().bundles.splice(+el.dataset.i, 1); };
  H.actions["imp-tag-drop"] = (el) => { const f = imp(); f.tagSugg[el.dataset.n] = f.tagSugg[el.dataset.n].filter((t) => t !== el.dataset.t); };
  H.actions["imp-next"] = () => {
    const f = imp();
    if (f.step === 0) {
      if (f.source === "git" && !/^(https:\/\/)?(github\.com|gitlab\.com)\/[\w.-]+\/[\w.-]+/.test(f.url.trim())) { f.err = "That doesn't look like a repository URL. Use github.com/owner/repo."; return; }
      if (f.source === "git" && /nope|does-not-exist/.test(f.url)) { f.err = "Repository not found or private: github.com/nope/does-not-exist. Check the URL, or your git credentials for private repos."; return; }
      if (f.source === "folder" && /empty/.test(f.path)) { f.err = `No artifacts found in ${esc(f.path)}. Harneo looks for .claude/, CLAUDE.md, .mcp.json and loose SKILL.md files.`; return; }
      f.step = 1; f.busy = true;
      const raw = f.source === "claude" ? D.SCAN.claudeHome : f.source === "git" ? D.SCAN.git : D.SCAN.claudeHome.slice(0, 3).concat([{ name: "deploy-anywhere", type: "skill", note: "skills/deploy-anywhere" }]);
      f.items = raw.map((r) => { const exists = S().artifacts.some((a) => a.name === r.name); return Object.assign({}, r, { exists, on: !exists, trust: r.name === "deploy-anywhere" ? 38 : 80 + (r.name.length % 18) }); });
      return later(1100, () => { f.busy = false; });
    }
    if (f.step === 1) {
      f.step = 2; f.busy = true; f.prog = 12;
      const sel = f.items.filter((x) => x.on).map((x) => x.name);
      f.bundles = [];
      if (sel.includes("test-driven-development") || sel.includes("tdd-lite")) f.bundles.push({ lead: sel.includes("tdd-lite") ? "tdd-lite" : "test-driven-development", members: sel.filter((n) => ["test-engineer", "code-reviewer", "reviewer", "incremental-implementation"].includes(n)), why: "Its SKILL.md tells Claude to hand off to these subagents and skills.", on: true });
      if (sel.includes("/review")) f.bundles.push({ lead: "/review", members: sel.filter((n) => ["code-reviewer", "code-review-and-quality"].includes(n)), why: "The command invokes the code-reviewer subagent with the code-review skill.", on: true });
      f.bundles = f.bundles.filter((b) => b.members.length);
      const guess = { skill: ["daily"], subagent: ["review"], command: ["daily"], settings: ["security"], mcp: ["daily"], style: ["daily"] };
      f.tagSugg = {};
      f.items.filter((x) => x.on).forEach((x) => { f.tagSugg[x.name] = /test|tdd/.test(x.name) ? ["testing"] : /review/.test(x.name) ? ["review"] : /secur|deploy/.test(x.name) ? ["security"] : /front/.test(x.name) ? ["frontend"] : /spec/.test(x.name) ? ["planning", "docs"] : guess[x.type] || []; });
      setTimeout(() => { f.prog = 64; H.render(); }, 600);
      return later(1500, () => { f.busy = false; });
    }
    // step 2: import
    const sel = f.items.filter((x) => x.on);
    const srcKey = f.source === "git" ? (/mattpocock/.test(f.url) ? "matt" : "addy") : "local";
    sel.forEach((it) => {
      if (S().artifacts.some((a) => a.name === it.name)) return;
      const b = S().claudeCli ? f.bundles.find((x) => x.on && x.lead === it.name) : null;
      S().artifacts.unshift({ id: H.uid("a"), name: it.name, type: it.type, source: srcKey, desc: "Imported from " + srcName(f) + ": " + it.note + ".", trust: it.trust, findings: it.trust < 60 ? [{ level: "high", title: "Remote code execution pattern", detail: "curl … | sh" }, { level: "high", title: "Unrestricted shell", detail: "allowed-tools: Bash(*)" }] : [], bundle: b ? b.members.map((m) => (S().artifacts.find((a) => a.name === m) || { id: m }).id) : [], tags: S().claudeCli ? (f.tagSugg[it.name] || []) : [], installedIn: [], reach: { files: "project files", shell: "none", network: "none", env: "none" }, invocation: it.type === "skill" || it.type === "subagent" ? "auto" : null, ...usageFor(f, it), latest: null, localEdits: false });
    });
    const n = sel.length;
    S().flow.imp = null;
    S().firstRun = false;
    location.hash = "#/library";
    H.toast(`Imported ${H.plural(n, "artifact")} into your library${S().claudeCli ? " with bundles and tags" : ""}`);
  };

  // ============================================================ SCAFFOLD
  const FOLDERS = [
    { path: "~/work/new-app", note: "Empty folder", empty: true },
    { path: "~/work/landing", note: "package.json, src/ · no harness", empty: true },
    { path: "~/work/site", note: ".claude/, CLAUDE.md · already has a harness", empty: false },
    { path: "~/readonly/demo", note: "Read-only (simulates a write failure)", empty: true, ro: true },
  ];
  const sc = () => S().flow.sc || (S().flow.sc = { step: 0, path: "", err: null, profile: null, sel: "HARNESS.md", busy: false, failed: null });
  H.screens.scaffold = function () {
    const f = sc();
    const p = f.profile ? H.profile(f.profile) : null;
    let body = "";
    if (f.step === 0) {
      body = `<div class="field"><label for="scpath">New project folder</label><div style="display:flex;gap:8px"><input class="input mono" id="scpath" value="${esc(f.path)}" placeholder="~/work/new-app" data-in="sc-path"><button class="btn" data-a="sc-browse">${ic("folder-search")}Browse…</button></div>
        ${f.err ? `<span class="err">${ic("circle-alert")}${f.err}</span>${f.err.includes("already has") ? `<div><a class="btn sm" href="#/refit" data-a="sc-to-refit">${ic("refresh-cw")}Refit it instead</a></div>` : ""}` : `<span class="hint">An empty folder, or a project with no Claude harness yet. Existing harnesses are refitted, not scaffolded.</span>`}</div>
        ${S().flow.browse ? `<div class="group"><header>${ic("folder-search")}<span>Choose a folder</span><span class="grow"></span><span class="muted">system dialog in the real app</span></header>${FOLDERS.map((x) => `<button class="row" style="width:100%;text-align:left" data-a="sc-pick" data-v="${x.path}">${ic("folder")}<span class="mono grow">${x.path}</span><span class="muted">${x.note}</span></button>`).join("")}</div>` : ""}`;
    } else if (f.step === 1) {
      body = S().profiles.length ? `<div class="choices">${S().profiles.map((x) => `<button class="choice${f.profile === x.id ? " on" : ""}" data-a="sc-profile" data-v="${x.id}" aria-pressed="${f.profile === x.id}"><span class="ic t-blue">${ic("layers")}</span><b>${esc(x.name)} <span class="mono muted">v${x.version}</span></b><span class="sec">${esc(x.desc)}</span><span class="muted" style="font-size:12px">${H.plural(H.filesFor(x).length, "file")}</span></button>`).join("")}</div>` : `<div class="empty"><h3>No profiles yet</h3><a class="btn primary" href="#/profiles">Create a profile</a></div>`;
    } else if (f.step === 2) {
      const files = H.filesFor(p);
      const fl = files.find((x) => x.path === f.sel) || files[0];
      const marks = {}; files.forEach((x) => { marks[x.path] = ["new", "new"]; });
      body = `${f.failed ? `<div class="callout bad" style="margin-bottom:14px">${ic("octagon-alert")}<div><b>Permission denied: ${esc(f.path)}/.claude</b><p>Nothing was written. Pick a folder you can write to, or fix its permissions and try again.</p></div></div>` : `<div class="callout info" style="margin-bottom:14px">${ic("eye")}<p><b>Dry run.</b> These ${files.length} files will be written to <span class="mono">${esc(f.path)}</span>. Nothing exists there yet, so nothing is replaced. Click any file to read its full content.</p></div>`}
        <div class="splitview"><div class="tree" id="treeview">${H.treeHtml(files, fl.path, "sc-file", marks)}</div><div class="code" id="codeview">${H.codeHtml(fl.path, fl.body)}</div></div>`;
    } else {
      body = `<div class="empty" style="padding-top:28px"><div class="ico" style="background:var(--green-soft);color:var(--green)">${ic("circle-check")}</div><h3>Scaffolded ${esc(f.path.split("/").pop())} with ${esc(p.name)} v${p.version}</h3>
        <p>${H.plural(H.filesFor(p).length, "file")} written, including <span class="mono">HARNESS.md</span>. The project is in your registry.</p>
        <div style="display:flex;gap:8px"><a class="btn" href="#/projects/${f.newId}">${ic("folder-git-2")}Open project</a><button class="btn primary" data-a="sc-again">${ic("folder-plus")}Scaffold another</button></div></div>`;
    }
    const can = f.step === 0 ? !!f.path && !f.err : f.step === 1 ? !!f.profile : true;
    return `<div class="page narrow"><div class="crumb"><a href="#/projects" class="muted">${ic("arrow-left")} Projects</a></div>
      <div class="ph"><h1>Scaffold a new project</h1></div>${steps(["Folder", "Profile", "Preview", "Done"], f.step)}${body}
      ${f.step < 3 ? `<div class="flowfoot">${f.step ? `<button class="btn" data-a="sc-back">Back</button>` : ""}<span class="grow"></span>${f.step === 2 ? `<span class="muted">${esc(f.path)} · ${esc(p.name)} v${p.version}</span>` : ""}<button class="btn primary" data-a="sc-next"${can ? "" : " disabled"}>${f.step === 2 ? ic("download") + "Write " + H.plural(H.filesFor(p).length, "file") : "Continue"}</button></div>` : ""}</div>`;
  };
  H.inputs["sc-path"] = (v) => { const f = sc(); f.path = v; f.err = null; H.render(); };
  H.actions["sc-browse"] = () => { S().flow.browse = !S().flow.browse; };
  H.actions["sc-pick"] = (el) => { const f = sc(); f.path = el.dataset.v; f.err = null; S().flow.browse = false; };
  H.actions["sc-to-refit"] = () => { const p = sc().path; S().flow.rf = null; rf().path = p; location.hash = "#/refit"; };
  H.actions["sc-profile"] = (el) => { sc().profile = el.dataset.v; };
  H.actions["sc-file"] = (el) => { sc().sel = el.dataset.path; };
  H.actions["sc-back"] = () => { const f = sc(); f.step -= 1; f.failed = null; };
  H.actions["sc-again"] = () => { S().flow.sc = null; };
  H.actions["sc-next"] = () => {
    const f = sc();
    if (f.step === 0) {
      const known = FOLDERS.find((x) => x.path === f.path.trim());
      if (known && !known.empty) { f.err = `${esc(f.path)} already has a harness (.claude/, CLAUDE.md).`; return; }
      f.step = 1; return;
    }
    if (f.step === 1) { f.step = 2; f.sel = "HARNESS.md"; return; }
    if (f.step === 2) {
      if (/readonly/.test(f.path)) { f.failed = true; return; }
      const p = H.profile(f.profile);
      const name = f.path.split("/").pop() || "new-app";
      const id = H.uid("pr");
      S().projects.unshift({ id, name, path: f.path, profile: p.id, version: p.version, status: "sync", applied: "just now", backups: 0 });
      p.projects.push(id);
      f.newId = id; f.step = 3;
      H.toast(`Wrote ${H.plural(H.filesFor(p).length, "file")} to ${esc(f.path)}`);
    }
  };

  // =============================================================== REFIT
  const rf = () => S().flow.rf || (S().flow.rf = { step: 0, path: "", err: null, profile: null, choice: {}, open: {}, sel: null, nothing: false });
  const REFIT_ITEMS = [
    { path: "CLAUDE.md", proj: true, prof: true, kind: "both", lost: ["## Deploy", "make deploy ENV=staging   # needs VPN", "Never run migrations on Fridays.", "Ping #site-team before changing the CDN config."], lostN: 14 },
    { path: ".claude/settings.json", proj: true, prof: true, kind: "both", lost: ['"Bash(docker compose:*)"', '"Bash(make migrate:*)"', '"Read(./secrets/**)" (deny)'], lostN: 3, what: "permission rules" },
    { path: ".claude/skills/tdd/SKILL.md", proj: true, prof: true, kind: "both" },
    { path: ".claude/agents/code-reviewer.md", proj: true, prof: true, kind: "same" },
    { path: ".claude/skills/old-lint/SKILL.md", proj: true, prof: false, kind: "projonly" },
    { path: ".claude/commands/deploy.md", proj: true, prof: false, kind: "projonly" },
    { path: ".claude/skills/incremental-implementation/SKILL.md", proj: false, prof: true, kind: "profonly" },
    { path: ".claude/agents/test-engineer.md", proj: false, prof: true, kind: "profonly" },
    { path: ".claude/commands/review.md", proj: false, prof: true, kind: "profonly" },
    { path: ".mcp.json", proj: false, prof: true, kind: "profonly" },
    { path: "HARNESS.md", proj: false, prof: true, kind: "profonly" },
  ];
  const defChoice = (it) => (it.kind === "both" ? "replace" : it.kind === "same" ? "keep" : it.kind === "projonly" ? "keep" : "add");
  H.screens.refit = function () {
    const f = rf();
    const p = f.profile ? H.profile(f.profile) : null;
    let body = "";
    if (f.step === 0) {
      const opts = S().projects.filter((x) => x.status !== "drift").slice(0, 3).map((x) => ({ path: x.path, note: "registered · " + ((H.profile(x.profile) || {}).name || "") + " v" + x.version }));
      opts.unshift({ path: "~/work/site", note: ".claude/ with 6 artifacts, CLAUDE.md, settings.json · not managed by Harneo" });
      body = `<div class="field"><label for="rfpath">Project folder</label><input class="input mono" id="rfpath" value="${esc(f.path)}" placeholder="~/work/site" data-in="rf-path">${f.err ? `<span class="err">${ic("circle-alert")}${f.err}</span>` : `<span class="hint">A project that already has a Claude harness. Harneo compares it with a profile item by item.</span>`}</div>
        <div class="group"><header>${ic("folder-search")}<span>Folders with a harness</span></header>${opts.map((o) => `<button class="row" style="width:100%;text-align:left" data-a="rf-pick" data-v="${esc(o.path)}">${ic("folder-git-2")}<span class="mono grow">${esc(o.path)}</span><span class="muted">${esc(o.note)}</span></button>`).join("")}</div>`;
    } else if (f.step === 1) {
      body = `<div class="choices">${S().profiles.map((x) => `<button class="choice${f.profile === x.id ? " on" : ""}" data-a="rf-profile" data-v="${x.id}" aria-pressed="${f.profile === x.id}"><span class="ic t-blue">${ic("layers")}</span><b>${esc(x.name)} <span class="mono muted">v${x.version}</span></b><span class="sec">${esc(x.desc)}</span></button>`).join("")}</div>`;
    } else if (f.step === 2) {
      if (f.nothing) body = `<div class="empty"><div class="ico" style="background:var(--green-soft);color:var(--green)">${ic("circle-check")}</div><h3>Nothing to change</h3><p><span class="mono">${esc(f.path)}</span> already matches ${esc(p.name)} v${p.version}, file for file.</p><a class="btn" href="#/projects">Back to projects</a></div>`;
      else {
        const counts = { replace: 0, add: 0, delete: 0, keep: 0, skip: 0 };
        REFIT_ITEMS.forEach((it) => { counts[f.choice[it.path] || defChoice(it)]++; });
        const backed = REFIT_ITEMS.filter((it) => ["replace", "delete"].includes(f.choice[it.path] || defChoice(it))).length;
        const flagged = REFIT_ITEMS.filter((it) => it.lost && (f.choice[it.path] || defChoice(it)) === "replace");
        body = `${flagged.length ? `<div class="callout bad" style="margin-bottom:14px">${ic("triangle-alert")}<div><b>${flagged.map((it) => `${esc(it.path.split("/").pop())}: ${it.lostN} ${it.what || "lines"} will be lost`).join(" · ")}</b><p>The project's version has content the profile doesn't. Harneo replaces, never merges. A backup is kept, but choose <b>Keep</b> to leave a file untouched.</p></div></div>` : ""}
          <table class="cmp sortable"><thead><tr>${H.th("rf", "path", "File")}${H.th("rf", "proj", "In project")}${H.th("rf", "prof", "In " + p.name)}${H.th("rf", "action", "Action", ' style="width:250px"')}</tr></thead><tbody>
          ${H.sortBy(REFIT_ITEMS, H.getSort("rf"), (it, k) => (k === "path" ? it.path : k === "proj" ? (it.kind === "same" ? 2 : it.proj ? 1 : 0) + "" : k === "prof" ? ({ both: "1", same: "2", profonly: "0", projonly: "3" })[it.kind] : (f.choice[it.path] || defChoice(it)))).map((it) => { const c = f.choice[it.path] || defChoice(it); const flag = it.lost && c === "replace"; const opts = it.kind === "profonly" ? [["add", "Add"], ["skip", "Skip"]] : it.kind === "projonly" ? [["keep", "Keep"], ["delete", "Delete"]] : [["keep", "Keep"], ["replace", "Replace"]];
            return `<tr class="${flag ? "flag" : ""}"><td><span class="mono">${esc(it.path)}</span>${flag ? `<div style="color:var(--red);margin-top:4px;display:flex;gap:6px;align-items:center">${ic("triangle-alert")}<b>${it.lostN} ${it.what || "lines"} will be lost</b><button class="btn ghost sm" data-a="rf-open" data-p="${esc(it.path)}">${f.open[it.path] ? "Hide" : "Show"}</button></div>${f.open[it.path] ? `<div class="lost">${it.lost.map((l) => `<div>− ${esc(l)}</div>`).join("")}${it.lostN > it.lost.length ? `<div class="muted" style="color:var(--t3)">  … ${it.lostN - it.lost.length} more</div>` : ""}</div>` : ""}` : ""}</td>
              <td>${it.proj ? (it.kind === "same" ? `<span class="badge ok">identical</span>` : `<span class="badge">exists</span>`) : `<span class="muted">—</span>`}</td><td>${it.prof ? (it.kind === "both" ? `<span class="badge warn">differs</span>` : it.kind === "same" ? `<span class="badge ok">identical</span>` : `<span class="badge info">new</span>`) : `<span class="muted">—</span>`}</td>
              <td><div class="seg" role="group" aria-label="Action for ${esc(it.path)}">${opts.map(([v, l]) => `<button class="${c === v ? "on" : ""}" data-a="rf-choice" data-p="${esc(it.path)}" data-v="${v}" aria-pressed="${c === v}">${l}</button>`).join("")}</div></td></tr>`; }).join("")}
          </tbody></table>
          <div class="callout" style="margin-top:14px">${ic("archive")}<p>Replace ${counts.replace} · add ${counts.add} · delete ${counts.delete} · keep ${counts.keep}${counts.skip ? " · skip " + counts.skip : ""}. <b>${H.plural(backed, "file")}</b> backed up first to <span class="mono">~/.local/share/io.harneo/backups/site/2026-10-20T0914</span>. <span class="mono">CLAUDE.local.md</span> and <span class="mono">settings.local.json</span> are never touched.</p></div>`;
      }
    } else if (f.step === 3) {
      const files = REFIT_ITEMS.filter((it) => !["keep", "skip"].includes(f.choice[it.path] || defChoice(it)));
      const marks = {}; files.forEach((it) => { const c = f.choice[it.path] || defChoice(it); marks[it.path] = c === "add" ? ["new", "new"] : c === "delete" ? ["del", "delete"] : ["rep", "replace"]; });
      const sel = files.find((x) => x.path === f.sel) || files[0];
      const c = sel ? f.choice[sel.path] || defChoice(sel) : null;
      const lines = !sel ? [] : c === "delete" ? [["-", "---"], ["-", "name: " + sel.path.split("/").slice(-2, -1)[0]], ["-", "---"], ["-", "(whole file removed; backed up)"]] : c === "add" ? (H.filesFor(p).find((x) => x.path === sel.path) || { body: ["(new file from " + p.name + ")"] }).body.map((l) => ["+", l]) : sel.path === "CLAUDE.md" ? [[" ", "# site"], [" ", ""], ["+", "@HARNESS.md"], [" ", ""], [" ", "## Build"], [" ", "npm run build"]].concat(sel.lost.map((l) => ["-", l])) : sel.path.endsWith(".json") ? [[" ", "{"], [" ", '  "permissions": {'], ["-", '    "allow": ["Bash(docker compose:*)", "Bash(make migrate:*)"],'], ["-", '    "deny": ["Read(./secrets/**)"],'], ["+", '    "deny": ["Read(./.env)", "Read(~/.ssh/**)", "Bash(rm -rf:*)"],'], [" ", '    "ask": ["Bash(git push:*)"]'], [" ", "  }"], [" ", "}"]] : [[" ", "---"], [" ", "name: tdd"], ["-", "description: TDD for this repo"], ["+", "description: Red-green-refactor loop for any logic change."], [" ", "---"]];
      body = `<div class="callout info" style="margin-bottom:14px">${ic("eye")}<p><b>Dry run.</b> ${H.plural(files.length, "file")} will change in <span class="mono">${esc(f.path)}</span>. Select a file to see its content diff.</p></div>
        <div class="splitview"><div class="tree" id="treeview">${H.treeHtml(files.map((x) => ({ path: x.path })), sel && sel.path, "rf-file", marks)}</div><div class="code" id="codeview">${sel ? H.codeHtml(sel.path, lines, true) : ""}</div></div>`;
    } else {
      body = `<div class="empty" style="padding-top:28px"><div class="ico" style="background:var(--green-soft);color:var(--green)">${ic("circle-check")}</div><h3>Refitted ${esc(f.path.split("/").pop())} with ${esc(p.name)} v${p.version}</h3>
        <p>${H.plural(f.backed || 0, "original")} backed up. The project is in your registry with a fresh <span class="mono">HARNESS.md</span>.</p>
        <div style="display:flex;gap:8px"><button class="btn" data-a="rf-restore">${ic("rotate-ccw")}Restore originals</button><a class="btn primary" href="#/projects">${ic("folder-git-2")}Go to projects</a></div></div>`;
    }
    const can = f.step === 0 ? !!f.path : f.step === 1 ? !!f.profile : !(f.step === 2 && f.nothing);
    return `<div class="page${f.step >= 2 ? "" : " narrow"}"><div class="crumb"><a href="#/projects" class="muted">${ic("arrow-left")} Projects</a></div>
      <div class="ph"><h1>Refit an existing project</h1>${f.path && f.step ? `<span class="mono muted">${esc(f.path)}</span>` : ""}</div>${steps(["Folder", "Profile", "Compare", "Preview", "Done"], f.step)}${body}
      ${f.step < 4 && !(f.step === 2 && f.nothing) ? `<div class="flowfoot">${f.step ? `<button class="btn" data-a="rf-back">Back</button>` : ""}<span class="grow"></span><button class="btn primary" data-a="rf-next"${can ? "" : " disabled"}>${f.step === 3 ? ic("check") + "Apply refit" : f.step === 2 ? "Preview changes" : "Continue"}</button></div>` : ""}</div>`;
  };
  H.inputs["rf-path"] = (v) => { const f = rf(); f.path = v; f.err = null; H.render(); };
  H.actions["rf-pick"] = (el) => { const f = rf(); f.path = el.dataset.v; f.err = null; };
  H.actions["rf-profile"] = (el) => { rf().profile = el.dataset.v; };
  H.actions["rf-choice"] = (el) => { rf().choice[el.dataset.p] = el.dataset.v; };
  H.actions["rf-open"] = (el) => { const f = rf(); f.open[el.dataset.p] = !f.open[el.dataset.p]; };
  H.actions["rf-file"] = (el) => { rf().sel = el.dataset.path; };
  H.actions["rf-back"] = () => { const f = rf(); f.step -= 1; f.nothing = false; };
  H.actions["rf-restore"] = () => { const f = rf(); H.toast(`Restored ${H.plural(f.backed || 0, "original")} in ${esc(f.path)}`); S().flow.rf = null; location.hash = "#/projects"; };
  H.actions["rf-next"] = () => {
    const f = rf();
    if (f.step === 0) {
      if (/new-app|empty/.test(f.path)) { f.err = `${esc(f.path)} has no Claude harness. Scaffold it instead.`; return; }
      f.step = 1; const reg = S().projects.find((x) => x.path === f.path.trim()); if (reg) f.profile = reg.profile; return;
    }
    if (f.step === 1) { const reg = S().projects.find((x) => x.path === f.path.trim()); f.nothing = !!(reg && reg.profile === f.profile && reg.status === "sync"); f.step = 2; return; }
    if (f.step === 2) { f.step = 3; f.sel = null; return; }
    if (f.step === 3) {
      const p = H.profile(f.profile);
      f.backed = REFIT_ITEMS.filter((it) => ["replace", "delete"].includes(f.choice[it.path] || defChoice(it))).length;
      const reg = S().projects.find((x) => x.path === f.path.trim());
      if (reg) { reg.profile = p.id; reg.version = p.version; reg.status = "sync"; reg.drift = null; reg.applied = "just now"; reg.backups += f.backed; }
      else { const id = H.uid("pr"); S().projects.unshift({ id, name: f.path.split("/").pop(), path: f.path, profile: p.id, version: p.version, status: "sync", applied: "just now", backups: f.backed }); p.projects.push(id); }
      f.step = 4;
      H.toast(`Refitted ${esc(f.path)}; ${H.plural(f.backed, "file")} backed up`);
    }
  };
})();
