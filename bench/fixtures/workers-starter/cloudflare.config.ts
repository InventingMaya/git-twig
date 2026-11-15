import { bindings, defineConfig } from "cf/config";
import * as entrypoint from "./src/index.ts" with { type: "cf-worker" };

export default defineConfig({
	worker: {
		name: "workers-starter",
		compatibilityDate: "2026-11-13",
		entrypoint,
		env: {
			WORLD: bindings.text("World"),
		},
	},
});
