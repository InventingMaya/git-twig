import { Hono } from "hono";
import skill from "../../../crates/git-twig/skills/git-twig/SKILL.md?raw";

export const onboarding = `# Twig — git-worktrees for agents

Twig creates ordinary linked Git worktrees using native copy-on-write.
Native cloning needs macOS/APFS or Linux on Btrfs or reflink-enabled XFS.
This is a prototype, not a sandbox.

## Install

Check whether git twig --version already works. Otherwise, with Homebrew installed:

\`\`\`sh
brew tap leostera/git-twig https://github.com/leostera/git-twig
brew install --HEAD leostera/git-twig/git-twig
git twig --version
\`\`\`

Homebrew currently builds the latest source; there is no tagged binary release yet.
Do not invent a curl-to-shell installer or a crates.io package.

## Use

Read the bundled skill with git twig skill. Its contents follow.

${skill}
`;

const app = new Hono<{ Bindings: { ASSETS: { fetch: typeof fetch } } }>();
app.get("/llms.txt", (c) => {
  c.header("Cache-Control", "public, max-age=300");
  c.header("X-Content-Type-Options", "nosniff");
  return c.text(onboarding);
});
app.all("*", (c) => c.env.ASSETS.fetch(c.req.raw));
export default app;
