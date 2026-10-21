/** Architecture gates (CONSTRAINTS.md, "Architecture (TS)"). */
module.exports = {
  forbidden: [
    {
      name: "invoke-only-in-ipc",
      comment:
        "Only src/lib/ipc.ts may import invoke (via @tauri-apps/api/core or the package root).",
      severity: "error",
      from: { pathNot: "^src/lib/ipc\\.ts$" },
      to: {
        path: "^node_modules/@tauri-apps/api/(core|index)\\.",
        dependencyTypesNot: [],
      },
    },
    {
      name: "features-are-independent",
      comment: "A feature never imports another feature.",
      severity: "error",
      from: { path: "^src/features/([^/]+)/" },
      to: { path: "^src/features/([^/]+)/", pathNot: "^src/features/$1/" },
    },
    {
      name: "ui-not-features",
      comment: "src/ui/ never imports src/features/.",
      severity: "error",
      from: { path: "^src/ui/" },
      to: { path: "^src/features/" },
    },
    {
      name: "ui-not-app",
      comment: "src/ui/ never imports src/app/.",
      severity: "error",
      from: { path: "^src/ui/" },
      to: { path: "^src/app/" },
    },
  ],
  options: {
    tsConfig: { fileName: "tsconfig.json" },
    doNotFollow: { path: "node_modules" },
    enhancedResolveOptions: {
      exportsFields: ["exports"],
      conditionNames: ["import", "require", "node", "default", "types"],
      mainFields: ["module", "main", "types", "typings"],
    },
  },
};
