import { expect, test } from "@playwright/test";
test("onboards an agent and shows source-backed space savings", async ({
  page,
  request,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      value: {
        writeText: async (text: string) => {
          (window as typeof window & { copied?: string }).copied = text;
        },
      },
    });
  });
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("twig");
  await expect(page.getByText("git-worktrees for agents")).toBeVisible();
  await page.screenshot({
    path: `test-results/landing-${test.info().project.name}.png`,
    fullPage: true,
  });
  await page.getByRole("button", { name: "Onboard your agent" }).click();
  await expect(page.getByRole("status")).toHaveText(
    "Copied. Paste it into your agent.",
  );
  expect(
    await page.evaluate(
      () => (window as typeof window & { copied?: string }).copied,
    ),
  ).toBe("curl -fsSL https://twig.leostera.dev/llms.txt");
  await expect(
    page.getByRole("heading", { name: "Linux kernel", exact: true }),
  ).toBeVisible();
  await expect(page.locator(".disk-chart")).toHaveCount(1);
  await expect(page.locator(".repo-group")).toHaveCount(4);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  const instructions = await request.get("/llms.txt");
  expect(instructions.headers()["content-type"]).toContain("text/plain");
  expect(await instructions.text()).toContain("brew install --HEAD");
});
test("clipboard failure leaves a selectable command, not a false copied message", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      value: {
        writeText: async () => {
          throw new Error("denied");
        },
      },
    });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Onboard your agent" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Copy the command below",
  );
  await expect(
    page.getByText("curl -fsSL https://twig.leostera.dev/llms.txt", {
      exact: true,
    }),
  ).toBeVisible();
});
