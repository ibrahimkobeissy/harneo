import { describe, expect, it } from "vitest";
import { authAdvice, describeFailure, sshForm } from "./errors";

describe("the authentication advice (#264)", () => {
  it("gives an HTTPS URL its SSH form and the credential-helper way", () => {
    const url =
      "https://github.com/ibrahimkobeissy/private-claude-repo-setup/tree/main";
    expect(sshForm(url)).toBe(
      "git@github.com:ibrahimkobeissy/private-claude-repo-setup.git",
    );
    const say = authAdvice(url);
    expect(say).toContain("This is an HTTPS URL");
    expect(say).toContain(
      "git@github.com:ibrahimkobeissy/private-claude-repo-setup.git",
    );
    expect(say).toContain("gh auth setup-git");
    expect(say).not.toContain("SSH could not log in");
  });

  it("keeps a .git suffix out of the SSH form and falls back without a path", () => {
    expect(sshForm("https://gitlab.com/o/r.git")).toBe(
      "git@gitlab.com:o/r.git",
    );
    expect(sshForm("https://example.com")).toBeNull();
    expect(authAdvice("https://example.com")).toContain(
      "git@host:owner/repo.git",
    );
  });

  it("tells an SSH URL to check the agent, naming its host", () => {
    for (const url of ["git@github.com:o/r.git", "ssh://git@gitlab.com/o/r"]) {
      const say = authAdvice(url);
      expect(say).toMatch(/^SSH could not log in to (github|gitlab)\.com/);
      expect(say).toContain("ssh-add -l");
      expect(say).not.toContain("HTTPS");
    }
  });

  it("is what the import shows for the core's error", () => {
    const failure = describeFailure(
      "git@github.com:o/r.git needs credentials or does not exist; Harneo never",
      "url",
      "git@github.com:o/r.git",
    );
    expect(failure.message).toContain("ssh -T git@github.com");
  });
});
