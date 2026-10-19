/* Harneo prototype: projects, project detail, cleanup, settings, first-run welcome. */
(function () {
  "use strict";
  const H = window.H, esc = H.esc, ic = H.icon;
  const S = () => H.S;

  function projStatus(pr) {
    const p = H.profile(pr.profile);
    if (pr.status === "drift") return `<span class="status" style="color:var(--amber)"><i class="dot" style="background:var(--amber-fill)"></i>Drift · ${H.plural(pr.drift.length, "file")}</span>`;
    if (p && pr.version < p.version) return `<span class="status" style="color:var(--blue)"><i class="dot" style="background:var(--blue)"></i>Profile v${p.version} available</span>`;
    return `<span class="status" style="color:var(--green)"><i class="dot" style="background:var(--green-fill)"></i>In sync</span>`;
  }
  H.projStatus = projStatus;

  // ------------------------------------------------------------- list
  H.screens.projects = function (r) {
    if (r.id) return detail(r.id);
    const list = S().projects;
    return `<div class="page">
      <div class="ph"><h1>Projects</h1><span class="muted">Projects Harneo has scaffolded or refitted, with the profile each one runs.</span><span class="grow"></span>
        <a class="btn" href="#/refit">${ic("refresh-cw")}Refit existing</a><a class="btn primary" href="#/scaffold">${ic("folder-plus")}Scaffold new</a></div>
      ${list.length ? `<div class="cards3">${list.map((pr) => { const p = H.profile(pr.profile); return `<a class="pcard" href="#/projects/${pr.id}" style="color:inherit;text-decoration:none">
        <div class="top"><span class="ic t-slate">${ic("folder-git-2")}</span><div style="min-width:0"><h3>${esc(pr.name)}</h3><div class="srcline">${esc(pr.path)}</div></div></div>
        <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap">${ic("layers")}<span>${p ? esc(p.name) : "?"} <span class="mono muted">v${pr.version}</span></span><span style="flex:1"></span>${projStatus(pr)}</div>
        <div class="foot">${ic("clock")}<span>Applied ${esc(pr.applied)}</span><span class="grow"></span><span class="muted">${H.plural(pr.backups, "backup")}</span></div></a>`; }).join("")}</div>`
      : `<div class="empty"><div class="ico">${ic("folder-git-2")}</div><h3>No projects yet</h3><p>Scaffold a new project from a profile, or refit one that already has a harness.</p><div style="display:flex;gap:8px"><a class="btn" href="#/refit">Refit existing</a><a class="btn primary" href="#/scaffold">Scaffold new</a></div></div>`}
    </div>`;
  };

  // ------------------------------------------------------------ detail
  function detail(id) {
    const pr = H.project(id);
    if (!pr) return `<div class="page"><div class="empty"><h3>Project not found</h3><a class="btn" href="#/projects">Back to projects</a></div></div>`;
    const p = H.profile(pr.profile);
    const tab = S().flow.ptab || (pr.status === "drift" ? "drift" : "files");
    const files = p ? H.filesFor(p) : [];
    const dsel = S().flow.dsel || (pr.drift ? pr.drift[0].path : null);
    const fsel = S().flow.fsel && files.some((f) => f.path === S().flow.fsel) ? S().flow.fsel : "HARNESS.md";
    let body = "";
    if (tab === "drift") {
      if (!pr.drift || !pr.drift.length) body = `<div class="empty"><div class="ico">${ic("circle-check")}</div><h3>No drift</h3><p>Every file in ${esc(pr.name)} matches ${p ? esc(p.name) + " v" + pr.version : "its profile"}.</p></div>`;
      else {
        const d = pr.drift.find((x) => x.path === dsel) || pr.drift[0];
        const lines = d.change.startsWith("deleted") ? [["-", "---"], ["-", "description: Run the test suite for changed files"], ["-", "---"], ["-", ""], ["-", "Run the tests related to the files changed in this branch…"]] : d.path.endsWith(".json") ? [[" ", '  "permissions": {'], [" ", '    "deny": ["Read(./.env)", "Read(~/.ssh/**)"],'], ["+", '    "allow": ["Bash(docker compose:*)",'], ["+", '              "Bash(make migrate:*)"],'], [" ", '    "ask": ["Bash(git push:*)"]'], [" ", "  }"]] : [[" ", "---"], [" ", "name: code-reviewer"], ["-", "tools: Read, Grep, Glob, Bash"], ["+", "tools: Read, Grep, Glob"], [" ", "---"], ["+", "Always check migrations against the expand/contract rule."], ["+", "Flag any new endpoint without rate limiting."]];
        body = `<div class="callout warn" style="margin-bottom:14px">${ic("triangle-alert")}<div style="flex:1"><b>${H.plural(pr.drift.length, "file")} in ${esc(pr.name)} no longer match ${esc(p.name)} v${pr.version}.</b><p>Re-apply the profile (the project's versions are backed up first), or promote a change into the library so every project can get it.</p></div><button class="btn sm" data-a="drift-reapply-all">${ic("rotate-ccw")}Re-apply all</button></div>
          <div class="splitview"><div class="tree" id="treeview">${pr.drift.map((x) => `<button class="tnode${x.path === d.path ? " on" : ""}" data-a="drift-sel" data-path="${esc(x.path)}">${ic("file-diff")}${esc(x.path.split("/").pop())}<span class="${x.change.startsWith("deleted") ? "del" : "rep"}">${x.change.startsWith("deleted") ? "deleted" : "+" + x.plus + " −" + x.minus}</span></button>`).join("")}</div>
          <div class="code" id="codeview"><div class="fh">${ic("file-diff")}${esc(d.path)}<span class="muted" style="margin-left:6px">${esc(d.change)}</span><span style="margin-left:auto;display:flex;gap:6px"><button class="btn sm" data-a="drift-promote" data-path="${esc(d.path)}"${d.change.startsWith("deleted") ? " disabled" : ""}>${ic("arrow-up-from-line")}Promote to library</button><button class="btn sm" data-a="drift-reapply" data-path="${esc(d.path)}">${ic("rotate-ccw")}Re-apply profile version</button></span></div>
            <pre>${lines.map((l, i) => `<div class="ln${l[0] === "+" ? " add" : l[0] === "-" ? " del" : ""}"><span class="no">${i + 1}</span><span class="mk">${l[0] === " " ? "" : l[0]}</span><span>${esc(l[1])}</span></div>`).join("")}</pre></div></div>`;
      }
    } else if (tab === "files") {
      const fl = files.find((f) => f.path === fsel) || files[0];
      body = `<div class="splitview"><div class="tree" id="treeview">${H.treeHtml(files, fl ? fl.path : null, "pfile-sel")}</div><div class="code" id="codeview">${fl ? H.codeHtml(fl.path, fl.body) : ""}</div></div>`;
    } else {
      body = `<div class="group">${Array.from({ length: pr.backups }, (_, i) => `<div class="row">${ic("archive")}<div class="grow"><b class="mono" style="font-size:12px">backups/${esc(pr.name)}/2026-${String(10 - i).padStart(2, "0")}-1${i}T21${i}4</b><div class="sec">${i === 0 ? "Before refit with " + esc(p ? p.name : "") + " · 5 files" : "Before re-apply · " + (i + 1) + " files"}</div></div><button class="btn sm" data-a="backup-restore" data-i="${i}">${ic("rotate-ccw")}Restore</button></div>`).join("") || `<div class="row muted">No backups.</div>`}</div>
        <p class="muted" style="margin-top:8px">Backups live in <span class="mono">~/.local/share/io.harneo/backups/${esc(pr.name)}/</span>. Harneo backs up every file before replacing it.</p>`;
    }
    const outdated = p && pr.version < p.version;
    return `<div class="page">
      <div class="crumb"><a href="#/projects" class="muted">${ic("arrow-left")} Projects</a></div>
      <div class="ph"><span class="ic t-slate" style="width:36px;height:36px">${ic("folder-git-2")}</span><div><h1>${esc(pr.name)}</h1><div class="mono muted">${esc(pr.path)}</div></div><span class="grow"></span>
        <button class="btn" data-a="toast" data-msg="Opened ${esc(pr.path)} in your editor">${ic("square-pen")}Open</button>
        <button class="btn" data-a="project-switch">${ic("arrow-left-right")}Switch profile</button>
        <button class="btn danger" data-a="project-remove">${ic("undo-2")}Remove profile</button></div>
      ${outdated ? `<div class="callout info" style="margin-bottom:14px">${ic("circle-arrow-up")}<div style="flex:1"><b>${esc(p.name)} is at v${p.version}; this project runs v${pr.version}.</b><p>Upgrading applies only the difference, with a preview first.</p></div><button class="btn sm primary" data-a="project-upgrade">Upgrade to v${p.version}</button></div>` : ""}
      <div class="stat3"><div class="panel"><span>Profile</span><b><a href="#/profiles/${pr.profile}">${p ? esc(p.name) : "?"}</a> <span class="mono muted">v${pr.version}</span></b></div><div class="panel"><span>Status</span><b>${projStatus(pr)}</b></div><div class="panel"><span>Last applied</span><b>${esc(pr.applied)}</b></div><div class="panel"><span>Backups</span><b>${pr.backups}</b></div></div>
      <div class="tabs" role="tablist">${[["drift", "Drift", "git-compare", pr.drift ? pr.drift.length : 0], ["files", "Installed files", "folder-tree", files.length], ["backups", "Backups", "archive", pr.backups]].map(([k, l, i, n]) => `<button role="tab" class="${tab === k ? "on" : ""}" data-a="ptab" data-v="${k}" aria-selected="${tab === k}">${ic(i)}${l} <span class="muted">${n}</span></button>`).join("")}</div>
      ${body}</div>`;
  }
  const curProj = () => H.project(H.route().id);
  H.actions.ptab = (el) => { S().flow.ptab = el.dataset.v; };
  H.actions["drift-sel"] = (el) => { S().flow.dsel = el.dataset.path; };
  H.actions["pfile-sel"] = (el) => { S().flow.fsel = el.dataset.path; };
  const fixDrift = (pr, path) => { pr.drift = pr.drift.filter((d) => !path || d.path !== path); pr.backups += 1; S().flow.dsel = null; if (!pr.drift.length) { pr.status = "sync"; pr.drift = null; } };
  H.actions["drift-reapply"] = (el) => { const pr = curProj(); fixDrift(pr, el.dataset.path); H.toast(`Re-applied ${esc(el.dataset.path.split("/").pop())}; the project's version is backed up`); };
  H.actions["drift-reapply-all"] = () => { const pr = curProj(); const n = pr.drift.length; fixDrift(pr, null); H.toast(`Re-applied ${H.plural(n, "file")}; originals backed up`); };
  H.actions["drift-promote"] = (el) => {
    const pr = curProj(), path = el.dataset.path;
    H.confirm({ title: "Promote this change to the library?", ok: "Promote to library", body: `<p class="sec">The library copy of <span class="mono">${esc(path.split("/").pop())}</span> becomes the project's version. Profiles that include it can then push it to their other projects.</p>`,
      onOk: () => { fixDrift(pr, path); H.toast("Promoted to the library. Other projects get it when they re-apply."); } });
  };
  H.actions["backup-restore"] = () => { H.toast("Restored the backed-up files. The current versions were backed up first."); return false; };
  H.actions["project-switch"] = () => switchModal(false);
  H.actions["project-upgrade"] = () => switchModal(true);
  function switchModal(upgrade) {
    const pr = curProj();
    S().flow.sw = upgrade ? pr.profile : S().flow.sw || S().profiles.find((p) => p.id !== pr.profile).id;
    H.modal({ title: upgrade ? `Upgrade ${pr.name}` : `Switch ${pr.name} to another profile`, sub: "Only the difference is applied. Every replaced file is backed up first.",
      body: () => {
        const to = H.profile(S().flow.sw), from = H.profile(pr.profile);
        const add = to.items.filter((x) => !from.items.includes(x)), rem = from.items.filter((x) => !to.items.includes(x));
        const chg = upgrade ? ["code-reviewer"] : to.settings !== from.settings ? ["settings"] : [];
        return `${upgrade ? "" : `<div class="optlist" style="margin-bottom:14px">${S().profiles.filter((p) => p.id !== pr.profile).map((p) => `<button class="opt${S().flow.sw === p.id ? " on" : ""}" data-a="sw-pick" data-id="${p.id}"><i class="radio"></i><div><b>${esc(p.name)} v${p.version}</b><div class="sec">${esc(p.desc)}</div></div></button>`).join("")}</div>`}
          <div class="group"><header>${ic("git-compare")}<span>${esc(from.name)} v${pr.version} → ${esc(to.name)} v${to.version}</span></header>
          ${add.map((x) => `<div class="row"><span class="badge ok">Add</span><span class="grow">${esc((H.art(x) || { name: x }).name)}</span></div>`).join("")}
          ${rem.map((x) => `<div class="row"><span class="badge bad">Remove</span><span class="grow">${esc((H.art(x) || { name: x }).name)}</span><span class="muted">restored from backup if it replaced a file</span></div>`).join("")}
          ${chg.map((x) => `<div class="row"><span class="badge warn">Update</span><span class="grow">${esc(x)}</span></div>`).join("")}
          ${!add.length && !rem.length && !chg.length ? `<div class="row muted">Same artifacts; only versions change.</div>` : ""}</div>`;
      },
      foot: `<button class="btn" data-a="modal-close">Cancel</button><button class="btn primary" data-a="sw-ok">${ic("check")}Apply difference</button>` });
  }
  H.actions["sw-pick"] = (el) => { S().flow.sw = el.dataset.id; };
  H.actions["sw-ok"] = () => { const pr = curProj(), to = H.profile(S().flow.sw); pr.profile = to.id; pr.version = to.version; pr.status = "sync"; pr.drift = null; pr.applied = "just now"; pr.backups += 1; S().ui.modal = null; H.toast(`${esc(pr.name)} now runs ${esc(to.name)} v${to.version}. Replaced files are backed up.`); };
  H.actions["project-remove"] = () => {
    const pr = curProj(), p = H.profile(pr.profile), n = p ? H.filesFor(p).length : 0;
    H.confirm({ title: `Remove ${p ? p.name : "the profile"} from ${pr.name}?`, danger: true, ok: "Remove and restore originals",
      body: `<p class="sec">Removes the ${n} files Harneo wrote and restores the ${pr.backups} originals it backed up. <span class="mono">CLAUDE.local.md</span> and <span class="mono">settings.local.json</span> are never touched. The project leaves the registry.</p>`,
      onOk: () => { S().projects = S().projects.filter((x) => x !== pr); location.hash = "#/projects"; H.toast(`Removed the harness from ${esc(pr.name)}; ${pr.backups} originals restored`); } });
  };

  // ------------------------------------------------------------ cleanup
  H.screens.cleanup = function () {
    const arts = S().artifacts;
    const fi = arts.filter((a) => H.status(a) === "findings"), out = arts.filter((a) => H.status(a) === "outdated"), un = arts.filter((a) => H.status(a) === "unused");
    const dup = arts.filter((a) => S().profiles.filter((p) => p.items.includes(a.id)).length >= 3).slice(0, 4);
    const rowA = (a, right) => `<div class="row">${H.typeIc(a.type, true, H.status(a) === "unused")}<div class="grow"><button class="nm" data-a="open" data-id="${a.id}" style="text-align:left">${esc(a.name)}</button><div class="srcline">${esc(H.srcLabel(a))}</div></div>${right}</div>`;
    const inProf = (a) => S().profiles.filter((p) => p.items.includes(a.id));
    return `<div class="page narrow">
      <div class="ph"><h1>Cleanup</h1><span class="muted">Keep the harness lean: what's risky, stale or never used.</span></div>
      ${!fi.length && !out.length && !un.length ? `<div class="empty"><div class="ico">${ic("circle-check")}</div><h3>Nothing to clean up</h3><p>No findings, no outdated or unused artifacts.</p></div>` : ""}
      ${fi.length ? `<div class="group"><header>${ic("shield-alert")}<span>Security findings</span><span class="n">${fi.length}</span></header>${fi.map((a) => rowA(a, `${H.meter(a.trust)}<span class="badge bad">${a.findings.length} findings</span><button class="btn sm danger" data-a="remove-artifact" data-id="${a.id}">Remove</button>`)).join("")}</div>` : ""}
      ${out.length ? `<div class="group"><header>${ic("circle-arrow-up")}<span>Outdated</span><span class="n">${out.length}</span><span class="grow"></span><button class="btn sm" data-a="update-all">Update all</button></header>${out.map((a) => rowA(a, `${a.localEdits ? `<span class="badge warn">local edits</span>` : ""}<span class="mono muted">${esc(H.SRC[a.source].version)} → ${esc(a.latest)}</span><button class="btn sm" data-a="update-artifact" data-id="${a.id}">Update</button>`)).join("")}</div>` : ""}
      ${un.length ? `<div class="group"><header>${ic("moon")}<span>Unused for 8+ weeks, in every project</span><span class="n">${un.length}</span></header>${un.slice(0, 12).map((a) => { const ps = inProf(a); return rowA(a, `<span class="muted">last used ${esc(a.lastUsed)}</span><span class="muted">${ps.length ? "in " + ps.map((p) => esc(p.name)).join(", ") : "in no profile"}</span>${ps.length ? `<button class="btn sm" data-a="unused-prune" data-id="${a.id}">Remove from profiles</button>` : `<button class="btn sm danger" data-a="remove-artifact" data-id="${a.id}">Remove</button>`}`); }).join("")}${un.length > 12 ? `<div class="row"><a href="#/library" data-a="status" data-v="unused">See all ${un.length} in the library</a></div>` : ""}</div>` : ""}
      ${dup.length ? `<div class="group"><header>${ic("copy")}<span>Repeated across profiles</span><span class="n">${dup.length}</span></header>${dup.map((a) => rowA(a, `<span class="muted">in ${inProf(a).map((p) => esc(p.name)).join(", ")}</span>`)).join("")}<div class="row sec">${ic("info")}Not a problem in itself; a candidate for one base profile you clone from.</div></div>` : ""}
    </div>`;
  };
  H.actions["unused-prune"] = (el) => {
    const a = H.art(el.dataset.id), ps = S().profiles.filter((p) => p.items.includes(a.id));
    const projs = [...new Set(ps.flatMap((p) => p.projects))];
    H.confirm({ title: `Remove ${a.name} from ${H.plural(ps.length, "profile")}?`, ok: projs.length ? `Remove and push to ${H.plural(projs.length, "project")}` : "Remove from profiles",
      body: `<p class="sec">Takes it out of ${ps.map((p) => `<b>${esc(p.name)}</b>`).join(", ")} (each saves a new version)${projs.length ? ` and switches ${projs.map((x) => esc((H.project(x) || { name: x }).name)).join(", ")} to the new versions, with a backup of every changed file` : ""}. It stays in your library.</p>`,
      onOk: () => { ps.forEach((p) => { p.items = p.items.filter((x) => x !== a.id); p.version += 1; }); projs.forEach((x) => { const pr = H.project(x); if (pr) { pr.version = H.profile(pr.profile).version; pr.applied = "just now"; } }); H.toast(`Removed ${esc(a.name)} from ${H.plural(ps.length, "profile")}${projs.length ? " and pushed to " + H.plural(projs.length, "project") : ""}`); } });
  };
  H.actions["update-all"] = () => {
    const out = S().artifacts.filter((a) => H.status(a) === "outdated");
    const edited = out.filter((a) => a.localEdits);
    H.confirm({ title: `Update ${H.plural(out.length, "artifact")}?`, ok: `Update ${out.length - edited.length}`,
      body: `<p class="sec">Updates the library copies. Projects get them when they re-apply or switch.</p>${edited.length ? `<div class="callout warn" style="margin-top:12px">${ic("triangle-alert")}<p>${edited.map((a) => esc(a.name)).join(", ")} ${edited.length === 1 ? "has" : "have"} local edits and ${edited.length === 1 ? "is" : "are"} skipped; update ${edited.length === 1 ? "it" : "them"} one by one to choose.</p></div>` : ""}`,
      onOk: () => { out.filter((a) => !a.localEdits).forEach((a) => { a.latest = null; }); H.toast(`Updated ${H.plural(out.length - edited.length, "artifact")}`); } });
  };

  // ----------------------------------------------------------- settings
  H.screens.settings = function () {
    const t = H.themePref();
    return `<div class="page narrow"><div class="ph"><h1>Settings</h1></div>
      <div class="group"><header>${ic("hard-drive")}<span>Data</span></header>
        <div class="row"><div class="grow"><b>Data folder</b><div class="sec">Library, profiles, project registry and backups.</div></div><span class="mono">~/.local/share/io.harneo</span><button class="btn sm ghost" data-a="toast" data-msg="Copied the path">${ic("copy")}</button></div>
        <div class="row"><div class="grow"><b>Session logs (read only)</b><div class="sec">Used for usage counts; never written.</div></div><span class="mono">~/.claude/projects/</span></div></div>
      <div class="group"><header>${ic("bot")}<span>Claude CLI</span></header>
        <div class="row"><div class="grow"><b>${S().claudeCli ? "Found" : "Not found"}</b><div class="sec">${S().claudeCli ? "Used for bundle detection and tag suggestions at import. Artifact text is sent to Claude through your own CLI; no API key." : "Bundles and tags won't be suggested at import; add them by hand."}</div></div><span class="mono">${S().claudeCli ? "~/.local/bin/claude" : "—"}</span>
          <button class="toggle${S().claudeCli ? " on" : ""}" data-a="cli-toggle" role="switch" aria-checked="${S().claudeCli}" aria-label="Simulate Claude CLI present"><i></i></button><span class="muted" style="font-size:12px">prototype: simulate</span></div></div>
      <div class="group"><header>${ic("sun-moon")}<span>Appearance</span></header>
        <div class="row"><div class="grow"><b>Theme</b></div><div class="seg">${[["system", "System"], ["light", "Light"], ["dark", "Dark"]].map(([v, l]) => `<button class="${t === v ? "on" : ""}" data-a="theme-set" data-v="${v}">${l}</button>`).join("")}</div></div>
        <div class="row"><div class="grow"><b>Keyboard shortcuts</b></div><button class="btn sm" data-a="shortcuts">${ic("keyboard")}Show <kbd>?</kbd></button></div></div>
      <div class="group"><header>${ic("info")}<span>About</span></header>
        <div class="row"><div class="grow"><b>Harneo</b><div class="sec">Harness manager for Claude Code. MIT licensed, built from source.</div></div><span class="mono">0.1.0 (prototype)</span></div></div>
    </div>`;
  };
  H.actions["cli-toggle"] = () => { S().claudeCli = !S().claudeCli; };
  H.actions["theme-set"] = (el) => { H.setTheme(el.dataset.v); return false; };

  // ------------------------------------------------------------ welcome
  H.screens.welcome = function () {
    return `<div class="welcome"><div class="wcard">
      <div style="display:flex;align-items:center;gap:10px">${H.logo(32)}<b style="font-size:18px">Harneo</b></div>
      <h1>Your Claude Code harness, fitted to every project.</h1>
      <p class="lead">Harneo keeps the skills, subagents, commands, settings and MCP servers you trust in one library, and installs the right set into any project, with a preview and a backup.</p>
      <div class="flowdiag">
        <div class="node"><b><span class="ic sm t-blue">${ic("layout-grid")}</span>Library</b><p>Everything you import, with its source, trust score, tags and usage. Kept outside your projects.</p></div><div class="arr">${ic("arrow-right")}</div>
        <div class="node"><b><span class="ic sm t-violet">${ic("layers")}</span>Profile</b><p>A named selection, like “Light” or “Heavy”. Picking an artifact brings its whole bundle.</p></div><div class="arr">${ic("arrow-right")}</div>
        <div class="node"><b><span class="ic sm t-slate">${ic("folder-git-2")}</span>Project</b><p>Scaffold a new one or refit an existing one. You see every file first; replaced files are backed up.</p></div>
      </div>
      <div style="display:flex;align-items:center;gap:10px"><button class="btn primary" data-a="welcome-import" style="height:38px;padding:0 16px">${ic("download")}Import your first artifacts</button><button class="btn ghost" data-a="welcome-skip">Skip for now</button><span class="grow" style="flex:1"></span><span class="muted">Start from <span class="mono">~/.claude</span>, a git URL, or a folder.</span></div>
    </div></div>`;
  };
  H.actions["welcome-import"] = () => { S().flow.imp = null; location.hash = "#/import"; };
  H.actions["welcome-skip"] = () => { S().firstRun = false; H.go("#/library"); };
})();
