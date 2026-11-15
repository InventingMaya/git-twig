import { expect, test } from "bun:test";
import { loadBenchmarks, summarize } from "../scripts/benchmarks";

test("uses verified checked-in allocation measurements", () => {
  const data = loadBenchmarks();
  expect(data.map((d) => d.name)).toEqual([
    "Linux kernel",
    "Axo",
    "git-twig",
  ]);
  expect(data[0]!.gitBytes).toBeGreaterThan(data[0]!.twigBytes);
  expect(
    data.every((d) =>
      d.source.startsWith("https://github.com/leostera/git-twig/"),
    ),
  ).toBe(true);
});
test("does not present missing cohorts as zero space", () => {
  expect(() =>
    summarize(
      {
        schemaVersion: 2,
        filesystem: "isolated-image",
        concurrency: 1,
        cohorts: [],
      },
      "linux",
      "Linux",
      "",
    ),
  ).toThrow("Missing paired");
});
