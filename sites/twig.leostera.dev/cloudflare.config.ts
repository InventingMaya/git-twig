import { bindings, defineConfig } from "cf/config";
import * as entrypoint from "./src/worker.ts" with { type: "cf-worker" };

export default defineConfig(({ isPreview }) => ({
  worker: {
    name: isPreview ? "twig-site-preview" : "twig-site",
    compatibilityDate: "2026-11-15",
    entrypoint,
    domains: isPreview ? [] : ["twig.leostera.dev"],
    workersDev: false,
    assets: { runWorkerFirst: ["/llms.txt"] },
    env: { ASSETS: bindings.assets() },
  },
}));
