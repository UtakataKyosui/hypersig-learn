import { bindings, defineConfig, exports, triggers } from "cf/config";

export const databaseName = "app";
export const localDatabaseId = "00000000-0000-4000-8000-000000000001";

function required(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`Missing ${name}; see docs/ci-cd.md`);
  return value;
}

export default defineConfig(({ mode }) => {
  const production = mode === "production";
  const databaseId = production ? required("D1_DATABASE_ID") : localDatabaseId;
  if (databaseId && !/^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(databaseId)) {
    throw new Error("D1_DATABASE_ID must be a UUID");
  }
  return {
    worker: {
      name: "app",
      compatibilityDate: "2026-10-01",
      entrypoint: "build/worker/shim.mjs",
      observability: {
        enabled: true,
        headSamplingRate: 1,
        // OAuth callbacks contain authorization codes in their query strings.
        redactQueryString: true,
        logs: { enabled: true, invocationLogs: true, persist: true },
      },
      triggers: [triggers.scheduled({ schedule: "* * * * *" })],
      env: {
        // Development uses one stable UUID for both cf's CLI and dev server.
        // Production must bind the existing ID, never provision another DB.
        DB: bindings.d1({ name: databaseName, id: databaseId }),
        HUB: bindings.durableObject({ worker: "app", exportName: "Hub" }),
        ...(production ? {
          VAPID_PUBLIC_KEY: bindings.text(required("VAPID_PUBLIC_KEY")),
          GOOGLE_CLIENT_ID: bindings.text(required("GOOGLE_CLIENT_ID")),
          GOOGLE_REDIRECT_URL: bindings.text(required("GOOGLE_REDIRECT_URL")),
        } : {
          // Declaring local values lets cf dev load them from .dev.vars.
          VAPID_PUBLIC_KEY: bindings.secret(),
          GOOGLE_CLIENT_ID: bindings.secret(),
          GOOGLE_REDIRECT_URL: bindings.secret(),
        }),
        VAPID_PRIVATE_KEY: bindings.secret(),
        VAPID_SUBJECT: bindings.secret(),
        GOOGLE_CLIENT_SECRET: bindings.secret(),
      },
      // Preserve the existing class name and SQLite lifecycle declaration.
      exports: { Hub: exports.durableObject({ storage: "sqlite" }) },
    },
  };
});
