import { defineWranglerConfig } from "wrangler/experimental-config";

export default defineWranglerConfig(({ mode }) => ({
	// The isolated Worker test uses the already built WASM in a temp directory.
	...(mode === "test" ? {} : { build: { command: "sh scripts/build.sh" } }),
	dev: { port: 8787 },
	types: { generate: false },
}));
