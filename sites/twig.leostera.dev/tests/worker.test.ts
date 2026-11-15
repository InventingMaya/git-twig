import { expect, test } from "bun:test";
import { plugin } from "bun";
import { readFile } from "node:fs/promises";
plugin({
  name: "raw markdown",
  setup(build) {
    build.onLoad({ filter: /SKILL\.md(?:\?raw)?$/ }, async (args) => ({
      contents: `export default ${JSON.stringify(await readFile(args.path.replace(/\?raw$/, ""), "utf8"))}`,
      loader: "js",
    }));
  },
});
const { default: app } = await import("../src/worker");
test("onboarding is plain text with real install and usage instructions", async () => {
  const response = await app.request("/llms.txt");
  expect(response.status).toBe(200);
  expect(response.headers.get("content-type")).toContain("text/plain");
  const text = await response.text();
  expect(text).toContain("brew install --HEAD leostera/git-twig/git-twig");
  expect(text).toContain("git twig --skill");
  expect(text).toContain("Ask before removing");
  expect(text).not.toContain("/Users/");
});
